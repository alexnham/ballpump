# BallPump — Codex Project Instructions

## Product

Build **BallPump**, a playful, real-time visualization of trading activity across the Pump.fun ecosystem. Each token is a bouncing ball. Its size reflects its **positive net buy flow over the last rolling 10 minutes**. Users should be able to switch to gross buy volume. The app is a visualization/analytics tool, not an automated trading bot.

The visual experience is the product: fast, fluid, attractive, and understandable at a glance. Avoid turning the MVP into a conventional trading dashboard.

## Scope and terminology

- Index **all relevant swaps** through the Pump.fun bonding-curve program and PumpSwap AMM, not just a list of popular mints or newly created tokens.
- Known mainnet program IDs (verify with official, current sources before relying on them):
  - Pump.fun: `6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P`
  - PumpSwap: `pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA`
- **Coverage boundary:** Tracking these programs is not equivalent to tracking every trade of Pump-origin tokens on every Solana DEX. Clearly label metrics as Pump.fun + PumpSwap activity.
- **Window:** A continuously rolling trailing 600 seconds, not fixed 10-minute intervals that reset together.
- **Primary score:** `net_flow_sol = buy_volume_sol - sell_volume_sol`. Display only positive-net-flow tokens in the default balls view, ranked descending.
- **Secondary score:** Gross buy volume over the same window.
- **Direction:** Buy/sell is from the perspective of the Pump token, not the trader's SOL balance alone.
- **Units:** Do not silently count token units as SOL. If a pool is not SOL-quoted, define a separately labeled conversion mechanism or exclude that pool from SOL-denominated rankings pending correct pricing. Document coverage.
- The complete stream is indexed, but only the top 50–100 qualifying tokens need to be rendered concurrently.

## Initial architecture — no Kafka

```text
Helius LaserStream (filtered transaction stream)
  -> Rust ingestion / validation / trade decoding
  -> in-memory sliding-window aggregator
  -> leaderboard snapshot publisher
  -> WebSocket endpoint
  -> React + TypeScript + ball physics visualization
```

Implement the MVP as a **single Rust backend service** with logically separated ingestion, decoding, aggregation, and broadcasting modules/tasks. Use bounded Tokio channels for backpressure and instrument their saturation. Do not introduce Kafka, Redis, ClickHouse, or Kubernetes without a measured requirement. For local development, support an offline replay fixture / mock trade source so the UI and aggregation can be tested without paying for a live stream.

## Backend requirements

1. Subscribe to transactions involving either program, using supported provider filters. Verify current LaserStream API shapes and program IDs from current documentation; never invent SDK methods or protobuf fields.
2. Decode only valid Pump.fun or PumpSwap **trade** instructions. Inspect top-level and inner instructions where necessary. A transaction merely mentioning a program is not proof of a trade.
3. Normalize each decoded swap into a trade event:

   ```rust
   // Conceptual schema; adjust types for precision and interoperability.
   struct TradeEvent {
       signature: String,
       slot: u64,
       instruction_index: u32,
       inner_instruction_index: Option<u32>,
       mint: String,
       event_timestamp_ms: i64,
       side: TradeSide, // Buy | Sell, relative to token
       quote_lamports: u64,
       venue: Venue,    // PumpFun | PumpSwap
   }
   ```

4. Avoid floating-point arithmetic for monetary accounting. Aggregate integer lamports where the quote is SOL/WSOL; convert only for API display.
5. Deduplicate **individual swap events**, not merely transaction signatures: one transaction may contain multiple swaps. Use a stable composite event key based on signature plus instruction/event position, with verified handling of inner instructions.
6. Maintain per-token buy/sell totals and trade counts for the last 600 seconds. Prefer an efficient ring buffer of fixed-duration buckets plus running totals; expire old buckets incrementally. Do not scan full histories on every frontend update. Record the trade-event timestamp policy and treatment of delayed arrivals, out-of-order slots, and events near window boundaries.
7. Produce a leaderboard snapshot roughly once per second, including mint, buy volume, sell volume, net flow, trade count, last trade time, and token metadata if known. Avoid rebuilding unnecessary state on every render tick.
8. Recover from stream interruption: use provider-supported replay/reconnect mechanisms where available, persist an appropriate durable cursor/checkpoint, and reprocess with deduplication. **Do not claim gap-free coverage until tested.** Define and surface degraded/stale state. A cursor alone does not restore a lost 10-minute in-memory window; use replay/warm-up or a validated snapshot strategy.
9. Be explicit about processed/confirmed/finalized commitment and fork consequences. Favor correctness and explain any freshness tradeoffs. Verify the provider's supported replay bounds and semantics.
10. Emit structured logs and metrics: received transactions/sec, decoded swaps/sec, decode failures, lag, dropped events, reconnects, aggregation latency, token count, outgoing snapshot size, and provider/bandwidth costs where observable.
11. Configuration belongs in environment variables with a checked-in `.env.example`. Never commit keys or secrets.

