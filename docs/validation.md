# Validation — 2026-10-23

## Passed

- `cargo test --manifest-path backend/Cargo.toml`: 19 tests (3 rolling-window unit tests, 2 metadata tests, and 14 decoder/replay integration tests).
- Exact 600-second cutoff, out-of-order insertion, duplicate event identity, simultaneous buys/sells, negative/zero net flow, future timestamps, and totals exceeding `u64`.
- Seven recorded SOL trade examples across both venues and both directions; four recorded non-SOL exclusions; a recorded program mention with no trade.
- Curve balance reconciliation and four independent WSOL vault reconciliations.
- Invocation ancestry, caught CPI failure, failed transaction, malformed events, multiple swaps per signature, and rebuilding a fresh window with replay.
- Paired LaserStream/RPC regression for truncated trailing logs, proven outer success, unresolved inner-call exclusion, spoofed log-text rejection, non-SOL uncertainty, and partial-trace ingestion metrics.
- Schema-built multi-hop/boost/post-completion cases, explicitly distinguished from observed golden examples.
- `cargo fmt --check` and Clippy on all targets with warnings denied.
- `npm test --prefix frontend`: 9 tests for the wire contract, integer serialization, demo rankings, bounded radii, unsafe URL rejection, coin-name fallbacks, ball momentum transfer, and viewport wall bounces (including low speeds).
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

- Controlled outages, provider billing, replay bounds, terminal SDK reconnects, and complete restoration under a real provider restart. Credentialed ingestion is running; historical startup replay was observed not to catch up on this connection. The local run explicitly uses `REPLAY_ON_START=false` and warms a fresh window. No gap-free coverage claim.
- Economic reconciliation of observed multi-hop, boost, synthetic post-completion, and single-venue v2/v3 SOL trades. Their parsing is schema-tested; non-SOL v3 samples are observed exclusions.
- Historical shorter event layouts and future incompatible protocol layouts; these fail closed. Unresolved inner outcomes still reduce coverage; benign clipping of unrelated trailing logs does not.
- Sustained mainnet decoding throughput and soak behavior, including real signatures and actual transaction sizes.
- A controlled sustained frontend FPS measurement across devices. The canvas exposes `data-fps`, ball count, and loaded-image count; no numeric performance guarantee is made.
- Missing or broken remote artwork, and metadata availability for every ranked mint. The DAS response shape was checked with three current live mints; names, symbols, and HTTPS artwork were returned. Metadata parser and both leaderboard enrichments are tested without modifying accounting.

## Fullscreen arena update

- Checked the full-viewport canvas at 1280 × 720 with no document overflow, metric switching, and the accessible token list in the in-app browser.
- Verified pointer dragging on a paused ball. The system reports reduced motion; the app starts paused and an explicit Resume enables the physics simulation.
- Rebuilt the frontend with TypeScript strict checking and Vite, ran the eight frontend tests, and passed Rust release tests, formatting, and Clippy with warnings denied.
- Packaged the checked Linux Rust binary in the normal runtime image and restarted with Docker Compose. Credentials remain server-side.

- Live WebSocket verification: full snapshots on two connections, 100 ranked tokens, 95 names and 94 artwork URLs at observation, positive lossless net-lamport strings.
- Inspected actual live artwork in the fullscreen arena at a narrow 319 × 941 viewport and corrected the compact header. Inspected the desktop layout at 1280 × 720.
- Verified startup from the finalized head with `REPLAY_ON_START=false`: live ingestion lag about one second, fresh visible trades, and incomplete/degraded status retained.

- Proportional sizing: area ratios are exact at 2×/4× flow, including sub-SOL values, tiny flows, and desktop/mobile dimensions. Removed the minimum-radius boost. Nine frontend tests and the production build passed; the live view was visually checked.
