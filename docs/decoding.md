# Decoder evidence

The decoder consumes the actual LaserStream protobuf types re-exported by the pinned SDK. Fixture tests construct those same types from recorded RPC transactions, then exercise production normalization and decoding. Both static and lookup-table account addresses, instruction ordering, inner stack heights, and invocation outcomes participate.

Successful transaction metadata proves top-level calls succeeded. Inner trade/event calls additionally require successful invocation outcomes for themselves and every inner ancestor. If a router catches a failed trade CPI, its emitted events are excluded. Monetary values come from Borsh CPI-event data, not logs. A recognized event must be directly emitted by a matching trade invocation of the same program, at the next stack depth. Pump event mints and directions and AMM event pools are checked against that invocation's accounts. Unknown-parent trade events increment failures instead of being silently accepted.

## Recorded examples

Public, finalized mainnet RPC responses were fetched on 2026-10-23 from `https://api.mainnet-beta.solana.com`, using `getSignaturesForAddress` followed by `getTransaction` with JSON encoding and `maxSupportedTransactionVersion: 1`. They contain only public chain data; no keys or account credentials. Full signatures, slots and timestamps are preserved in `fixtures/manifest.json` and each RPC response. No test requires network access.

| Fixture | Result | Expected fee-exclusive SOL volume, lamports |
| --- | --- | ---: |
| `observed-pump-0` | Sell | 58,226,455 |
| `observed-pump-sol-0` | Buy | 111,288,166 |
| `observed-pump-sol-1` | Buy | 110,894,027 |
| `observed-amm-sol-0` | Sell | 13,316,584 |
| `observed-amm-sol-1` | Exact-input buy | 12,954,823 |
| `observed-amm-sol-2` | Exact-input buy | 56,147,779 |
| `observed-amm-sol-3` | Exact-input buy | 14,173 |
| `observed-pump-1`, `observed-pump-2` | Routed non-SOL quotes | Excluded |
| `observed-amm-0`, `observed-amm-1` | Non-SOL quotes | Excluded |
| `observed-amm-2` | Program mentioned; no trade executed | No event |

The Pump sell's bonding-curve balance falls by exactly 58,226,455 lamports. For the four SOL PumpSwap examples, the WSOL vault delta reconciles with the decoded volume plus LP fee for buys, minus LP fee for sells. These are assertions in the integration tests, independent of the generated event structs.

For example, `observed-amm-sol-1` records a 12,993,689-lamport input budget. Its actual WSOL vault credit is 12,980,733 and LP fee is 25,910. The indexed amount is therefore 12,954,823, not the budget. The corresponding event field names alone are insufficient to establish semantics.

## Synthetic coverage

Mutated observed transactions test whole-transaction failure, missing parents, truncated event data, missing invocation completion, multiple swaps under one signature, and replay into a fresh aggregator. A dedicated invocation-trace fixture tests an event emitted inside a CPI that fails while its outer router succeeds.

Schema-derived Borsh fixtures test multi-hop pool buys, SOL curve legs, boost buy-and-burn actual-used amounts, and the additional synthetic post-completion buy event. They establish parsing and context handling; they do **not** independently validate those paths' economic semantics. Add observed transactions and balance reconciliation for these paths before claiming full validated protocol coverage.

## Partial invocation logs

The paired `observed-stream-truncated` and `observed-stream-complete` fixtures refer to the same public transaction at slot 460550433. The first was captured from LaserStream while reproducing the reported warning; the second was fetched from public mainnet RPC. The stream's 145-line log ends with `Log truncated` during a later token-program call; RPC returns 150 lines. Completed trade branches precede that tail. Tests assert identical decoded trade results and quote exclusions for both payloads.

The validator uses the instruction metadata to establish ancestry. It reads the unambiguous log prefix, retains known outcomes, and stops on truncation or a mismatch. It does not assume that an unresolved CPI succeeded. A completed event beneath an unresolved inner parent is excluded because the parent might subsequently fail. Conversely, an absent top-level success log does not erase the transaction metadata's proof of outer success. Program-emitted `Program log:` text cannot impersonate runtime invocation messages.

`partial_log_transactions` counts partial diagnostics without classifying the transaction envelope as invalid. `unverified_trade_calls` counts uncertain SOL-eligible trade calls; these still mark coverage degraded. Known non-SOL calls cannot change the SOL rankings and do not cause a coverage alarm merely because their diagnostic logs are missing. Malformed instruction ancestry and missing transaction metadata remain hard validation errors.

The official IDLs can lag the deployed program: the captured events carry an additional trailing 8 bytes. The parser reads the complete known schema prefix and leaves trailing fields uninterpreted. It rejects shorter schemas. Upgrade the pinned schemas deliberately and rerun golden fixtures when protocol changes occur.
