use serde::{Deserialize, Serialize};

pub const WINDOW_MS: i64 = 600_000;
pub const PUMP: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";
pub const AMM: &str = "pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA";
pub const WSOL: &str = "So11111111111111111111111111111111111111112";
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum Side {
    Buy,
    Sell,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Trade {
    pub signature: String,
    pub slot: u64,
    pub instruction_index: u32,
    pub inner_instruction_index: Option<u32>,
    pub mint: String,
    pub event_timestamp_ms: i64,
    pub side: Side,
    pub quote_lamports: u64,
    pub venue: String,
}
impl Trade {
    pub fn key(&self) -> String {
        format!(
            "{}:{}:{:?}",
            self.signature, self.instruction_index, self.inner_instruction_index
        )
    }
}
#[derive(Clone, Default, Debug, Serialize)]
pub struct Token {
    pub mint: String,
    pub symbol: Option<String>,
    pub name: Option<String>,
    pub image_url: Option<String>,
    pub buy_lamports: String,
    pub sell_lamports: String,
    pub net_lamports: String,
    pub trade_count: u64,
    pub last_trade_ms: i64,
}
#[derive(Clone, Debug, Serialize)]
pub struct Snapshot {
    pub r#type: &'static str,
    pub window_seconds: u32,
    pub as_of_ms: i64,
    pub status: String,
    pub coverage: &'static str,
    pub commitment: &'static str,
    pub indexed_tokens: usize,
    pub net_flow: Vec<Token>,
    pub buy_volume: Vec<Token>,
}
pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}
