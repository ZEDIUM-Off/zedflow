# Relevant one-to-one Pi ↔ Rust file/test map

Paths are relative to repository root. This is the relevant functional TUI slice, not a claim that every package file has been re-audited. “Gap/disposition” records source-verified gaps or the test boundary.

| Frozen Pi source/tests | Rust source/tests | Gap / disposition |
|---|---|---|
| `packages/tui/src/components/editor.ts`; `test/editor.test.ts` | `crates/zedflow-tui/src/components/editor.rs`; `tests/editor.rs` | P1 async/debounced trigger completion and visual movement absent; retain unit port tests, add separate differential fixtures |
| `packages/tui/src/autocomplete.ts`; `test/autocomplete.test.ts` | `crates/zedflow-tui/src/autocomplete.rs`; `tests/autocomplete.rs` | P1 trigger chars, cancellation, command-argument completions absent |
| `src/word-navigation.ts`; `test/word-navigation.test.ts` | `src/word-navigation.rs`; `tests/word-navigation.rs` | P1 marker-aware words/visual lines require differential cases |
| `src/{keybindings,keys,stdin-buffer,undo-stack,kill-ring}.ts`; matching tests | Rust same-named files/tests | P2 interactive focus/dispatch unproven; no source mismatch asserted here |
| `src/{tui,terminal,utils,primitives}.ts`; terminal/render/width/overlay tests | Rust same-named files/tests incl. `virtual-terminal`, `tui-render`, `wrap-ansi` | P1 whole-CLI PTY differs; retain tests and extend raw-PTY fixtures |
| `src/components/{box,input,markdown,select-list,settings-list,text,truncated-text,image,loader,cancellable-loader,spacer}.ts`; matching tests | Rust same-named modules/tests | P2 component-specific actual-CLI coverage missing; no blanket source equivalence claim |
| `coding-agent/src/modes/interactive/interactive-mode.ts` | `crates/zedflow-coding-agent/src/modes/interactive/interactive-mode.rs`; `tests/interactive-*`, `tui-parity-rust` | P0/P1/P2 root dispatch, provenance, selectors, session flows, and commands as lane reports |
| `.../components/user-message.ts`; user-message behavior via interactive tests | `.../components/user-message.rs`; `tests/user-message.rs` | P0 theme background/text absent; multiline OSC/style fixture missing |
| `.../components/assistant-message.ts` | `.../components/assistant-message.rs`; `tests/assistant-message.rs` | P0 visible provider/model provenance not found; raw stream fixture missing |
| `.../components/footer.ts`; `core/footer-data-provider.ts` | `.../components/footer.rs`; `core/footer-data-provider.rs`; `tests/footer-*` | P0/P1 live identity and cumulative/footer styling contract incomplete |
| `.../theme/{theme,theme-controller}.ts`, JSON/schema | Rust same theme paths; `tests/test-theme-colors.rs`, `theme-*` | JSON/schema exact match; P1 custom/path/watch/auto lifecycle differs |
| `.../components/{settings,model,scoped-models,theme,config,thinking}-selector.ts` | Rust matching selector files; `tests/interactive-settings-selectors.rs`, `theme-picker.rs` | P0 login dispatch; P1 scoped; P2 inventory/submenus/theme/config behaviors |
| `.../components/oauth-selector.ts`; `core/{auth-storage,auth-guidance}.ts` | Rust matching files; `tests/{interactive-login,oauth-selector,auth-storage,config-auth-guidance}.rs` | P0 login provider selection nonfunctional; safety fixture required |
| `.../components/{session-selector,session-selector-search,tree-selector,user-message-selector}.ts` | Rust matching files; `tests/interactive-session-selectors.rs`, `tree-selector.rs`, session tests | P2 empty/tree navigation/editor restoration gaps |
| `coding-agent/src/core/slash-commands.ts`; interactive command switch | Rust `core/slash-commands.rs`; `tests/interactive-builtins.rs`, import/clone/compaction tests | Same 22 inventory names; P1 `/share`, P2 placeholders/parser/alias/session effects |
| `tools/tui-parity/*` (audit-only differential oracle) | `tests/tui-parity-rust.rs`, interactive parity tests | Component/lifecycle seam, not a copied Pi test or complete Pi CLI |
| N/A: new fidelity contract | `tools/tui-fidelity/{run.py,fixtures/*}` | Actual frozen Pi/Rust CLI dispatch exists; P1 current login/settings frames differ; add independent fixtures |

Test names shown as matching do **not** make their assertions interchangeable. Copied/ported Pi unit tests protect local semantics; new fidelity tests must run both frozen Pi and Rust behavior and fail on a difference.
