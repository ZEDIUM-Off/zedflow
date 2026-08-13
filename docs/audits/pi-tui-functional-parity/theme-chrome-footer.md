# Theme, chrome, and footer

**Scope.** Built-in/custom/automatic theme resolution, root chrome, footer data/styling, terminal title, and terminal progress.

## Tests/source evidence

Exact `dark.json`, `light.json`, and `theme-schema.json` match at the audited heads. Pi `packages/coding-agent/src/modes/interactive/theme/theme.ts` supports registered/path themes, watch/reload behavior, and terminal color-mode/automatic selection. Rust `interactive-mode.rs::set_theme` loads built-in literal names and `InteractiveMode::with_terminal` initializes dark unconditionally. Pi `components/footer.ts` derives cumulative usage/cache/cost/OAuth subscription/context/reasoning and applies dim/error/warning styling. Rust `components/footer.rs` renders supplied `stats`, model/provider, and extension statuses but does not derive the Pi fields or styling. `zedflow-tui/src/terminal.rs` has title support; no interactive-mode call site was found. `terminal-progress` is persisted/catalogued but no OSC 9;4 emission was found.

Saved lane `efb77538` reports passing focused theme/footer/status Rust tests and real-PTY `login.json`/`settings.json` differences. It also observed Rust startup borders/footer versus Pi input-only frames at those checkpoints; that observation is a current frame difference, not a universal chrome specification.

## Matching behavior

Theme assets/schema, width truncation primitives, footer cwd/home formatting, token compact formatting, and sorted/sanitized extension status intent are present on both sides.

## Mismatches

- **P1 — theme resolution/lifecycle.** Custom registered/path themes, watcher reload, automatic light/dark selection, and initial auto mode do not match Pi.
- **P1 — footer fidelity.** Rust does not source Pi's cumulative stats/context/cost/OAuth/reasoning state or its ANSI dim/warning/error styling; it starts `no-model`.
- **P2 — terminal chrome hooks.** Title and OSC 9;4 progress have no discovered interactive integration.
- **P2 — root frame composition differs** in current real-PTY login/settings captures.
- **P3 — exact custom-theme error/cancel/reload behavior is untested.**

## Missing fidelity fixtures

Raw-PTY fixtures need fixed dark/light/custom/path/automatic environments, resize-narrow footer/context thresholds, title bytes, progress bytes, reload/watch event handling, and selector preview/cancel restoration. They must invoke real CLI settings/theme dispatch and compare decoded cells plus raw OSC output.

## Fix boundary

Repair theme controller/config resolution, interactive chrome/footer data wiring, and terminal integration only. Keep the exact JSON assets untouched unless a frozen-source comparison finds a real asset mismatch; do not add a new theme format.
