use ballpump::{aggregate::Window, ingest, metrics::Metrics, model::*};
use std::sync::{Arc, atomic::Ordering::Relaxed};
#[tokio::main(worker_threads = 2)]
async fn main() {
    let count = 1_000_000u64;
    let now = 1_000_000i64;
    let m = Arc::new(Metrics::default());
    let (tx, mut rx) = tokio::sync::mpsc::channel(1024);
    let metrics = m.clone();
    let start = std::time::Instant::now();
    let producer = tokio::spawn(async move {
        for n in 0..count {
            let trade = Trade {
                signature: format!("burst-{n}"),
                slot: n,
                instruction_index: 0,
                inner_instruction_index: Some(1),
                mint: format!("token-{}", n % 10000),
                event_timestamp_ms: now - (n % 599) as i64 * 1000,
                side: if n % 3 == 0 { Side::Sell } else { Side::Buy },
                quote_lamports: 1_000_000_000,
                venue: "PumpFun".into(),
            };
            assert!(ingest::send(&tx, trade, &metrics).await);
        }
    });
    let mut window = Window::default();
    let mut consumed = 0;
    while let Some(trade) = rx.recv().await {
        window.insert(trade, now);
        consumed += 1;
    }
    producer.await.unwrap();
    assert_eq!(consumed, count);
    let elapsed = start.elapsed();
    println!(
        "events={consumed} tokens={} elapsed_ms={} events_per_sec={:.0} channel_capacity=1024 saturation_observations={} local_drops=0",
        window.snapshot("demo".into(), false).indexed_tokens,
        elapsed.as_millis(),
        count as f64 / elapsed.as_secs_f64(),
        m.saturation.load(Relaxed)
    );
}
