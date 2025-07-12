use std::env;
use std::sync::Arc;
use chrono::Utc;
use tokio::sync::Mutex;
use tokio::net::TcpStream;
use tokio_native_tls::TlsStream;
use tokio_util::codec::Framed;
use futures_util::{SinkExt, StreamExt};
use log::{debug, error, info};
use uuid::Uuid;

use crate::utils::fix_util::FixCodec;
use crate::utils::key_util::load_signing_key;
use crate::utils::message_util::{
    compute_raw_data,
    build_logon_message,
    build_heartbeat_message,
    build_market_data_request,
    build_new_order_single,
    build_order_cancel_request,
    extract_field,
};
use crate::utils::connection_util::connect_fix_endpoint;
use crate::types::{StrategyState,};


pub async fn start_market_data_client(strategy: Arc<Mutex<StrategyState>>) -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    let sender_comp_id = Uuid::new_v4()
        .simple()
        .to_string()[..8]
        .to_string();

    let target_comp_id = env::var("BINANCE_TARGET_COMP_ID")?;
    let username = env::var("BINANCE_API_KEY")?;
    let signing_key = load_signing_key()?;
    let sending_time = Utc::now().format("%Y%m%d-%H:%M:%S%.3f").to_string();
    let msg_seq_num = 1;

    let raw_data = compute_raw_data(
        &signing_key,
        &sender_comp_id,
        &target_comp_id,
        msg_seq_num,
        &sending_time,
    );

    let hostname = env::var("BINANCE_MD_HOSTNAME").unwrap();
    let port: u16 = env::var("BINANCE_PORT").unwrap().parse()?;
    let mut framed = connect_fix_endpoint(&hostname, port).await?;

    // Build and send logon message
    let logon_msg = build_logon_message(
        &sender_comp_id,
        &target_comp_id,
        msg_seq_num,
        &sending_time,
        &raw_data,
        &username,
    );

    debug!("Sending Market Data Logon: {}", logon_msg.replace('\x01', "|"));
    framed.send(logon_msg).await?;
    info!("Sent Market Data Logon");

    // Start sequence number after logon
    let mut seq_num = 2;

    // Monitor the stream for messages
    while let Some(msg) = framed.next().await {
        match msg {
            Ok(line) => {
                debug!("Received: {}", line.replace('\x01', "|"));
                
                // Parse the message to check if it's a TestRequest
                if let Some(msg_type) = extract_field(&line, "35") {
                    match msg_type.as_str() {
                        "1" => {
                            // TestRequest - respond with Heartbeat
                            if let Some(test_req_id) = extract_field(&line, "112") {
                                let heartbeat = build_heartbeat_message(
                                    &sender_comp_id,
                                    &target_comp_id,
                                    seq_num,
                                    Some(&test_req_id),
                                );
                                debug!("Sending Heartbeat: {}", heartbeat.replace('\x01', "|"));
                                framed.send(heartbeat).await?;
                                info!("Sent Heartbeat in response to TestRequest");
                                seq_num += 1;
                            }
                        },
                        "0" => {
                            // Heartbeat received
                            info!("Received Heartbeat");
                        },
                        "A" => {
                            // Logon acknowledgment
                            info!("Market Data Logon successful");

                            // Build a MarketDataRequest for Book Ticker
                            let req_id = "BOOK_TICKER_STREAM";
                            let symbol = "BTCUSDT";
                            let entry_types = vec!["0", "1"]; // BID and OFFER
                            let book_msg = build_market_data_request(
                                &sender_comp_id,
                                &target_comp_id,
                                seq_num,
                                req_id,
                                symbol,
                                &entry_types,
                                Some(1), // MarketDepth = 1
                            );
                            debug!("Sending MarketDataRequest: {}", book_msg.replace('\x01', "|"));
                            info!("Sending MarketDataRequest");
                            framed.send(book_msg).await?;
                            seq_num += 1;
                        },
                        "X" | "W" => {
                            handle_market_data_with_strategy(
                                &line,
                                Arc::clone(&strategy),
                                &mut framed,
                                &sender_comp_id,
                                &target_comp_id,
                                &mut seq_num,
                            ).await;
                        },
                        "5" => {
                            // Logout
                            info!("Logout received, session ending");
                            break;
                        },
                        _ => {
                            // Other message types
                            info!("Received message type: {}", msg_type);
                        }
                    }
                }
            },
            Err(e) => {
                error!("Error reading from stream: {:?}", e);
                break;
            }
        }
    }

    Ok(())
}

