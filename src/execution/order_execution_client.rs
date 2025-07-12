use std::env;
use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use log::{debug, info};
use uuid::Uuid;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::utils::key_util::load_signing_key;
use crate::utils::message_util::{
    compute_raw_data,
    build_logon_message,
    build_new_order_single,
    build_order_cancel_request,
    extract_field,
};
use crate::utils::connection_util::connect_fix_endpoint;
use crate::types::{StrategyState,};


pub async fn start_order_entry_session(strategy: Arc<Mutex<StrategyState>>) -> anyhow::Result<()> {
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

    let hostname = env::var("BINANCE_OE_HOSTNAME").unwrap();
    let port: u16 = env::var("BINANCE_PORT").unwrap().parse()?;
    let mut framed = connect_fix_endpoint(&hostname, port).await?;

    // Send logon
    let logon_msg = build_logon_message(
        &sender_comp_id,
        &target_comp_id,
        msg_seq_num,
        &sending_time,
        &raw_data,
        &username,
    );
    framed.send(logon_msg).await?;
    info!("Sent order-entry logon");

    // Wait for logon acknowledgment
    let mut seq = 2;
    while let Some(msg) = framed.next().await {
        let msg = msg?;
        debug!("Received: {}", msg.replace('\x01', "|"));
        if extract_field(&msg, "35").as_deref() == Some("A") {
            info!("Order entry logon successful");

            let mut state = strategy.lock().await;
            state.oe_logon_ready = true; // Mark order entry session as ready

            // Generate a unique ClOrdID for the order
            let orig_cl_ord_id = Uuid::new_v4().to_string();

            // Send a NewOrderSingle
            let order_msg = build_new_order_single(
                &sender_comp_id,
                &target_comp_id,
                seq,
                "BTCUSDT",
                "BUY",       // or "SELL"
                0.0001,      // quantity
                100000.0,    // limit price
                &orig_cl_ord_id,
            );
            framed.send(order_msg).await?;
            info!("Sent NewOrderSingle | ClOrdID = {}", orig_cl_ord_id);
            seq += 1;

            // Wait a bit before canceling
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;

            // Generate a cancel ClOrdID
            let cancel_cl_ord_id = Uuid::new_v4().to_string();

            // Send OrderCancelRequest
            let cancel_msg = build_order_cancel_request(
                &sender_comp_id,
                &target_comp_id,
                seq,
                "BTCUSDT",
                &cancel_cl_ord_id,
                &orig_cl_ord_id,
            );
            framed.send(cancel_msg).await?;
            info!(
                "Sent OrderCancelRequest | OrigClOrdID = {}, CancelClOrdID = {}",
                orig_cl_ord_id, cancel_cl_ord_id
            );
            seq += 1;
        }
        else if extract_field(&msg, "35").as_deref() == Some("8") {
            let exec_type = extract_field(&msg, "150").unwrap_or_default();
            if exec_type.is_empty() {
                info!("ExecutionReport received | ExecType is missing");
                debug!("Raw ExecutionReport: {}", msg.replace('\x01', "|"));
            } else {
                debug!("ExecutionReport | ExecType = {}", exec_type);
                handle_execution_report(&msg);
            }
        }
    }

    Ok(())
}

fn handle_execution_report(message: &str) {
    let cl_ord_id = extract_field(message, "11").unwrap_or_default();
    let exec_type = extract_field(message, "150").unwrap_or_default();
    let _ord_status = extract_field(message, "39").unwrap_or_default();
    let symbol = extract_field(message, "55").unwrap_or_default();
    let side = extract_field(message, "54").unwrap_or_default();
    let qty = extract_field(message, "38").unwrap_or_default();
    let price = extract_field(message, "44").unwrap_or_default();
    let text = extract_field(message, "58").unwrap_or_default(); // error reason (if any)

    match exec_type.as_str() {
        "0" => log::info!(
            "Order Accepted | Symbol: {} | Side: {} | Qty: {} | Price: {} | ClOrdID: {}",
            symbol, side, qty, price, cl_ord_id
        ),
        "4" => log::info!(
            "Order Canceled | Symbol: {} | ClOrdID: {}",
            symbol, cl_ord_id
        ),
        "8" => log::error!(
            "Order Rejected | ClOrdID: {} | Reason: {}",
            cl_ord_id, text
        ),
        _ => log::warn!(
            "Unknown ExecType received | ExecType: {} | ClOrdID: {}",
            exec_type, cl_ord_id
        ),
    }
}
