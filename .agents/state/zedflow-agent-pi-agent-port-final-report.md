<!-- migration-document-status: SUPERSEDED / HISTORICAL -->
> [!CAUTION]
> **Migration status: SUPERSEDED / HISTORICAL.** Do not use this file as the current execution tracker. See `.agents/state/zedflow-ai-agent-pi-fidelity-current-status.md`.

# Zedflow Agent Pi Agent Port Final Report

Run: AV1 (`67a241f9-07fe-45c9-8f6b-f66e4745c675`), AV2 validation refresh after FX1/RV3
Date: 2026-07-10
Plan: `.agents/plans/zedflow-agent-pi-agent-port.md`

## Result

`zedflow-agent` remains ready for reviewer acceptance and the next global port wave after FX1. Source and test manifest rows are represented, final package gates pass, RV3 verified the three FX1 blocker fixes, and remaining parity gaps are explicit placeholders or ignored tests with exact reasons.

## FX1/RV3 refresh

FX1 fixed the three RV2 blockers:

- `Agent::wait_for_idle` now waits for the active run and async `agent_end` subscribers to settle.
- `before_provider_payload` hooks now chain returned payloads into later hooks and the provider call.
- Tool update callbacks now enqueue async update events instead of synchronously `block_on`ing the event sink.

RV3 reviewed those fixes in fresh context and found no blockers for the three requested FX1 items. The three related tests are now runnable, so the ignored-test count is reduced from 9 to 6.

## Manifest completion

| Manifest | Rows | Represented targets | Missing targets |
|---|---:|---:|---:|
| `.agents/port-manifests/agent-src.tsv` | 25 | 25 | 0 |
| `.agents/port-manifests/agent-tests.tsv` | 20 | 20 | 0 |

Audit command:

```bash
python3 - <<'PY'
from pathlib import Path
for name in ['agent-src.tsv','agent-tests.tsv']:
    p=Path('.agents/port-manifests')/name
    rows=[]; missing=[]
    for line in p.read_text().splitlines():
        if not line.strip(): continue
        src,target=line.split('\t')
        rows.append((src,target))
        if not Path(target).exists(): missing.append((src,target))
    print(f'{name}: rows={len(rows)} represented_targets={len(rows)-len(missing)} missing={len(missing)}')
PY
```

Output:

```text
agent-src.tsv: rows=25 represented_targets=25 missing=0
agent-tests.tsv: rows=20 represented_targets=20 missing=0
```

## Validation commands

All cargo commands used `CARGO_TARGET_DIR=/tmp/zedflow-agent-av2-target` and `TMPDIR=/tmp/zedflow-agent-av2-tmp`.

| Command | Result | Notes |
|---|---|---|
| `cargo fmt --all --check` | passed | no output |
| `cargo check -p zedflow-agent --all-targets` | passed | warnings only; existing `zedflow-ai` dead-code/function-pointer warnings plus `zedflow-agent` test unused-must-use/dead-code warnings |
| `cargo test -p zedflow-agent --all-targets --no-run` | passed | test executables built; warnings only |
| `cargo test -p zedflow-agent --test agent-loop --test agent --test harness --test prompt-templates --test skills --test system-prompt --test truncate --test resource-formatting --test nodejs-env --test utils --test agent-harness --test agent-harness-stream --test e2e` | passed | `115 passed, 6 ignored` |
| placeholder audit (`grep -R "PORT PLACEHOLDER" -n crates/zedflow-agent`) | passed | 3 documented source placeholders |
| ignored-test audit (`grep -R "#\[ignore" -n crates/zedflow-agent/tests`) | passed | 6 ignored tests, all with explicit reasons |
| manifest audit | passed | source rows `25/25`, test rows `20/20`; no missing targets |
| dependency manifest audit | passed | approved dependencies present: `zedflow-ai`, `ignore`, `yaml_serde`, `serde_json`, `jsonschema`, `uuid`, `wait-timeout` |
| `git diff --cached --name-only` | passed | no staged files |

No live/network/browser tests were run.

## Remaining `PORT PLACEHOLDER` markers

| File | Reason |
|---|---|
| `crates/zedflow-agent/src/harness/session/uuid.rs` | Pi `uuidv7` is time-ordered UUIDv7; this port follows the approved `uuid::Uuid::new_v4()` replacement unless UUIDv7 parity is later required. |
| `crates/zedflow-agent/src/harness/env/nodejs.rs` | `std::process` plus `wait-timeout` kills the spawned child only, not Pi's exact process-tree kill semantics. |
| `crates/zedflow-agent/src/proxy.rs` | Pi `streamProxy` HTTP fetch/SSE wrapper awaits an approved HTTP/runtime dependency; current Rust code ports the JSON event parsing seam. |

## Ignored tests

| File | Reason |
|---|---|
| `crates/zedflow-agent/tests/harness/session-uuid.rs` | source blocker: approved UUID v4 replacement does not implement Pi UUIDv7 layout/monotonic order. |
| `crates/zedflow-agent/tests/harness/storage.rs` | source blocker: `SessionStorage::set_leaf_id` is non-fallible, so invalid leaf ids cannot reject like Pi. |
| `crates/zedflow-agent/tests/harness/truncate.rs` | JS-only: Rust `str` cannot contain lone UTF-16 surrogate code units used by Pi Buffer edge cases. |
| `crates/zedflow-agent/tests/harness/compaction.rs` | live provider behavior excluded from AV1/AT4/AV2; deterministic fake providers cover compaction without network/model calls. |
| `crates/zedflow-agent/tests/e2e.rs` | source blocker: abort timing requires async token-paced faux streaming; current faux provider emits synchronously. |
| `crates/zedflow-agent/tests/scratch/simple.rs` | live scratch sample requires credentials, network, local `.pi` resources, and possible OAuth/browser login. |

## Dependency and docs audit

- Approved replacements are present in `crates/zedflow-agent/Cargo.toml`: `zedflow-ai`, `ignore`, `yaml_serde`, `serde_json`, `jsonschema`, `uuid`, and `wait-timeout`.
- Source uses `zedflow-ai` types directly instead of duplicating message/model/stream/tool primitives.
- `ignore`, `yaml_serde`, `serde_json`, `uuid`, and `wait_timeout` have direct package usage. `jsonschema` is present as the approved TypeBox-like validation dependency, but no current `zedflow-agent` source imports `jsonschema::`; current schemas are represented as `serde_json::Value`.
- No broad HTTP runtime or process supervisor dependency was introduced.
- Package/module docs and public item rustdocs are present for the ported public facade and placeholder blockers. The final gate did not run a `missing_docs` rustdoc lint because it is not part of this package gate.

## Residual risks

- The 3 source placeholders and 6 ignored tests above are the remaining known parity gaps.
- `cargo check -p zedflow-agent --all-targets` and the test gates emit warnings in dependency/test code but no errors.
- Workspace has extensive unrelated modified/untracked work from prior waves; AV2 preserved it.

## Next-wave recommendation

After reviewer acceptance, mark W3/P2 `packages/agent` source and tests complete in the global port tracker and continue with the next package wave: `packages/tui` source rows (`tui-src.tsv`). Keep the agent placeholders/ignored tests as tracked follow-ups, not blockers for starting the next package wave.
