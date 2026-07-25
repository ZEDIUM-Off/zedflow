<!-- migration-document-status: SUPERSEDED / HISTORICAL -->

> Historical AI/Agent snapshot. Current authority is `docs/porting/BASELINE.md` plus controller/manifest status.

# Zedflow AI/Agent Pi fidelity — current status

**Status:** HISTORICAL AI/AGENT SNAPSHOT
**Audited:** 2026-07-13
**Historical plan:** `.agents/plans/zedflow-ai-agent-pi-fidelity-consolidation.md`
**Historical tracker:** `.agents/state/zedflow-ai-agent-pi-fidelity-consolidation-orchestration.md`

## Source-of-truth order

When documents disagree, use this order:

1. Current worktree code and runnable checks.
2. This status snapshot for unit progress.
3. The active consolidation plan for requirements and execution order.
4. The active orchestration tracker for run history.
5. Frozen baselines and supporting audits.
6. Superseded reports and raw subagent outputs only as historical evidence.

A historical `passed`, `complete`, or `ready` verdict does not override the current worktree.

## Verified worktree snapshot

- Git HEAD: `c293eac3 Initial Zedflow repository`.
- Worktree at audit time: 147 modified files and 58 untracked files; most migration work is not committed.
- Manifest presence: AI source 148/148, AI tests 98/98, Agent source 25/25, Agent tests 20/20.
- Missing AI test targets: 0.
- AI-M1 deterministic gate: 848 passed, 0 failed, 51 ignored; ignores classify as 46 live-capability, 3 JS-only, 2 upstream-skipped, 0 deterministic implementation-gap.
- `cargo test -p zedflow-ai --test faux-provider --test abort`: 26 passed, 0 ignored.
- `cargo check -p zedflow-agent --all-targets`: failed with the planned fallible-provider-hook propagation mismatch at `crates/zedflow-agent/src/harness/agent-harness.rs:1222` and `:1234` (four emitted `E0271` errors across targets).

## Unit status

