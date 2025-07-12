pub struct StrategyState {
    pub reference_price: f64,
    pub active_order_id: Option<String>,
    pub side: Option<String>,
    pub oe_logon_ready: bool,
}
