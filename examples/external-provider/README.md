# Synthetic external-provider example

This Python 3 example illustrates the [draft protocol](../../docs/provider-development.md). It uses only the standard library, no credentials and no network. **Permesh does not execute it or accept an external provider configuration.** Record payloads are illustrative draft wire objects, not the current Rust snapshot schema.

Run it deliberately in a terminal with `python3 examples/external-provider/provider.py`, then send one JSON object per line:

```json
{"protocol":1,"id":"hello","method":"handshake"}
{"protocol":1,"id":"health","method":"check"}
{"protocol":1,"id":"snapshot","method":"discover"}
{"protocol":1,"id":"stop","method":"cancel"}
```

The peer writes a handshake, health, three synthetic records and completion, then cancellation acknowledgment. It rejects oversized or malformed frames without reflecting their contents. The example is synchronous; it demonstrates framing and orderly cancellation between requests, not interruption of a blocked discovery operation or host process supervision.

Run its tests with `python3 -m unittest discover -s examples/external-provider -p 'test_*.py'`. These tests do not qualify a future runtime's trust or sandbox behavior.
