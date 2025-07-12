mod types;
mod utils;
mod market_data;
mod execution;

use std::io::Write;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::types::{StrategyState,};


#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    init_logger();

    log::info!("Binance FIX Trading Bot Starting...");

    let strategy_state = Arc::new(Mutex::new(StrategyState {
        reference_price: 100000.0,
        active_order_id: None,
        side: None,
        oe_logon_ready: false,
    }));

    // Clone shared state for each task
    let price_state = Arc::clone(&strategy_state);
    let order_state = Arc::clone(&strategy_state);

    // Spawn Market Data Session
    tokio::spawn(async move {
        if let Err(e) = market_data::market_data_client::start_market_data_client(price_state).await {
            log::error!("FIX market data stream failed: {}", e);
        }
    });

    // Spawn Order Entry Session
    tokio::spawn(async move {
        if let Err(e) = execution::order_execution_client::start_order_entry_session(order_state).await {
            log::error!("FIX order entry session failed: {}", e);
        }
    });

    // Keep the app alive
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
}

fn init_logger() {
    env_logger::Builder::from_default_env()
        .format(|f, record| {
            writeln!(
                f,
                "[{} {}] {}",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                record.level(),
                record.args()
            )
        })
        .init();
}
