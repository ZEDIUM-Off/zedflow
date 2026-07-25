# Port coordination decisions

- Stage 1 is a one-to-one port of the five frozen Pi packages; package/file exceptions require manifest disposition, and dependency substitutions require human arbitration.
- The controller is event-driven: accepted work immediately makes the next DAG unit eligible; no cron dispatches port work.
- Every unit has a fresh Pi session/worktree. Only one writer is active. Durable progress is external runtime state plus Git.
- The controller verifies exact base/result SHA, candidate cleanliness, frozen Pi gitlink, ownership, manifest/package gates, declared commands, and CAS before acceptance.
- Ordinary technical blockers use bounded repair/validation/review loops without DAG mutation. Structural replans use the approved `plan-writer` process and fresh IDs. Arbitration pauses.
- `ACCEPTING` is persisted before CAS and startup reconciles interrupted acceptance. Newly accepted worktrees are cleaned after durable evidence; historical cleanup is explicit and dry-run first.
- `docs/porting/BASELINE.md` is the sole current human status. `.agents/state/` and `docs/porting/pi-fidelity-decisions/` retain historical evidence only.
- The current recovery order is TUI closure → Coding-agent closure → Orchestrator → final Stage-1 gate. Recovery must migrate controller/DAG/runtime identities together before dispatch resumes.

- `NEXT-PORT-PLAN-V20` replaces the unaccepted downstream tail after the declared TUI recovery ancestry with fresh, deterministic manifest-gap batches: 59 TUI rows, 237 Coding-agent rows, then 13 Orchestrator rows, before package and final gates.
