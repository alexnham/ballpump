# BallPump

**Watch memecoins pump in real time.** A rolling view of buying pressure through Pump.fun and PumpSwap. Every token is a bouncing ball; positive net SOL flow determines its size. Switch to gross buy volume at any time.

## Run locally

### Docker Compose

With Docker Desktop running and several gigabytes of free disk space for the first image build, start the complete app from the repository root:

```sh
docker compose up --build -d
```

Open [BallPump](http://localhost:3000). This builds React and Rust in separate build stages, then serves both the UI and WebSocket feed from one container. Demo mode needs no `.env` or credentials. No host Rust or Node installation is required.

```sh
docker compose logs -f ballpump
docker compose down
```

If port 3000 is already occupied, use `PORT=3001 docker compose up --build -d` and open [port 3001](http://localhost:3001). Set `PORT=3001` in `.env` to keep that port for subsequent `up` commands.

For live mode, copy `.env.example` to `.env`, set `SOURCE=live` and your LaserStream endpoint/key, then run `docker compose up -d` again. Compose injects only the source credentials and logging settings; the container keeps its own bind address, static asset path, and checkpoint path. Secrets are excluded from the image build context. A named volume preserves the replay cursor across container recreation; the in-memory window still needs replay/warm-up. `docker compose down` preserves this volume; `docker compose down -v` deletes it.

The container runs as a non-root user, and its logs rotate at 10 MiB with three files retained. Its healthcheck verifies that HTTP responds; consult `/health`'s `status` for stream freshness and coverage. Published ports bind to localhost only.

### Without Docker

Requirements: current stable Rust, Node.js 22.12+ (24 recommended), npm, and a C/C++ build toolchain for the SDK's protobuf dependency. The first Rust build can take several minutes.

From this repository's root:

```sh
cp .env.example .env
npm ci --prefix frontend
npm run build --prefix frontend
cargo run --manifest-path backend/Cargo.toml
```

Open [BallPump](http://127.0.0.1:3000). The default backend generates synthetic trades through the real rolling aggregator and WebSocket publisher. The UI labels them **Demo** and disables token links. Stop with Ctrl+C.

For frontend development, run `npm run dev --prefix frontend` in another terminal and open [Vite](http://127.0.0.1:5173). Vite proxies `/ws`, `/health`, and `/metrics` to port 3000. Open [standalone demo](http://127.0.0.1:5173/?demo=1) to use the frontend without a backend. The Explore demo control does the same thing.

## Live mainnet activity

Set `SOURCE=live`, `LASERSTREAM_ENDPOINT` to your provisioned mainnet gRPC endpoint, and `LASERSTREAM_API_KEY` in `.env`, then restart the backend. The example endpoint must be available on your Helius plan. Missing credentials fail at startup. Keys remain server-side. Do not put them in any `VITE_` variable.

Live mode requests finalized, successful, non-vote transactions with either program in `account_include` (OR semantics), including mints outside the visible leaderboard. All qualifying SOL-quoted trades are indexed; only 100 entries per metric are published and 50 balls rendered.

**Live LaserStream connectivity and reconnection have not been credential-tested. Coverage is unverified.** Public mainnet transactions are checked into `fixtures/` and tested against amounts and balance changes. Replay tests prove local idempotency, not provider delivery completeness. Warming, stale, and degraded states are shown explicitly. The UI never equates a coverage gap with zero activity.

## Architecture

```text
LaserStream SDK → bounded raw channel (128)
                → invocation validation + trade-event decoder
                → bounded trade-batch channel (1024)
                → rolling in-memory totals + deduplication
                → 1 Hz full snapshots (Tokio watch)
                → Axum WebSocket → React / Canvas / Matter.js
```

One Rust service. No external database, message broker, or signing wallet. Modules live in `backend/src/`; UI code in `frontend/src/`. Official IDLs and their provenance live in `docs/upstream/`. Physics and rendering run independently of incoming snapshots. Ball positions persist between rankings, radii interpolate, size is bounded and nonlinear, and reduced-motion preferences freeze movement. A keyboard-accessible token list complements the canvas.

Both live token symbols and images use safe fallbacks when metadata is unavailable. This MVP does not yet enrich token metadata. It supports supplied metadata in the wire contract.

## Configuration

| Variable | Default | Purpose |
| --- | --- | --- |
| `SOURCE` | `demo` | `demo` or `live`; invalid values fail startup |
| `BIND_ADDR` | `127.0.0.1:3000` | HTTP and WebSocket listen address |
| `FRONTEND_DIST` | `frontend/dist` | Static build directory, relative to process working directory |
| `CHECKPOINT_PATH` | `.ballpump-cursor` | Durable consumed slot checkpoint; writable parent directory required |
| `LASERSTREAM_ENDPOINT` | required in live mode | Provisioned Helius mainnet gRPC endpoint |
| `LASERSTREAM_API_KEY` | required in live mode | Helius secret; never sent to the browser |
| `RUST_LOG` | `ballpump=info` | Structured JSON logging filters |
| `VITE_WS_URL` | same-origin `/ws` | Optional public frontend WebSocket URL |

The Rust process loads `.env` from its working directory. Start it from the repository root. Bind to loopback for local use; public deployment needs TLS, access controls and connection limits at a reverse proxy.

## API and observability

- `/ws`: approximately one full leaderboard snapshot per second; new clients immediately receive the latest snapshot. Both `net_flow` and `buy_volume` arrays are included so toggling needs no resubscription. Integer lamport totals are decimal strings.
- `/health`: the current full snapshot, including source status and timestamp. HTTP 200 means the service is answering; inspect `status` for feed health.
- `/metrics`: JSON counters for received transactions, decoded swaps, failures, non-SOL exclusions, duplicates, expired/future events, channel saturation, outer reconnect attempts, lag, latest aggregation time, and outgoing snapshot size. Provider cost is `null` because the stream does not supply billing data. `dropped_events=0` measures local channel drops only; it is not a claim about provider delivery.
- JSON logs add transactions/sec, swaps/sec, token count and status each second. SDK reconnect details appear in its own logs; the outer reconnect counter cannot count the SDK's silent retries.

See [data semantics and coverage](docs/coverage.md), [decoder evidence](docs/decoding.md), and [validation results](docs/validation.md) before relying on live totals.

## Verify

```sh
cargo fmt --manifest-path backend/Cargo.toml --check
cargo test --manifest-path backend/Cargo.toml
cargo clippy --manifest-path backend/Cargo.toml --all-targets -- -D warnings
npm test --prefix frontend
npm run build --prefix frontend
cargo build --manifest-path backend/Cargo.toml
node scripts/smoke.mjs
cargo run --release --manifest-path backend/Cargo.toml --example load
cargo run --release --manifest-path backend/Cargo.toml --example channel_load
```

The smoke test starts its own service on port 3099, validates HTTP/WebSocket messages, disconnects and reconnects a client, and restarts the process. It requires no credentials. Golden fixtures cover both venues and quote exclusions; generated tests cover multi-hop/boost/post-completion events. Regenerate schemas after deliberately reviewing an IDL update with `python3 scripts/generate_decoder.py`, followed by `cargo fmt --manifest-path backend/Cargo.toml`.

Buying pressure does not establish price appreciation, token safety, or legitimacy. BallPump is an analytics visualization, not a trading bot.
