# Offline fixtures

`*.rpc.json` are public finalized transaction responses recorded from mainnet RPC on 2026-10-23. They contain no secrets. `manifest.json` lists expected SOL trades and quote exclusions; response files retain complete public signatures and slots for independent investigation.

`backend/tests/decoder.rs` converts them into LaserStream protobuf types and invokes the production normalizer and decoder. The tests also reconcile executed volume with curve/vault balance changes and mutate fixtures to cover failure and replay cases. Test-time mutations and schema-built events are synthetic, not extra recorded transactions.

The standalone frontend demo and backend demo are intentionally synthetic. Demo mints/symbols must not be interpreted as real token observations or used as token links.

`observed-stream-truncated.rpc.json` is a sanitized public transaction exported from LaserStream while reproducing a log warning at slot 460550433; it contains instruction metadata and logs, without balance arrays. `observed-stream-complete.rpc.json` is the same transaction fetched through public RPC with complete logs. They test that clipping an unrelated trailing call does not invalidate earlier verified trades or quote exclusions.
