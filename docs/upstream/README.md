# Verified sources

Retrieved 2026-10-23 from Pump's official repository. Main commit at retrieval: `639b87fb45c3adeffa6f7c4a1bf0cb85e392bf22`.

- [Pump IDL](https://github.com/pump-fun/pump-public-docs/blob/639b87fb45c3adeffa6f7c4a1bf0cb85e392bf22/idl/pump.json), SHA-256 `7616681ea557975f9f623ac211080f22a93060781eaf4765b0a488d980997207`
- [PumpSwap IDL](https://github.com/pump-fun/pump-public-docs/blob/639b87fb45c3adeffa6f7c4a1bf0cb85e392bf22/idl/pump_amm.json), SHA-256 `b7d8c57a4d9c4dd0109a9ab893052352333252d4eedced10ac091d4d66cab89b`
- [Multi-hop instruction and per-hop events](https://github.com/pump-fun/pump-public-docs/blob/639b87fb45c3adeffa6f7c4a1bf0cb85e392bf22/docs/instructions/MULTI_HOP_SWAP.md)
- [Pump sell and SOL quote convention](https://github.com/pump-fun/pump-public-docs/blob/639b87fb45c3adeffa6f7c4a1bf0cb85e392bf22/docs/instructions/SELL.md)
- [Anchor event CPI format](https://www.anchor-lang.com/docs/features/events)
- [LaserStream Rust SDK source](https://github.com/helius-labs/laserstream/tree/main/rust), crate pinned to `helius-laserstream = 1.0.1` in Cargo.lock
- [gRPC request/filter reference](https://www.helius.dev/docs/laserstream/grpc)
- [Historical replay](https://www.helius.dev/docs/laserstream/historical-replay)
- [Delivery guarantees and finality](https://www.helius.dev/docs/laserstream/delivery-guarantees)

Program addresses are read directly from these IDLs and match the project's supplied addresses. The checked-in schema generator derives trade instruction discriminators, account positions, event discriminators and Borsh structures from these files. The decoder verifies execution context rather than parsing arbitrary `Program data` text.

Observed events have additional trailing bytes beyond these published schemas. Known fields are Borsh-decoded as a prefix; unknown trailing extensions are ignored. Known-field truncation or invalid booleans/lengths is rejected. Balance-change fixture tests validate the monetary fields independently. Do not infer that ignoring trailing fields supports arbitrary incompatible protocol changes.
