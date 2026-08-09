# Stage-1 final-gate evidence

This is the controller acceptance snapshot used by `SEM-FINAL-V5-DOCS`.

- Integration ref: `refs/heads/automation/pi-port`
- Integration SHA: `a9a23c387f372ed027c5a742047f93d0689955ed`
- Frozen Pi gitlink: `2b00dade7cec918aefb025c8b7a4fa304a30acdd`

| Gate | Accepted SHA | Controller evidence |
|---|---|---|
| Workspace | `0b7206444c22b9f2d3ec7beebad4529ba9709962` | `cargo fmt --all --check`, `cargo check --workspace --all-targets`, `cargo test --workspace --all-targets`, and `python3 tools/pi-port-swarm/manifest.py check` returned 0 |
| Fidelity | `26fd6e77dca31fe7c3ca13c1e85dcbc7809b8894` | Independent review accepted; `cargo test -p zedflow-ai --test frozen-oracle` returned 0 |
| Rust quality | `26fd6e77dca31fe7c3ca13c1e85dcbc7809b8894` | Independent review accepted with no owned blocker |
| End user | `a9a23c387f372ed027c5a742047f93d0689955ed` | Independent review accepted with no owned blocker |

The gates did **not** accept one immutable SHA. The workspace gate predates fidelity and end-user repairs; the fidelity and Rust-quality gates predate end-user repairs. Consequently this snapshot does not satisfy the Stage-1 exit gate, does not authorize promotion, and does not authorize Stage 2. Workspace, fidelity, Rust-quality, and end-user acceptance must converge on the same integration SHA; after explicit promotion, every gate must pass again on `main`.
