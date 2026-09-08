# Synthetic external-provider example

This Python 3 peer implements the [draft protocol](../../docs/provider-protocol.md)
using only the standard library, synthetic data and no network or credentials.
**Permesh does not execute it or accept an external provider configuration.**
Its normalized records can now be validated by the Rust protocol crate.

Run the peer deliberately with `python3 examples/external-provider/provider.py`,
then send these requests and close stdin (EOF):

```json
{"protocol":1,"id":"handshake","method":"handshake","instance":"example-main"}
{"protocol":1,"id":"discover","method":"discover"}
```

The output is the checked-in [discovery transcript](discovery.ndjson): a
handshake, six records and completion. Alice has read access to a synthetic
repository through the Backend group. The Rust tests pass that snapshot through
the ordinary core identity query and assert the preserved membership path.
Every identity, email and resource here is fictional.

From the repository root, validate the finite fixture on Unix:

```sh
cargo run --locked -q -p permesh-provider-protocol --example validate -- synthetic-example example-main < examples/external-provider/discovery.ndjson
```

The developer validator prints JSON counts only; it does not print identities or
access records. Exit 0 means valid (possibly explicitly incomplete) input, exit 2
means invalid input/arguments, and exit 5 means output failure. This summary is a
developer-tool output, separate from Permesh CLI output schema 1.

For a cross-platform check that deliberately runs the known Python peer, validates
its actual output in Rust and rejects a duplicate-key exchange:

```sh
python scripts/check_protocol_example.py
```

Use `python3` if that is your Python command. The helper works on Windows without
shell input-redirection syntax. CI runs it on Linux, macOS and Windows.

The peer also supports one `check` request and a `cancel` acknowledgment for
manual exploration. It is synchronous: cancellation works between requests,
not during blocked discovery. The offline validator consumes stdin until EOF;
it provides no live-process deadlines. No trust registration, sandbox or
process-tree supervision is implemented or qualified by these tests.

```sh
python -m unittest discover -s examples/external-provider -p 'test_*.py'
cargo test --locked -p permesh-provider-protocol
```
