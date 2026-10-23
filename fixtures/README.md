# Offline fixtures

`*.rpc.json` are public finalized transaction responses recorded from mainnet RPC on 2026-10-23. They contain no secrets. `manifest.json` lists expected SOL trades and quote exclusions; response files retain complete public signatures and slots for independent investigation.

`backend/tests/decoder.rs` converts them into LaserStream protobuf types and invokes the production normalizer and decoder. The tests also reconcile executed volume with curve/vault balance changes and mutate fixtures to cover failure and replay cases. Test-time mutations and schema-built events are synthetic, not extra recorded transactions.

The standalone frontend demo and backend demo are intentionally synthetic. Demo mints/symbols must not be interpreted as real token observations or used as token links.
