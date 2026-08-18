# Real differential harness

Each executable crosses one real observable Pi/Zedflow boundary and writes immutable-by-content evidence. It does not repair or normalize product behavior.

```bash
python3 tools/differential-harness/common.py
python3 tools/differential-harness/batch.py --artifacts /tmp/zedflow-batch
python3 tools/differential-harness/rpc.py --artifacts /tmp/zedflow-rpc
python3 tools/differential-harness/tui.py --artifacts /tmp/zedflow-tui
python3 tools/differential-harness/confirmed_divergences.py
```

- `batch.py` launches each real CLI in `--print` mode.
- `rpc.py` exchanges JSONL with each real `--mode rpc` process.
- `tui.py` launches each real default TUI in a 24×80 PTY and retains raw terminal bytes.
- `confirmed_divergences.py` reruns the revision-locked source probes for confirmed red findings; it exits nonzero if a probe no longer matches frozen Pi or the `e91b44be9c897aef63c84c34b4e14b387a8141a7` baseline. It is not passing differential evidence.

Every target directory contains the exact argv, cwd, selected environment, stdin, stdout, stderr, exit status, provider request bodies, persistent-state hashes, frozen Pi SHA, tested Zedflow SHA, and SHA-256 for every artifact in `manifest.json`. The run-level manifest binds both target manifests and `verdict.json`.

The local replay checks its dummy authorization header but evidence never stores headers or credential values. Frozen Pi is archived at `2b00dade7cec918aefb025c8b7a4fa304a30acdd` and installed offline from its lockfile. Existing artifact directories are rejected. Any byte difference remains red and exits nonzero; there is no blessing path.
