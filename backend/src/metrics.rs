use serde::Serialize;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering::Relaxed};
#[derive(Default)]
pub struct Metrics {
    pub received: AtomicU64,
    pub swaps: AtomicU64,
    pub failures: AtomicU64,
    pub partial_traces: AtomicU64,
    pub unverified_trade_calls: AtomicU64,
    pub excluded: AtomicU64,
    pub duplicates: AtomicU64,
    pub expired: AtomicU64,
    pub future: AtomicU64,
    pub saturation: AtomicU64,
    pub reconnects: AtomicU64,
    pub last_message_ms: AtomicI64,
    pub latest_chain_ms: AtomicI64,
    pub decode_degraded_until: AtomicI64,
    pub aggregation_us: AtomicU64,
    pub snapshot_bytes: AtomicU64,
}
#[derive(Debug, Serialize)]
pub struct Report {
    received_transactions: u64,
    decoded_swaps: u64,
    decode_failures: u64,
    partial_log_transactions: u64,
    unverified_trade_calls: u64,
    excluded_non_sol_quotes: u64,
    duplicates: u64,
    expired: u64,
    future: u64,
    channel_saturation: u64,
    reconnect_attempts: u64,
    last_message_ms: i64,
    lag_ms: i64,
    aggregation_us: u64,
    snapshot_bytes: u64,
    dropped_events: u64,
    provider_cost: Option<u64>,
}
impl Metrics {
    pub fn report(&self) -> Report {
        Report {
            received_transactions: self.received.load(Relaxed),
            decoded_swaps: self.swaps.load(Relaxed),
            decode_failures: self.failures.load(Relaxed),
            partial_log_transactions: self.partial_traces.load(Relaxed),
            unverified_trade_calls: self.unverified_trade_calls.load(Relaxed),
            excluded_non_sol_quotes: self.excluded.load(Relaxed),
            duplicates: self.duplicates.load(Relaxed),
            expired: self.expired.load(Relaxed),
            future: self.future.load(Relaxed),
            channel_saturation: self.saturation.load(Relaxed),
            reconnect_attempts: self.reconnects.load(Relaxed),
            last_message_ms: self.last_message_ms.load(Relaxed),
            lag_ms: (crate::model::now_ms() - self.latest_chain_ms.load(Relaxed)).max(0),
            aggregation_us: self.aggregation_us.load(Relaxed),
            snapshot_bytes: self.snapshot_bytes.load(Relaxed),
            dropped_events: 0,
            provider_cost: None,
        }
    }
}
