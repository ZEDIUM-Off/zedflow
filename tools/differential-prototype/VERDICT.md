# Prototype verdict

## Answer

Use one Python executable per observable scope (`batch`, `rpc`, later `tui`) over a deliberately small shared layer:

- archive and verify the frozen Pi SHA;
- create isolated agent/session roots;
- provide a deterministic local model endpoint;
- launch a real CLI;
- retain raw outputs, provider request bodies, commands, exit status, and persistent-file hashes;
- compare channels exactly and exit nonzero on any difference.

Recorded Pi sessions are sandbox seeds, not saved verdicts. Fresh execution of the frozen Pi CLI remains the oracle. Provider tapes are independent inputs to the local replay.

## What the vertical slice proved

Both scope tools reused the same primitives without importing PTY or TUI concepts. The frozen Pi batch and RPC runs selected the synthetic model, reached the replay server, returned `deterministic reply`, and wrote a session. Exact comparison retained and reported every mismatch.

At Zedflow `913488c2022a6ed595e65d19394953d566d4edbc`, both probes exposed the same real red gap: the CLI did not dispatch the configured `fixture/fixture-model` provider. Batch exited with `Error: dispatch failure`; RPC fell back to `amazon-bedrock/amazon.nova-2-lite-v1:0`, emitted a dispatch error, and made no replay request. No comparator rule hid the difference.

First-run local cost was dominated by the existing Rust build (about 75 seconds); frozen offline `npm ci` took about 3 seconds. Subsequent Rust builds were effectively cached. This is enough evidence to decide CI caching separately.

## Deliberately absent

- no production framework or generalized fixture schema;
- no TUI driver;
- no normalization or blessing mechanism;
- no committed credentials or captured authorization headers;
- no migration of `tools/tui-parity` or `tools/tui-fidelity`.
