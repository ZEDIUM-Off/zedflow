# Stage-1 final-gate evidence

This snapshot records the controller-accepted replacement final gate. It supersedes the prior split-SHA table and its invalid TUI/end-user completion evidence.

- Tested product SHA: `7ae6374063da1f60cc5767d0a4e51d907cfc61d6`
- Accepted unit: `P8.T1-V1`
- Controller acceptance time: `1786459338`
- Frozen Pi gitlink: `2b00dade7cec918aefb025c8b7a4fa304a30acdd`
- Evidence/docs commit: the later descendant commit containing this file; it was **not** product-tested and is not a substitute for the product SHA above.

The controller records are under `~/.local/state/zedflow-pi-port/sessions/p8.t1-v1-1-0c933c3959a2/validation/`:

| Record | Command | Return code |
|---|---|---:|
| `00.json` | `cargo fmt --all --check` | 0 |
| `01.json` | `cargo check --workspace --all-targets` | 0 |
| `02.json` | `cargo test --workspace --all-targets` | 0 |
| `03.json` | `python3 tools/pi-port-swarm/manifest.py check` | 0 |
| `04.json` | `python3 tools/pi-port-swarm/controller.py validate` | 0 |

The fresh read-only validator returned `DONE` for the exact product SHA, and the controller recorded `P8.T1-V1` as `ACCEPTED`. The validator emitted no separate durable differential-oracle, manual-PTY, or three-review artifact paths; this snapshot does not invent them. The executed workspace suite and controller validation are the exact accepted replacement evidence.

The earlier acceptances on `0b7206444c22b9f2d3ec7beebad4529ba9709962`, `26fd6e77dca31fe7c3ca13c1e85dcbc7809b8894`, and `a9a23c387f372ed027c5a742047f93d0689955ed` do not establish current TUI/end-user completion and are explicitly superseded.

**Human approval is requested to promote tested product SHA `7ae6374063da1f60cc5767d0a4e51d907cfc61d6` to `main`.** This evidence commit does not authorize promotion by itself. After explicit promotion, a fresh read-only validator must repeat every final gate on promoted `main`. Stage 2 remains forbidden until that succeeds.
