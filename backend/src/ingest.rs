use crate::{decoder, metrics::Metrics, model::*};
use anyhow::{Context, Result};
use futures_util::StreamExt;
use helius_laserstream::{LaserstreamClient, LaserstreamConfig, grpc::*, subscribe};
use std::{
    sync::{Arc, atomic::Ordering::Relaxed},
    time::Duration,
};
use tokio::sync::mpsc;

pub enum Raw {
    Transaction(Box<SubscribeUpdateTransaction>),
    ChainTime(i64),
}
pub struct Batch {
    pub trades: Vec<Trade>,
    pub slot: u64,
}
pub async fn send<T>(tx: &mpsc::Sender<T>, item: T, metrics: &Metrics) -> bool {
    if tx.capacity() == 0 {
        metrics.saturation.fetch_add(1, Relaxed);
    }
    tx.send(item).await.is_ok()
}
pub async fn decode_loop(mut rx: mpsc::Receiver<Raw>, tx: mpsc::Sender<Batch>, m: Arc<Metrics>) {
    while let Some(raw) = rx.recv().await {
        match raw {
            Raw::ChainTime(ms) => {
                m.latest_chain_ms.fetch_max(ms, Relaxed);
            }
            Raw::Transaction(update) => {
                let slot = update.slot;
                let decoded = decoder::normalize(&update).map(|tx| decoder::decode(&tx));
                let trades = match decoded {
                    Ok(d) => {
                        m.swaps.fetch_add(d.trades.len() as u64, Relaxed);
                        m.failures.fetch_add(d.failures, Relaxed);
                        m.excluded.fetch_add(d.excluded_quotes, Relaxed);
                        if d.failures > 0 {
                            m.decode_degraded_until.store(now_ms() + WINDOW_MS, Relaxed);
                        }
                        d.trades
                    }
                    Err(e) => {
                        tracing::warn!(error=%e,slot,"invalid transaction envelope");
                        m.failures.fetch_add(1, Relaxed);
                        m.decode_degraded_until.store(now_ms() + WINDOW_MS, Relaxed);
                        vec![]
                    }
                };
                if !send(&tx, Batch { trades, slot }, &m).await {
                    break;
                }
            }
        }
    }
}
pub async fn live(tx: mpsc::Sender<Raw>, m: Arc<Metrics>, checkpoint: String) -> Result<()> {
    let endpoint = std::env::var("LASERSTREAM_ENDPOINT")
        .context("LASERSTREAM_ENDPOINT is required in live mode")?;
    let key = std::env::var("LASERSTREAM_API_KEY")
        .context("LASERSTREAM_API_KEY is required in live mode")?;
    let mut cursor = tokio::fs::read_to_string(checkpoint)
        .await
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .map(|s| s.saturating_sub(6000));
    loop {
        let config =
            LaserstreamConfig::new(endpoint.clone(), key.clone()).with_max_reconnect_attempts(3);
        // SDK handles transient reconnects and replay. Rebuild after terminal failures.
        let attempt = async {
            let client = LaserstreamClient::connect(config.clone()).await?;
            let head = client
                .get_slot(Some(CommitmentLevel::Finalized))
                .await?
                .slot;
            let first = client.subscribe_replay_info().await?.first_available;
            let requested = cursor
                .map(|s| s.saturating_sub(2))
                .unwrap_or(head.saturating_sub(6000));
            let from_slot = first.map(|first| requested.max(first));
            if from_slot != Some(requested) {
                tracing::warn!(
                    requested,
                    ?first,
                    "replay unavailable or truncated; window must warm up"
                );
            }
            let request = SubscribeRequest {
                transactions: [(
                    "pump-ecosystem".into(),
                    SubscribeRequestFilterTransactions {
                        account_include: vec![PUMP.into(), AMM.into()],
                        vote: Some(false),
                        failed: Some(false),
                        ..Default::default()
                    },
                )]
                .into(),
                blocks_meta: [("clock".into(), Default::default())].into(),
                commitment: Some(CommitmentLevel::Finalized as i32),
                from_slot,
                ..Default::default()
            };
            let (stream, _handle) = subscribe(config, request);
            tokio::pin!(stream);
            loop {
                // The SDK retries silently; a watchdog ensures prolonged silence is visible.
                let next = tokio::time::timeout(Duration::from_secs(20), stream.next())
                    .await
                    .context("stream watchdog timeout")?;
                let update = next.context("stream ended")??;
                match update.update_oneof {
                    Some(subscribe_update::UpdateOneof::Transaction(t)) => {
                        m.last_message_ms.store(now_ms(), Relaxed);
                        m.received.fetch_add(1, Relaxed);
                        let slot = t.slot;
                        if !send(&tx, Raw::Transaction(Box::new(t)), &m).await {
                            return Ok(());
                        }
                        cursor = Some(cursor.unwrap_or(slot).max(slot));
                    }
                    Some(subscribe_update::UpdateOneof::BlockMeta(b)) => {
                        m.last_message_ms.store(now_ms(), Relaxed);
                        if let Some(time) = b.block_time
                            && !send(&tx, Raw::ChainTime(time.timestamp * 1000), &m).await
                        {
                            return Ok(());
                        }
                    }
                    _ => {}
                }
            }
            #[allow(unreachable_code)]
            Ok::<(), anyhow::Error>(())
        }
        .await;
        if tx.is_closed() {
            return Ok(());
        }
        m.reconnects.fetch_add(1, Relaxed);
        m.decode_degraded_until.store(now_ms() + WINDOW_MS, Relaxed);
        // Avoid logging provider error bodies, which may contain credentials or endpoint URLs.
        tracing::warn!(
            failed = attempt.is_err(),
            "LaserStream session ended; retrying with overlap"
        );
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}
pub async fn demo(tx: mpsc::Sender<Batch>, m: Arc<Metrics>) {
    let mut tick = tokio::time::interval(Duration::from_millis(100));
    let mut n = 0u64;
    loop {
        tick.tick().await;
        n += 1;
        let now = now_ms();
        m.last_message_ms.store(now, Relaxed);
        m.latest_chain_ms.store(now, Relaxed);
        let i = (n % 45 + 1) as u8;
        let trade = Trade {
            signature: format!("demo-{n}"),
            slot: n,
            instruction_index: 0,
            inner_instruction_index: None,
            mint: bs58::encode([i; 32]).into_string(),
            event_timestamp_ms: now,
            side: if n.is_multiple_of(4) {
                Side::Sell
            } else {
                Side::Buy
            },
            quote_lamports: (46 - i as u64).pow(2) * 17_000_000 + (n % 13) * 100_000_000,
            venue: if n.is_multiple_of(2) {
                "PumpFun"
            } else {
                "PumpSwap"
            }
            .into(),
        };
        m.received.fetch_add(1, Relaxed);
        m.swaps.fetch_add(1, Relaxed);
        if !send(
            &tx,
            Batch {
                trades: vec![trade],
                slot: n,
            },
            &m,
        )
        .await
        {
            break;
        }
    }
}
