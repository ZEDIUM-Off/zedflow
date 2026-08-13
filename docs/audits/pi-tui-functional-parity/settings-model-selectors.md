# Settings, model selectors, and login

**Scope.** `/settings`, `/model`, `/scoped-models`, theme/config/thinking submenus, persistence, and login selector routing.

## Tests/source evidence

Pi selector components live under `packages/coding-agent/src/modes/interactive/components/`, driven by its settings manager and interactive callbacks. Rust counterparts exist under `crates/zedflow-coding-agent/src/modes/interactive/components/`. Source review from saved lane `e7adda82` found: Rust `scoped-models-selector.rs` turns Pi's `None` (all enabled) state into only the toggled model, whereas Pi starts from all IDs before toggling; `settings-selector.rs` receives arbitrary caller rows rather than Pi's complete ordered configuration inventory/actions; `theme-selector.rs` lacks automatic light/dark nesting and cancel-preview restoration; `config-selector.rs` lacks Pi grouped/subgrouped/filter/scoped tri-state UI.

`interactive-mode.rs` persists selected model and scoped IDs and has login method/provider overlay code. As documented in [auth-model-provenance](auth-model-provenance.md), login provider Enter has no login action. `tools/tui-fidelity` only supplies `settings.json` and `login.json`, both currently differ. The settings lane did not finish a focused Cargo test; no passing test is inferred from it.

## Matching behavior

Both have settings manager, model selector, scoped selector, theme schema/assets, configuration selector, thinking selector, OAuth selector, and persisted setting/model APIs. Rust can display and navigate a reduced overlay.

## Mismatches

- **P0 — login selection nonfunctional** after provider-list entry.
- **P1 — scoped models initial semantics** differ for `None`/all-enabled.
- **P2 — settings inventory and action callbacks** are incomplete; warning/thinking/theme menus and persistence behavior are not one-to-one.
- **P2 — theme selector** lacks auto nested selection and cancel restoration.
- **P2 — config selector** lacks grouped/subgrouped/filter/scoped tri-state behavior.
- **P3 — model filtering/catalogue ordering and selector style** lack a complete CLI differential assertion.

## Missing fidelity fixtures

Require real `/settings`, `/model`, `/scoped-models`, `/login`, and `/logout` PTY dispatch fixtures with deterministic HOME/settings. Cover ordered rows, selected/cancel behavior, all-enabled toggle, persistence/reopen, warning and thinking selections, custom/automatic theme preview/cancel, config tri-state/filter/group navigation, and no-secret login cancellation. Capture cells/cursor/raw escape output rather than accepting screenshots.

## Fix boundary

Repair selectors, settings-manager integration, and login state transitions only. Do not invent configuration rows or authenticate against live providers; the frozen Pi inventory/callback ordering is authoritative.