## Frontend requirements

- React + TypeScript, dark theme, responsive design.
- Main view is animated bouncing/colliding balls with token logo and ticker where metadata exists; use a readable fallback otherwise.
- Smooth transitions in ball sizes as ranking changes; do not abruptly teleport balls on every update.
- Primary view: **10m Net Buy Flow**. Toggle: **10m Buy Volume**.
- Clearly show the metric, its rolling window, a live/stale indicator, and SOL units.
- Use a nonlinear, bounded size mapping (e.g., square-root scale plus minimum and maximum sizes) so outliers do not consume the whole viewport.
- Use efficient rendering (Canvas or PixiJS with a suitable physics engine such as Matter.js). Limit rendered balls, not indexed trades.
- Click a ball to open the corresponding token page on Pump.fun; handle invalid/missing metadata safely.
- Use gentle buy/sell pulse animations without overloading the render loop. Respect reduced-motion preferences.
- A fixture/demo mode must work without the backend.
- Do not falsely imply that net inflow proves price appreciation or legitimacy.

## API contract (initial proposal; refine before implementation)

WebSocket message sent at approximately 1 Hz:

```json
{
  "type": "leaderboard",
  "window_seconds": 600,
  "metric": "net_flow_sol",
  "as_of_ms": 0,
  "status": "live",
  "tokens": [
    {
      "mint": "example",
      "symbol": "TOKEN",
      "name": "Token Name",
      "image_url": null,
      "buy_lamports": "120000000000",
      "sell_lamports": "40000000000",
      "net_lamports": "80000000000",
      "trade_count": 123,
      "last_trade_ms": 0
    }
  ]
}
```

This is a conceptual payload, not a final wire format. Serialize large integer values losslessly (e.g., decimal strings). Reconnect clients should receive a current full snapshot; later consider deltas only if bandwidth warrants them.

## Repository layout (suggested)

```text
ballpump/
  AGENTS.md
  README.md
  .env.example
  backend/       # Rust service and tests
  frontend/      # React / TypeScript web app
  fixtures/      # Synthetic and recorded sanitized events
  docs/          # Decoder notes, coverage, operational decisions
```

## Correctness and tests

- Unit tests for rolling-window expiration at exact 600-second boundaries, out-of-order trades, duplicate events, simultaneous buys/sells, and negative net flow.
- Decoder fixture tests for both venues, buys and sells, swaps in inner instructions, failed/non-trade transactions, multiple swaps per transaction, and quote-token edge cases.
- Replay tests to prove idempotency and expected results after disconnect/restart.
- Load test with simulated bursts and document throughput, memory use, channel backpressure, and frontend frame rates.
- Validate observed decoded amounts against known transaction examples before trusting live metrics.
- Explicitly distinguish coverage gaps from a token having zero volume.

## Implementation approach for Codex

1. Read this file and inspect the repository before making changes.
2. Briefly propose the implementation plan, dependencies, and unresolved data semantics; verify current external APIs before coding.
3. Build and test the **trade event schema + rolling 600-second aggregator** first using fixtures.
4. Add program-specific decoders with golden transaction fixtures.
5. Integrate the streaming provider and implement recovery/observability.
6. Add WebSocket snapshots and then the interactive ball arena.
7. Run available formatting, linting, tests, and builds. State exactly what was tested and what still requires live credentials.
8. Keep changes modular and incremental; prefer simple working code over speculative infrastructure. When uncertain, document assumptions instead of fabricating behavior.

## Non-goals for MVP

- Kafka or a permanent chain-wide data warehouse.
- Automated trading, token recommendations, wallet custody, or signing transactions.
- Full trade coverage on Raydium, Meteora, Jupiter, or every DEX.
- Long-term historical charting and analytics dashboards.
- Guaranteed zero-loss indexing without validated replay and durability.

## Product voice

Project name: **BallPump**. Short description: **Watch memecoins pump in real time.**
