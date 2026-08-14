# PROTOTYPE — differential CLI scopes

Throwaway asset for [Prototyper le harnais agentique différentiel réel](https://github.com/ZEDIUM-Off/zedflow/issues/4).

It tests one architectural decision: keep one executable per observable scope while sharing only frozen-Pi preparation, isolated sandboxes, a deterministic local OpenAI-compatible replay, evidence collection, persistence manifests, and exact comparison.

```bash
python3 tools/differential-prototype/common.py
python3 tools/differential-prototype/batch.py --artifacts /tmp/zedflow-batch-proof
python3 tools/differential-prototype/rpc.py --artifacts /tmp/zedflow-rpc-proof
```

`batch.py` and `rpc.py` both launch the frozen Pi CLI and the Rust CLI with the same synthetic model and prompt. They retain stdout, stderr, exit status, provider request bodies, session manifests, commands, and a verdict. A difference exits nonzero and remains visible; nothing is normalized or blessed.

The replay verifies the dummy authorization header but never records headers. Recorded Pi sessions can later seed each sandbox independently of provider request/response tapes.

This branch is evidence, not production code. The planned TUI scope should consume the same narrow primitives while keeping its PTY/cell decoder in its own tool.
