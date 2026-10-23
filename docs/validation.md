# Validation — 2026-10-23

## Passed

- `cargo test --manifest-path backend/Cargo.toml`: 11 tests (3 rolling-window unit tests and 8 decoder/replay integration tests).
- Exact 600-second cutoff, out-of-order insertion, duplicate event identity, simultaneous buys/sells, negative/zero net flow, future timestamps, and totals exceeding `u64`.
- Seven recorded SOL trade examples across both venues and both directions; four recorded non-SOL exclusions; a recorded program mention with no trade.
- Curve balance reconciliation and four independent WSOL vault reconciliations.
- Invocation ancestry, caught CPI failure, failed transaction, malformed events, multiple swaps per signature, and rebuilding a fresh window with replay.
- Schema-built multi-hop/boost/post-completion cases, explicitly distinguished from observed golden examples.
- `cargo fmt --check` and Clippy on all targets with warnings denied.
- `npm test --prefix frontend`: 4 tests for the wire contract, integer serialization, demo rankings, bounded radii, and unsafe URL rejection.
- Frontend TypeScript strict checking and Vite production build: approximately 404 kB JS / 124 kB gzip.
- `node scripts/smoke.mjs`: live local HTTP and WebSocket endpoints, current full snapshot on reconnect, demo integer contract, and service restart.
- Desktop Chrome visually displayed the arena, correct demo label, readable ball labels, and switched to gross buy volume.

## Synthetic load measurements

macOS ARM64, optimized Rust build. These are local core measurements, not an assertion about live network ingestion capacity.

| Workload | Measured result |
| --- | --- |
| 1,000,000 events, 10,000 active mints, 599 timestamp buckets, direct aggregation | 1,890 ms insertion; about 529k events/sec |
| Same direct workload, top-100 snapshots for both metrics | 710 μs snapshot construction plus serialization; 38,998-byte JSON |
| Direct workload peak RSS (`/usr/bin/time -l`) | 386,514,944 bytes (about 369 MiB) |
| 1,000,000 events via a bounded 1,024-capacity Tokio channel, two worker threads | 1,058 ms; about 945k events/sec |
| Channel workload | 57,183 observations of a full channel; awaited capacity; all 1,000,000 consumed; zero local drops |
| Channel workload peak RSS | 382,697,472 bytes (about 365 MiB) |

Development tools were active concurrently, and the workloads allocate differently; do not compare the two rates as an optimization claim. An earlier direct run measured about 987k events/sec, showing the variability. Synthetic signatures are shorter than real Solana signatures; actual stream memory per key will be higher. Memory scales with the number of distinct events in the trailing window. Decoder and TLS/gRPC costs are not included in these throughput numbers.

Reproduce:

```sh
cargo build --release --manifest-path backend/Cargo.toml --example load --example channel_load
/usr/bin/time -l backend/target/release/examples/load
/usr/bin/time -l backend/target/release/examples/channel_load
```

The direct load example also expires the entire window and asserts that no token totals remain. The channel example asserts the exact consumed count.

## Still unverified

- Credentialed LaserStream connection, provisioned endpoint availability, bandwidth/billing, replay bounds in practice, controlled outages, terminal SDK reconnects, and complete restoration under a real provider restart. No gap-free coverage claim.
- Economic reconciliation of observed multi-hop, boost, synthetic post-completion, and single-venue v2/v3 SOL trades. Their parsing is schema-tested; non-SOL v3 samples are observed exclusions.
- Historical shorter event layouts and future incompatible protocol layouts; these fail closed.
- Sustained mainnet decoding throughput and soak behavior, including real signatures and actual transaction sizes.
- Mobile visual inspection and a measured frontend FPS run. Browser automation detached/timed out during the final responsive checks. The app includes a `data-fps` diagnostic on its canvas, responsive CSS, and reduced-motion handling, but no numeric frame-rate claim is made.
- Token metadata enrichment. Live views use mint fallbacks until symbols/images are supplied.