async fn handle_market_data_with_strategy(
    message: &str,
    state: Arc<Mutex<StrategyState>>,
    framed: &mut Framed<TlsStream<TcpStream>, FixCodec>,
    sender_comp_id: &str,
    target_comp_id: &str,
    seq_num: &mut i32,
) {
    let symbol = extract_field(message, "55").unwrap_or_default();
    let side_tag = extract_field(message, "269").unwrap_or_default(); // 0 = BID, 1 = ASK
    let price_str = extract_field(message, "270").unwrap_or_default();
    let qty_str = extract_field(message, "271").unwrap_or_default();

    log::info!(
        "MarketData | Symbol: {} | Side: {} | Price: {} | Qty: {}",
        symbol, side_tag, price_str, qty_str
    );

    let price = price_str.parse::<f64>().unwrap_or(0.0);
    let qty = qty_str.parse::<f64>().unwrap_or(0.0);

    // Lock the state
    let mut state = state.lock().await;

    if !state.oe_logon_ready {
        log::info!("⚠️ Order entry session not ready yet.");
        return;
    }

    let reference_price = state.reference_price;
    let buy_threshold = reference_price * 0.99;
    let sell_threshold = reference_price * 1.01;

    if side_tag == "0" && price > sell_threshold && state.side.as_deref() != Some("SELL") {
        // SELL signal
        if let Some(ref orig_id) = state.active_order_id {
            let cancel_id = Uuid::new_v4().to_string();
            let cancel_msg = build_order_cancel_request(
                sender_comp_id,
                target_comp_id,
                *seq_num,
                &symbol,
                &cancel_id,
                orig_id,
            );
            if let Err(e) = framed.send(cancel_msg).await {
                log::error!("Failed to send cancel: {}", e);
            }
            *seq_num += 1;
        }

        let cl_ord_id = Uuid::new_v4().to_string();
        let order_msg = build_new_order_single(
            sender_comp_id,
            target_comp_id,
            *seq_num,
            &symbol,
            "SELL",
            0.0001,
            price,
            &cl_ord_id,
        );
        if let Err(e) = framed.send(order_msg).await {
            log::error!("Failed to send SELL order: {}", e);
        }
        *seq_num += 1;

        state.active_order_id = Some(cl_ord_id);
        state.side = Some("SELL".to_string());

        log::info!("📈 Strategy Signal - SELL @ {:.2} | Qty: {} | Symbol: {}", price, qty, symbol);
    }

    if side_tag == "1" && price < buy_threshold && state.side.as_deref() != Some("BUY") {
        // BUY signal
        if let Some(ref orig_id) = state.active_order_id {
            let cancel_id = Uuid::new_v4().to_string();
            let cancel_msg = build_order_cancel_request(
                sender_comp_id,
                target_comp_id,
                *seq_num,
                &symbol,
                &cancel_id,
                orig_id,
            );
            if let Err(e) = framed.send(cancel_msg).await {
                log::error!("Failed to send cancel: {}", e);
            }
            *seq_num += 1;
        }

        let cl_ord_id = Uuid::new_v4().to_string();
        let order_msg = build_new_order_single(
            sender_comp_id,
            target_comp_id,
            *seq_num,
            &symbol,
            "BUY",
            0.0001,
            price,
            &cl_ord_id,
        );
        if let Err(e) = framed.send(order_msg).await {
            log::error!("Failed to send BUY order: {}", e);
        }
        *seq_num += 1;

        state.active_order_id = Some(cl_ord_id);
        state.side = Some("BUY".to_string());

        log::info!("📉 Strategy Signal - BUY @ {:.2} | Qty: {} | Symbol: {}", price, qty, symbol);
    }
}
