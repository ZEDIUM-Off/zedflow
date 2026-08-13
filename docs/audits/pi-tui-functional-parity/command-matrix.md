# Frozen Pi built-in slash-command matrix

Inventory source: `references/pi/packages/coding-agent/src/core/slash-commands.ts` at `2b00dade`. Rust mapping source: `crates/zedflow-coding-agent/src/core/slash-commands.rs` and `src/modes/interactive/interactive-mode.rs` at `913488c2`. “Partial” means a source-proven visible/semantic gap; “Unproven” means no whole-CLI differential fixture, not a claim of failure.

| Pi command | Frozen Pi behavior | Rust mapping | Status / disposition |
|---|---|---|---|
| `/settings` | Settings selector/callbacks | `Settings` → `SettingsSelector` | Partial P2: inventory/actions/theme subflows differ; PTY differs |
| `/model [query]` | Model selector/filter | `Model(search)` → `ModelSelector` | Unproven P3: selector/catalogue whole-CLI parity |
| `/scoped-models` | Enabled-model selector | `ScopedModels` → `ScopedModelsSelector` | Partial P1: `None` all-enabled toggle differs |
| `/export [path]` | First token, outer quotes stripped | `Export(PathBuf)` | Partial P2: argument parsing differs |
| `/import [path]` | First token, outer quotes stripped, confirm | `Import(PathBuf)` → confirm overlay | Partial P2: argument parsing differs |
| `/share` | Export HTML, `gh` auth/secret gist/cancel/cleanup/URL | `Share` → status text | Missing P1 placeholder |
| `/copy` | Copy latest assistant message | `Copy` → `copy_last_assistant` | Unproven P3 |
| `/name [name]` | Set normalized name; query current/usage | `Name` → `name_session` | Partial P2: user-visible semantics differ |
| `/session` | Transcript session-info content | `Session` → status | Partial P2 placeholder |
| `/changelog` | Transcript changelog content | `Changelog` → status | Partial P2 placeholder |
| `/hotkeys` | Transcript hotkey content | `Hotkeys` → status | Partial P2 placeholder |
| `/fork` | Select prior user message; empty guard | `Fork` → message selector | Partial P2: empty/flow behavior differs |
| `/clone` | Clone current session | `Clone` → `clone_active_session` | Unproven P3; focused tests pass |
| `/tree` | Tree selector/navigation with guards/branch flow | `Tree` → tree selector | Partial P2: empty/navigation/restore behavior differs |
| `/trust` | Project trust selector | `Trust` → `TrustSelectorState` | Unproven P3 |
| `/login` | Auth-method then provider/auth completion | `Login` → method/Auth overlay | Missing P0: provider selection has no login action |
| `/logout` | Stored-provider logout selector | `Logout` → Auth selector/logout | Unproven P2: presentation/state parity |
| `/new` | Clear/new session | `New` → `replace_runtime` | Unproven P3 |
| `/compact [instructions]` | Manual compaction | `Compact` action | Unproven P3; component oracle covers synthetic state |
| `/resume` | Session selector | `Resume` → session selector | Unproven P3 |
| `/reload` | Reload config/extensions/skills/prompts/themes | `Reload` action | Unproven P2: theme lifecycle differs |
| `/quit` | Shutdown | `Quit` action | Partial P2: Rust also accepts non-Pi `/exit` alias |

**Non-inventory note.** Pi's interactive switch also recognizes `/debug`, `/arminsayshi`, and `/dementedelves`; they are not in frozen `BUILTIN_SLASH_COMMANDS`, so this audit does not require adding them to Rust’s built-in list. Unknown/malformed slash text must retain Pi’s normal extension/prompt/model routing behavior.