| Wave | Unit | Current status | Evidence / next gate |
|---|---|---|---|
| W0 | F0 | complete — frozen baseline | `.agents/state/zedflow-ai-agent-full-fidelity-test-ledger.md`; counts are the 2026-07-10 baseline, not live counts. |
| W0 | D0 | complete | Required dependencies/features are present in both crate manifests and `Cargo.lock`. |
| W1 | AI-C1 | implemented; targeted gate passed | Fallible `PayloadHook`/`ResponseHook` and `ProviderHookError` exist in `crates/zedflow-ai/src/types.rs`; targeted tests are recorded in AI-V1. |
| W1 | AI-C2 | implemented; targeted gate passed | Models/auth/catalog/dispatch changes and targeted tests are recorded in AI-V1. |
| W1 | AI-C3 | complete | 26 targeted faux/abort tests pass with 0 owned ignores after three review/fix rounds; canonical typed async factory, pacing, abort, terminal, bounds, usage, cache/session, and registration behavior is active. |
| W1 | AI-C4 | complete | Duplicate lazy types/conversions/materialized stream were removed; compat and nine chat lazy entrypoints now use canonical types/shared EventStream. 43 focused tests/checks passed and independent review returned PASS. |
| W2 | AI-P1 | complete | Registered canonical Anthropic HTTP/SSE is immediate and incremental; deterministic abort is race-safe; 3 manifest targets added; 61 integration tests pass with 5 capability-only live ignores; final review PASS. |
| W2 | AI-P2 | complete | Copilot Anthropic reuses AI-P1 transport with exact Bearer/identity/initiator/intent/beta headers and mixed-API dispatch; 3 deterministic tests pass, 0 ignores; review PASS. |
| W2 | AI-P3 | complete | Codex SSE production requests use exact Zstd level 3 with correct header/fallback; captured bodies decompress to exact JSON; 27 deterministic tests pass; review PASS. |
| W2 | AI-P4 | complete | Bedrock canonical errors preserve structured/non-JSON status/body/message/metadata/source with no public genai leak; 40 deterministic tests pass; review PASS. |
| W2 | AI-P5 | complete | Google Generative registered REST/SSE uses exact wire shape, incremental progressive events, race-safe abort, normalized errors, model costs, and key precedence; 4 targets added; 41 deterministic tests pass; review PASS. |
| W2 | AI-P6 | complete | Vertex API-key/full standard ADC endpoints, hooks, incremental stream, abort/error/responseId and service-account token capture pass 26 deterministic tests; 1 target added; review PASS. |
| W2 | AI-P7 | complete | Mistral registered request/SSE, reasoning modes, strict schemas, responseId/usage/abort and exact camelCase-hook-to-snake_case wire mapping pass 20 deterministic tests; 2 targets added; review PASS. |
| W2 | AI-P8 | complete | OpenAI Completions registered transport is incremental/abort-safe, settles on `[DONE]` before EOF, preserves partial state/reasoning/cost/errors, passes 76 deterministic tests, closes the first broad-gate hang, and adds 5 targets; review PASS. |
| W2 | AI-P9 | complete | OpenAI Responses registered transport is incremental and terminal-safe; Copilot/IDs/partial JSON/transforms pass 67 deterministic tests with 4 live ignores; second broad-gate hang closed; 5 targets added; review PASS. |
| W2 | AI-P10 | complete | Azure endpoint/deployment/version/api-key/body/incremental Responses/retries/abort/errors/responseId pass 19 deterministic tests; Responses regressions pass with 6 live ignores; review PASS. |
| W2 | AI-P11 | complete | Image/OpenRouter catalog order, auth/env, hooks, abort and production serializer/parser pass 16 deterministic tests; 1 target added; parent verification passed. |
| W3 | AI-U1–AI-U8 | complete | Compat env, error body, Fireworks, lax content, deterministic OAuth device-code timing, overflow, retry, and validation targets pass. |
| W3 | AI-M1 | complete | 98/98 target equality; 848 deterministic tests pass; all 51 ignores dispositioned with zero deterministic gaps in `.agents/state/zedflow-ai-full-fidelity-validation.md`. |
| W4 | R-AI | complete | fmt/check/doc and no-deps clippy `-D warnings` pass; 844 deterministic tests pass with 51 dispositioned ignores. Evidence: `.agents/state/zedflow-ai-rust-cleanup.md`. |
| W4 | AI-V1 | **GO — complete** | Final gates were rerun after matching Pi's single-pass OpenAI/Azure Responses processing: 845 deterministic tests pass, 51 ignores are fully dispositioned, both historical hangs are closed, and Agent exposes only the two planned AG-H2 mismatches. |
| W5 | AG-C1 | **ready — next** | Final AI-V1 authorizes Agent work on the frozen canonical AI boundary. |
| W5 | AG-C2 | not started / blocked by AG-C1 | Agent event and policy contracts follow AG-C1. |
| W6 | AG-L1, AG-L2 | not started / blocked by W5 | Existing `block_on` and lifecycle gaps remain unchanged. |
| W7 | AG-H1–AG-H4 | not started / blocked by W6 | Persistence, hooks, compaction, and wait lifecycle remain unchanged. |
| W8 | AG-P1–AG-P4, AG-T1 | not started / blocked by W7 | Agent placeholder and test closure remains unchanged. |
| W9 | R-AG, V1, RV-FID, RV-RUST, V2 | not started / blocked by W8 | Final Agent cleanup, cross-crate validation, independent reviews, and synthesis. |

## Resume point

1. Start AG-C1 on the frozen AI boundary.
2. Keep the two current Agent hook-result mismatches assigned to the planned Agent propagation work; do not reopen AI.

The prior AI-V1 NO-GO remains historical diagnostic evidence for AI-P8/P9; the final gate now records GO.

## Document classification

### Active / canonical

- `plans/zedflow-ai-agent-pi-fidelity-consolidation.md` — active requirements and execution order.
- `state/zedflow-ai-agent-pi-fidelity-current-status.md` — current unit status.
- `state/zedflow-ai-agent-pi-fidelity-consolidation-orchestration.md` — active run history/tracker.
- `state/zedflow-ai-agent-prerequisite-gate.md` — retained prior NO-GO evidence; AI-V1 updates it as the final W4 AI gate.

### Frozen supporting baselines

- `state/zedflow-ai-agent-full-fidelity-test-ledger.md`
- `state/zedflow-agent-consolidation-audit.md`
- `state/zedflow-ai-pi-ai-runtime-drift-final-report.md`

Their observations remain inputs to the active plan, but their counts and verdicts are not current execution status.

### Superseded plans

- `plans/pi-to-rust-package-port.md`
- `plans/zedflow-agent-pi-agent-port.md`
- `plans/zedflow-ai-pi-ai-parity-finalization.md`
- `plans/zedflow-ai-pi-ai-runtime-drift-fixes.md`
- `plans/zedflow-ai-placeholder-deps-replacement.md`

### Historical/completed state artifacts

Every other Markdown file under `.agents/state/` is historical evidence or a completed artifact from an earlier wave. It must not be used to determine current progress unless the active plan explicitly cites it as a frozen baseline.
