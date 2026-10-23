# Data semantics and operational decisions

## Coverage

All successful finalized transactions involving either official mainnet program are subscribed, without a token allowlist. This is **Pump.fun + PumpSwap activity**, not all trades of Pump-origin tokens on other DEXs. Only SOL/WSOL-quoted legs enter SOL rankings. Non-SOL quotes, including pump-coin quotes and stablecoins, are excluded and counted. A multi-hop route can consequently contribute only its SOL-quoted leg; downstream token volume is not relabeled as SOL. No unverified price conversion is applied.

Known `buy`, exact-input buy, `sell`, v2/v3 variants, multi-hop events, synthetic post-completion buys, and PumpSwap boost buy-and-burn events are recognized from pinned official IDLs. Added/changed instruction or event layouts require updates. Missing invocation traces or truncated known event fields fail closed and increment decode failures. Historical event layouts shorter than the current schema are not supported. Those failures make totals incomplete and are surfaced as degraded status; they are not zero volume.

Single trade, buy-and-burn, and synthetic post-completion events are counted individually by their event positions. Synthetic post-completion quote flow is attributed to Pump.fun, where it executes. A routed swap's individual venue legs are separate swap events. Boost/post-completion/multi-hop support is derived from official schemas and documentation and has only synthetic tests so far.

## Amounts and direction

Buy/sell refers to the base Pump token. Pump's legacy native-SOL paths use executed `TradeEvent.sol_amount`; quote-aware paths use `quote_amount`, requiring a SOL quote account. A default public key in a Pump event represents native SOL. PumpSwap buys use `quote_amount_in_with_lp_fee - lp_fee`; sells use `quote_amount_out`. These are quote amounts before trading fees. In the observed exact-input AMM examples, `quote_amount_in` is the input budget, so it must not be used as executed volume. Balance reconciliation tests protect this distinction. Fees, tips, rents and wallet SOL balance changes are not counted as trading flow.

Internal totals use `u128`, event amounts use `u64`, and subtraction for positive net rankings never underflows. Wire values are signed/unsigned decimal strings. JavaScript converts to floating point only for labels and bounded radius scaling.

## Time and memory

The window at wall-clock time `T` is `(T - 600000 ms, T]`. Exactly-at-cutoff events expire. Event timestamps are the programs' executed Clock timestamps in seconds, converted to milliseconds; receipt time is not substituted. The watermark never moves backwards if the host clock is adjusted. Future timestamps are rejected and cause degraded status, so keep the host clock synchronized. Delayed arrivals inside the window are inserted at their event time; older arrivals are counted as expired and ignored. Slot order does not determine expiration.

A sparse ordered map of event-time buckets stores per-mint deltas and deduplication keys; running totals serve rankings. On-chain timestamps have second resolution, so there are at most 600 occupied on-chain time buckets. Expiration touches only buckets crossing the cutoff, removes their keys, subtracts deltas, and removes inactive token totals. It does not scan each token's entire history. This is simpler than a dense ring while preserving exact boundary behavior for millisecond fixtures and demo trades. Cost scales with active mints, swap keys, and occupied mint/time pairs in the trailing window. It is not a fixed-memory service under arbitrary input rates.

Deduplication key: signature + outer instruction index + inner event index. Loaded writable and read-only addresses are appended to static keys in the documented order. Inner events retain their provider instruction index and verified stack height. Multiple swaps in one transaction are never collapsed into one signature. Dedup keys expire with their events; replays older than the window cannot re-add expired activity.

## Finality and recovery

Finalized commitment favors correctness over freshness (Helius documents roughly 12–15 seconds of finality delay). Processed forks are not counted, and this MVP has no rollback implementation. A finalized-chain failure would require clearing state and rebuilding.

The official SDK enables replay/reconnect and tracks stream slots. On first startup the service requests approximately 6,000 slots of overlap from the finalized head. On restart it requests the durable consumed cursor minus 6,000 slots. This is deliberately an overlap estimate, **not** a conversion of slots into an exact 10-minute interval. `subscribe_replay_info` supplies the available lower bound; unavailable/truncated replay is logged. Helius currently documents roughly 48 hours of replay, with older-than-roughly-20-minute replay served from finalized historical storage. Bounds and availability can change.

The checkpoint is the maximum decoded-and-consumed transaction slot, written once per second through a temporary file, `sync_all`, and atomic rename. It is a replay hint, not a database or a complete-window snapshot. The directory must exist. It does not prove that every transaction in that slot was seen. A restart discards the old window and replays with overlap; it always marks the result warming for a full 600 seconds of healthy reception. This prevents a cursor alone from being mistaken for restored state. Directory fsync/power-loss durability is not guaranteed; overlap and warm-up cover an old/missing checkpoint conservatively.

The SDK can retry silently. A 20-second stream watchdog rebuilds a terminal/stalled session with overlap; a 15-second reception gap or chain lag over 60 seconds marks stale. Any stale period restarts the 600-second warm-up. Decoding/future-time errors and outer reconnects mark the window degraded for 600 seconds. Backpressure awaits bounded channels instead of dropping data. If queues cannot keep up, chain lag and saturation expose it.

`live` means locally fresh and past the warm-up, **not verified gap-free coverage**. Replay and ordering guarantees have not been validated with live credentials or controlled outages. The UI retains this qualification while live. If replay storage has an internal gap, the current cursor API cannot detect it. Alert on failures, stale/degraded status, lag, saturation, and provider-side usage before treating the system as an operational index.
