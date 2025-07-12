use base64::{engine::general_purpose, Engine as _};
use ed25519_dalek::{SigningKey, Signer};
use chrono::Utc;

use crate::utils::fix_util::build_fix_message;

pub fn compute_raw_data(
    private_key: &SigningKey,
    sender: &str,
    target: &str,
    seq_num: i32,
    sending_time: &str,
) -> String {
    let payload = format!("A\x01{}\x01{}\x01{}\x01{}", sender, target, seq_num, sending_time);
    let sig = private_key.sign(payload.as_bytes());
    general_purpose::STANDARD.encode(sig.to_bytes())
}

pub fn build_logon_message(
    sender: &str,
    target: &str,
    seq_num: i32,
    sending_time: &str,
    raw_data: &str,
    username: &str,
) -> String {
    let raw_len = raw_data.len();

    let fields = vec![
        "8=FIX.4.4".to_string(),
        "9=000".to_string(), // Placeholder
        "35=A".to_string(),
        format!("34={}", seq_num),
        format!("49={}", sender),
        format!("52={}", sending_time),
        format!("56={}", target),
        format!("95={}", raw_len),
        format!("96={}", raw_data),
        "98=0".to_string(),
        "108=30".to_string(),
        "141=Y".to_string(),
        format!("553={}", username),
        "25035=1".to_string(),
    ];

    build_fix_message(fields)
}

pub fn build_heartbeat_message(
    sender: &str,
    target: &str,
    seq_num: i32,
    test_req_id: Option<&str>,
) -> String {
    let sending_time = Utc::now().format("%Y%m%d-%H:%M:%S%.3f").to_string();
    
    let mut fields = vec![
        "8=FIX.4.4".to_string(),
        "9=000".to_string(), // Placeholder
        "35=0".to_string(), // Heartbeat message type
        format!("34={}", seq_num),
        format!("49={}", sender),
        format!("52={}", sending_time),
        format!("56={}", target),
    ];
    
    // Add TestReqID if this is a response to TestRequest
    if let Some(req_id) = test_req_id {
        fields.push(format!("112={}", req_id));
    }

    build_fix_message(fields)
}

pub fn build_market_data_request(
    sender: &str,
    target: &str,
    seq_num: i32,
    req_id: &str,
    symbol: &str,
    entry_types: &[&str], // e.g., ["0", "1"] for BID and OFFER
    market_depth: Option<i32>, // e.g., Some(1) for BookTicker
) -> String {
    let sending_time = Utc::now().format("%Y%m%d-%H:%M:%S%.3f").to_string();

    let mut fields = vec![
        "8=FIX.4.4".to_string(),
        "9=000".to_string(), // placeholder
        "35=V".to_string(),
        format!("49={}", sender),
        format!("56={}", target),
        format!("34={}", seq_num),
        format!("52={}", sending_time),
        format!("262={}", req_id),
        "263=1".to_string(), // 1 = SUBSCRIBE
        format!("146=1"),
        format!("55={}", symbol),
        format!("267={}", entry_types.len()),
    ];

    // Add MDEntryTypes (tag 269)
    for et in entry_types {
        fields.push(format!("269={}", et));
    }

    // Add MarketDepth if specified
    if let Some(depth) = market_depth {
        fields.push(format!("264={}", depth));
    }

    // AggregatedBook is required
    fields.push("266=Y".to_string());

    build_fix_message(fields)
}

pub fn build_new_order_single(
    sender: &str,
    target: &str,
    seq_num: i32,
    symbol: &str,
    side: &str,    // "BUY" or "SELL"
    qty: f64,
    price: f64,
    cl_ord_id: &str, // Original Client Order ID for canceling
) -> String {
    let sending_time = Utc::now().format("%Y%m%d-%H:%M:%S%.3f").to_string();
    let side_code = match side {
        "BUY" => "1",
        "SELL" => "2",
        _ => panic!("Invalid side"),
    };

    let fields = vec![
        "8=FIX.4.4".to_string(),
        "9=000".to_string(), // placeholder
        "35=D".to_string(), // New Order Single
        format!("34={}", seq_num),
        format!("49={}", sender),
        format!("56={}", target),
        format!("52={}", sending_time),
        format!("11={}", cl_ord_id),
        format!("55={}", symbol),
        format!("54={}", side_code),
        format!("38={}", qty),
        "40=2".to_string(), // LIMIT order
        format!("44={}", price),
        "59=1".to_string(), // TimeInForce = GTC
    ];

    build_fix_message(fields)
}

pub fn build_order_cancel_request(
    sender: &str,
    target: &str,
    seq_num: i32,
    symbol: &str,
    cancel_cl_ord_id: &str,
    orig_cl_ord_id: &str,
) -> String {
    let sending_time = Utc::now().format("%Y%m%d-%H:%M:%S%.3f").to_string();

    let fields = vec![
        "8=FIX.4.4".to_string(),
        "9=000".to_string(), // placeholder
        "35=F".to_string(),  // OrderCancelRequest
        format!("49={}", sender),
        format!("56={}", target),
        format!("34={}", seq_num),
        format!("52={}", sending_time),
        format!("11={}", cancel_cl_ord_id),
        format!("41={}", orig_cl_ord_id),
        format!("55={}", symbol),
    ];

    build_fix_message(fields)
}

// Helper function to extract a field value from a FIX message
pub fn extract_field(message: &str, tag: &str) -> Option<String> {
    let search_pattern = format!("{}=", tag);
    if let Some(start) = message.find(&search_pattern) {
        let value_start = start + search_pattern.len();
        let value_end = message[value_start..].find('\x01').unwrap_or(message.len() - value_start);
        Some(message[value_start..value_start + value_end].to_string())
    } else {
        None
    }
}
