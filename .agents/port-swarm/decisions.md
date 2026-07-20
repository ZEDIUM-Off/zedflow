# Port coordination decisions

- The V2 controller is event-driven: accepted work immediately makes the next DAG unit eligible; no cron dispatches port work.
- Every unit has a fresh Pi session and worktree. Durable progress is external runtime state plus Git, not a growing model conversation.
- Only one writer is active. The controller validates exact base/result SHA, candidate-worktree HEAD/cleanliness, frozen Pi gitlink/submodule state, ownership, declared commands, and CAS before acceptance.
- Workers commit only their owned unit. Only a structured, evidence-backed `PLAN_CHANGE` can invoke a fresh coordinator to mutate open DAG/state/docs control paths.
- Context is bounded: workers receive a compact assignment capsule; reviews are fresh and wave-scoped; the coordinator is invoked only for plan mutation.
- `ACCEPTING` is persisted before CAS; startup reconciles interrupted acceptance and requires explicit `retry --unit` for failures. A monitoring timer may call the deterministic read-only `monitor` command. It must never run, repair, or mutate the port.
- `AG-R1-JSONL-LEAF-ERROR` is an audit-discovered prerequisite before the remaining AG-P units.
