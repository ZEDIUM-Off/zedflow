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
- On 2026-07-27 the user approved TUI dependency arbitration proposal A: exact pins `markdown = 1.0.0`, `icu_properties = 2.2.0`, `icu_segmenter = 2.2.0`, and `emojis = 0.9.0`. Rust must retain Pi's terminal Markdown renderer and composite grapheme-width policy; `unicode-width` and Unicode-16 `east-asian-width` are not direct substitutes. Acceptance requires differential coverage against frozen Pi Markdown tests and Unicode 17 EAW/segmentation/emoji regressions.
- On 2026-07-27 the user invalidated the completed mechanical DAG as a Stage-1 completion claim: file presence, marker-only modules, empty test targets, dead modules, ignored implementation gaps, and a minimal terminal smoke loop are not a faithful port. `automation/pi-port@f83a96fe` is evidence only; promotion and Stage 2 are forbidden until semantic closure and end-user TUI/CLI gates pass.
- `--no-session` is only a smoke-test isolation flag that prevents test conversations from being persisted; it is not required for normal use and does not disable tools.
