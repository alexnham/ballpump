use ballpump::{aggregate::Window, model::*};
fn main() {
    let count = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(1_000_000u64);
    let mut w = Window::default();
    let start = std::time::Instant::now();
    let now = 1_000_000i64;
    for n in 0..count {
        w.insert(
            Trade {
                signature: format!("load-{n}"),
                slot: n,
                instruction_index: 0,
                inner_instruction_index: Some(1),
                mint: format!("token-{}", n % 10000),
                event_timestamp_ms: now - (n % 599) as i64 * 1000,
                side: if n % 3 == 0 { Side::Sell } else { Side::Buy },
                quote_lamports: 1_000_000_000,
                venue: "PumpFun".into(),
            },
            now,
        );
    }
    let elapsed = start.elapsed();
    let s = std::time::Instant::now();
    let snapshot = w.snapshot("demo".into(), false);
    let size = serde_json::to_vec(&snapshot).unwrap().len();
    println!(
        "events={count} tokens={} ingest_ms={} events_per_sec={:.0} snapshot_us={} snapshot_bytes={size}",
        snapshot.indexed_tokens,
        elapsed.as_millis(),
        count as f64 / elapsed.as_secs_f64(),
        s.elapsed().as_micros()
    );
    w.advance(now + WINDOW_MS);
    assert_eq!(w.snapshot("demo".into(), false).indexed_tokens, 0);
}
