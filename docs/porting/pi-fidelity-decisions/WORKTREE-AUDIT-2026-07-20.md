# Worktree audit — 2026-07-20

The clean crate-based `main` baseline was compared with all registered port worktrees, archive refs, recovery snapshots, and the frozen Pi source at `references/pi@2b00dade7cec918aefb025c8b7a4fa304a30acdd`.

## Accepted recovery

`2fb02850 fix(agent): preserve JSONL leaf append context` identified one missing behavior: `set_leaf_id` must report `Failed to append session leaf <id>` while `append_entry` retains its general entry context. It is represented by `AG-R1-JSONL-LEAF-ERROR`, rather than cherry-picked, so the V2 controller can validate it from the current baseline.

## Rejected recovery

- `c0bb6779`: UUIDv4 and ignored UUIDv7 test regression.
- `5120fc16`: stash snapshot, not an integration candidate.
- `a4b2e7fb`, `14b8693a`, `8a05ca4e`, `a325f242`: broad older snapshots with superseded agent changes.
- V1 `swarm.py`: incompatible role/model/state contract and scheduled launcher.

The audit does not reopen closed units. The repair is a new evidence-backed prerequisite for the remaining independent agent units.

## AG-P2 review repair

Independent fidelity and Rust reviews of `df523c8eb62cfd62012610b853908ef8a8ab7698` found that timeout coverage omitted grandchildren and active abort, while the implementation used a synchronous wait and PATH-resolved `kill`. `AG-P2-R1-PROCESS-TREE` was inserted before `AG-T1`; later work must preserve the accepted AG-P2 evidence while closing these review blockers.
