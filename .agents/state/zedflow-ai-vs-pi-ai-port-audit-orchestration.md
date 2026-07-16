<!-- migration-document-status: SUPERSEDED / HISTORICAL -->
> [!CAUTION]
> **Migration status: SUPERSEDED / HISTORICAL.** Do not use this file as the current execution tracker. See `.agents/state/zedflow-ai-agent-pi-fidelity-current-status.md`.

# Zedflow AI vs Pi-AI Port Audit Orchestration

Goal: analyze drift across the full `references/pi/packages/ai` -> `crates/zedflow-ai` port before planning fixes.

## Runs

| Area | Status | Run ID | Output |
|---|---|---|---|
| Provider/model registry/auth | running | d91137ba / child 0 | `.agents/state/port-audit-provider-model-auth.md` |
| API transports/request builders | running | d91137ba / child 1 | `.agents/state/port-audit-api-transports.md` |
| Stream/events/result aggregation | running | d91137ba / child 2 | `.agents/state/port-audit-stream-events.md` |
| Tests/parity matrix/residual ignores | running | d91137ba / child 3 | `.agents/state/port-audit-tests-residuals.md` |
| Public API/types/errors | running | d91137ba / child 4 | `.agents/state/port-audit-public-api-types.md` |
| Compat/faux/usage/cache/session | running | d91137ba / child 5 | `.agents/state/port-audit-compat-faux-accounting.md` |

## Notes

Read-only audit. Do not modify code.
