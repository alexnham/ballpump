use crate::model::*;
use std::collections::{BTreeMap, HashMap, HashSet};
#[derive(Default)]
struct Total {
    buy: u128,
    sell: u128,
    count: u64,
    last: i64,
}
#[derive(Default)]
struct Bucket {
    tokens: HashMap<String, Total>,
    keys: Vec<String>,
}
#[derive(Default)]
pub struct Window {
    buckets: BTreeMap<i64, Bucket>,
    totals: HashMap<String, Total>,
    seen: HashSet<String>,
    watermark: i64,
}
#[derive(Debug, PartialEq)]
pub enum Insert {
    Added,
    Duplicate,
    Expired,
    Future,
}
impl Window {
    pub fn advance(&mut self, now: i64) {
        self.watermark = self.watermark.max(now);
        while self
            .buckets
            .first_key_value()
            .is_some_and(|(t, _)| *t <= self.watermark - WINDOW_MS)
        {
            let (_, b) = self.buckets.pop_first().unwrap();
            for key in b.keys {
                self.seen.remove(&key);
            }
            for (mint, old) in b.tokens {
                let total = self.totals.get_mut(&mint).unwrap();
                total.buy -= old.buy;
                total.sell -= old.sell;
                total.count -= old.count;
                if total.count == 0 {
                    self.totals.remove(&mint);
                }
            }
        }
    }
    pub fn insert(&mut self, t: Trade, now: i64) -> Insert {
        self.advance(now);
        if t.event_timestamp_ms <= self.watermark - WINDOW_MS {
            return Insert::Expired;
        }
        if t.event_timestamp_ms > self.watermark {
            return Insert::Future;
        }
        let key = t.key();
        if !self.seen.insert(key.clone()) {
            return Insert::Duplicate;
        }
        let b = self.buckets.entry(t.event_timestamp_ms).or_default();
        b.keys.push(key);
        for total in [
            b.tokens.entry(t.mint.clone()).or_default(),
            self.totals.entry(t.mint).or_default(),
        ] {
            match t.side {
                Side::Buy => total.buy += t.quote_lamports as u128,
                Side::Sell => total.sell += t.quote_lamports as u128,
            }
            total.count += 1;
            total.last = total.last.max(t.event_timestamp_ms);
        }
        Insert::Added
    }
    pub fn snapshot(&self, status: String, demo: bool) -> Snapshot {
        let rank = |gross: bool| {
            let mut entries: Vec<_> = self
                .totals
                .iter()
                .filter(|(_, t)| if gross { t.buy > 0 } else { t.buy > t.sell })
                .collect();
            let score = |t: &Total| {
                if gross {
                    t.buy
                } else {
                    t.buy.saturating_sub(t.sell)
                }
            };
            let cmp = |a: &(&String, &Total), b: &(&String, &Total)| {
                score(b.1).cmp(&score(a.1)).then_with(|| a.0.cmp(b.0))
            };
            if entries.len() > 100 {
                entries.select_nth_unstable_by(100, cmp);
                entries.truncate(100);
            }
            entries.sort_unstable_by(cmp);
            entries
                .into_iter()
                .map(|(mint, t)| Token {
                    mint: mint.clone(),
                    symbol: if demo {
                        Some(format!(
                            "ORB{}",
                            bs58::decode(mint)
                                .into_vec()
                                .unwrap_or_default()
                                .first()
                                .copied()
                                .unwrap_or(0)
                        ))
                    } else {
                        None
                    },
                    buy_lamports: t.buy.to_string(),
                    sell_lamports: t.sell.to_string(),
                    net_lamports: if t.buy >= t.sell {
                        (t.buy - t.sell).to_string()
                    } else {
                        format!("-{}", t.sell - t.buy)
                    },
                    trade_count: t.count,
                    last_trade_ms: t.last,
                    ..Default::default()
                })
                .collect()
        };
        Snapshot {
            r#type: "leaderboard",
            window_seconds: 600,
            as_of_ms: self.watermark,
            status,
            coverage: "Pump.fun + PumpSwap · SOL/WSOL quotes",
            commitment: if demo { "synthetic" } else { "finalized" },
            indexed_tokens: self.totals.len(),
            net_flow: rank(false),
            buy_volume: rank(true),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn trade(id: &str, at: i64, side: Side, amount: u64) -> Trade {
        Trade {
            signature: id.into(),
            slot: 1,
            instruction_index: 0,
            inner_instruction_index: None,
            mint: "mint".into(),
            event_timestamp_ms: at,
            side,
            quote_lamports: amount,
            venue: "PumpFun".into(),
        }
    }
    #[test]
    fn exact_boundary_and_out_of_order() {
        let mut w = Window::default();
        assert_eq!(
            w.insert(trade("new", 600_001, Side::Buy, 20), 600_001),
            Insert::Added
        );
        assert_eq!(
            w.insert(trade("old", 2, Side::Sell, 10), 600_001),
            Insert::Added
        );
        assert_eq!(
            w.insert(trade("expired", 1, Side::Buy, 99), 600_001),
            Insert::Expired
        );
        assert_eq!(
            w.snapshot("demo".into(), false).net_flow[0].net_lamports,
            "10"
        );
        w.advance(600_002);
        assert_eq!(
            w.snapshot("demo".into(), false).net_flow[0].net_lamports,
            "20"
        );
        w.advance(1_200_001);
        assert_eq!(w.totals.len(), 0);
        assert!(w.seen.is_empty());
    }
    #[test]
    fn replay_and_multiple_swaps() {
        let mut w = Window::default();
        let a = trade("tx", 100, Side::Buy, u64::MAX);
        assert_eq!(w.insert(a.clone(), 100), Insert::Added);
        assert_eq!(w.insert(a.clone(), 100), Insert::Duplicate);
        let mut b = a.clone();
        b.inner_instruction_index = Some(1);
        w.insert(b, 100);
        let s = w.snapshot("demo".into(), false);
        assert_eq!(
            s.net_flow[0].buy_lamports,
            (2 * u64::MAX as u128).to_string()
        );
        assert_eq!(s.net_flow[0].trade_count, 2);
    }
    #[test]
    fn negative_zero_future_and_last_time() {
        let mut w = Window::default();
        w.insert(trade("a", 10, Side::Buy, 10), 20);
        w.insert(trade("b", 20, Side::Sell, 20), 20);
        let s = w.snapshot("demo".into(), false);
        assert!(s.net_flow.is_empty());
        assert_eq!(s.buy_volume[0].net_lamports, "-10");
        assert_eq!(s.buy_volume[0].last_trade_ms, 20);
        assert_eq!(
            w.insert(trade("future", 21, Side::Buy, 2), 20),
            Insert::Future
        );
        w.insert(trade("c", 20, Side::Buy, 10), 20);
        assert!(w.snapshot("demo".into(), false).net_flow.is_empty());
    }
}
