# Executable Pi TUI functional-parity repair plan

**Contract:** repair against frozen `references/pi@2b00dade7cec918aefb025c8b7a4fa304a30acdd`, not current Pi. Start from Zedflow audit head `913488c2`. This plan changes implementation/tests only in later units; this document makes no product fix. Every fidelity fixture is newly authored, runs **actual frozen Pi CLI dispatch and actual Rust CLI dispatch** under `tools/tui-fidelity`, and compares raw terminal/cell/cursor output. It is separate from copied/ported Pi tests and `tools/tui-parity`’s bounded component seam. Do not bless or normalize a mismatch.

**Known red baseline:** `tools/tui-fidelity/run.py --all` currently differs for `login.json` and `settings.json`. **Known safety blocker:** Rust login provider selection is nonfunctional (`SelectorOverlay::select` only acts for logout); do not advertise it as working or exercise real credentials until P0. Unknowns in the audit remain unknown until a frozen behavior and differential fixture resolve them.

## Global rules and gates

- Each unit begins by adding a failing differential fixture and saving Pi/Rust raw + decoded artifacts. A source-only assertion supplements, never replaces, CLI dispatch.
- Keep fixture HOME/temp sessions/fake executables isolated; `NO_PROXY=*`, offline Cargo/npm, and no real provider or credential network calls. Assert secrets are absent from raw captures and logs.
- Focused gate order: fixture command named below, then affected `cargo test -p ... --test ...`; use `cargo fmt --all --check` before integration. At phase boundaries run `cargo check --workspace --all-targets` and `cargo test --workspace --all-targets --no-run` with temporary target dir when appropriate.
- Do not edit frozen Pi. Do not alter `tools/tui-fidelity` to ignore cells/styles/cursor or make copied Pi tests be fidelity tests.

## P0.1 — Unknown-provider/model response provenance and authentication safety

- **Owned files:** `crates/zedflow-ai/src/models.rs`, `crates/zedflow-ai/tests/models-runtime.rs`, `crates/zedflow-coding-agent/src/modes/interactive/{interactive-mode.rs,components/oauth-selector.rs,components/footer.rs}`, `src/core/{auth-storage.rs,model-registry.rs}`, focused auth/interactive tests, `tools/tui-fidelity/{run.py,fixtures/login-*.json}`.
- **Prerequisites:** deterministic test auth/provider adapter and fixture-only model catalog; no real OAuth/API key entry.
- **Frozen behavior:** Pi treats only `unknown/unknown/unknown` as unknown; after login it refreshes, validates provider default and availability, calls `setModel`, and reports distinct failures. Unknown provider dispatch is a terminal assistant error, not an invented provider/model. Pi login is method → provider → completion/cancel.
- **Test first:** make `login-unknown-model.json` fail by dispatching `/login`, choosing method/provider, completing through the fake adapter, and comparing successful selection; add cases for no default, unavailable default, selection error, cancellation, and unknown-provider request error. Assert no secret/plain credential appears in raw/decoded artifacts and selected model/provider reaches transcript/footer.
- **Focused gates:** `python3 tools/tui-fidelity/run.py login-unknown-model.json --artifacts /tmp/...`; `cargo test -p zedflow-ai --test models-runtime`; `cargo test -p zedflow-coding-agent --test interactive-login --test auth-storage --test oauth-selector --test footer-data-provider`.
- **Acceptance:** Enter on Rust login provider produces the frozen-auth-equivalent action; all branches compare equal offline; identity/error provenance is preserved end-to-end; secret safety assertions pass. Do not decide behavior for non-sentinel unknown model shapes without frozen evidence.

## P1.1 — Real differential PTY harness as the fidelity gate

- **Owned files:** `tools/tui-fidelity/{README.md,run.py,decoder.mjs,fixtures/*.json}` and only fixture-support test hooks required by later units.
- **Prerequisites:** P0 test adapter interface; retain existing `login.json` and `settings.json` as red findings until their owning units fix them.
- **Frozen behavior:** run archived Pi object at exact SHA and Rust binary as complete CLIs in raw PTYs; compare every decoded cell/style/cursor and preserve raw artifacts.
- **Test first:** add a harness self-check that deliberately changes one terminal cell and proves nonzero exit/artifact retention; add fixture schema validation and CLI-command assertion so a fixture cannot silently bypass actual dispatch.
- **Focused gates:** `python3 tools/tui-fidelity/run.py --all --artifacts /tmp/zedflow-tui-fidelity`; Python syntax/self-check command documented in tool README.
- **Acceptance:** deterministic offline run, frozen-SHA check, actual dual CLI dispatch, and fail-closed comparison. Coverage grows through later units; green is not claimed until all fixtures are equal.

## P1.2 — Chat, streaming, tools, abort, compaction, persistence

- **Owned files:** `crates/zedflow-coding-agent/src/modes/interactive/{interactive-mode.rs,components/{user-message.rs,assistant-message.rs,tool-execution.rs,compaction-summary-message.rs}}`, existing interactive transcript tests, `tools/tui-fidelity/fixtures/{chat,streaming,tools-compaction,abort-error,transcript-persistence}.json`.
- **Prerequisites:** P0 identity adapter; P1.1 fixture contract.
- **Frozen behavior:** user messages use `userMessageBg`/`userMessageText` Markdown and OSC 133 zones; transcript visibly updates on stream/tool/abort/compaction and restores persisted state.
- **Test first:** add a multiline Markdown user-turn differential fixture that fails on background/text/zone cells, then fixture traces for chunked assistant output, tool lifecycle, abort/error, compaction, and reopen. Use an offline response stream.
- **Focused gates:** each new `tui-fidelity` fixture; `cargo test -p zedflow-coding-agent --test user-message --test assistant-message --test interactive-transcript --test interactive-enduser-flow`.
- **Acceptance:** all traces have matching cells/styles/cursor/protocol bytes where captured; selected model/provider is visible only as Pi makes it visible; no synthetic lifecycle event is presented as CLI evidence.

## P1.3 — Theme, root chrome, footer, title, and progress

- **Owned files:** `crates/zedflow-coding-agent/src/modes/interactive/{interactive-mode.rs,components/footer.rs,theme/{theme.rs,theme-controller.rs}}`, `crates/zedflow-tui/src/terminal.rs`, theme/footer tests, `tools/tui-fidelity/fixtures/{theme-*,footer-*,terminal-*.json}`.
- **Prerequisites:** P1.1; P1.2 supplies real transcript/root composition.
- **Frozen behavior:** Pi resolves built-in, registered/path, automatic terminal themes and reload/watch behavior; footer derives cumulative usage/cache/cost/OAuth/context/reasoning with dim/warning/error styling; interactive mode emits configured title/progress behavior.
- **Test first:** create failing fixed-environment dark/light/custom/path/automatic fixtures, narrow footer thresholds, title/progress raw-byte checks, and preview/cancel/reload trace.
- **Focused gates:** fixture family; `cargo test -p zedflow-tui --test test-themes --test terminal`; `cargo test -p zedflow-coding-agent --test test-theme-colors --test footer-width --test theme-detection --test theme-picker`.
- **Acceptance:** exact assets remain exact; resolver/lifecycle/footer/chrome frames match Pi. Do not invent a theme format or claim platform watcher behavior not reproducible offline.

## P2.1 — Selectors, settings, and login integration

- **Owned files:** `crates/zedflow-coding-agent/src/modes/interactive/{interactive-mode.rs,components/{settings-selector.rs,model-selector.rs,scoped-models-selector.rs,theme-selector.rs,config-selector.rs,thinking-selector.rs,oauth-selector.rs}}`, `src/core/settings-manager.rs`, selector tests, `tools/tui-fidelity/fixtures/{settings,model,scoped-models,login,logout}.json`.
- **Prerequisites:** P0 login safety and P1.1 harness; P1.3 theme preview support.
- **Frozen behavior:** Pi ordered settings inventory/actions/persistence; `None` scoped models means all enabled before toggle; automatic theme nested selector/cancel restores preview; config selector is grouped/subgrouped/filterable/scoped tri-state.
- **Test first:** make each dispatch fixture fail on row order/selection/cancel/persist/reopen, all-enabled toggle, warning/thinking/theme submenu, config filtering/tri-state, and login/logout provider UI. Do not store real credentials.
- **Focused gates:** fixture family; `cargo test -p zedflow-coding-agent --test interactive-settings-selectors --test interactive-login --test oauth-selector --test settings-manager --test theme-picker`.
- **Acceptance:** supplied `login.json` and `settings.json` become equal without ignored regions; every frozen settings action represented by evidence; unknown catalogue/policy rows are recorded rather than fabricated.

## P2.2 — Editor/input/keybindings

- **Owned files:** `crates/zedflow-tui/src/{components/editor.rs,autocomplete.rs,word-navigation.rs,keybindings.rs,keys.rs,stdin-buffer.rs}`, matching Rust tests, `tools/tui-fidelity/fixtures/editor-*.json`.
- **Prerequisites:** P1.1; fixture-only deterministic completion source.
- **Frozen behavior:** async/debounced trigger character completion with cancellation and slash argument completion; marker-aware words; wrapped visual-line arrow/page navigation and scrolling.
- **Test first:** failing component differential cases plus actual-CLI PTY sequences for trigger/debounce/cancel/arguments, bracketed marker paste, Unicode/combining/wide characters, resize, history, overlay focus, and visual page movement.
- **Focused gates:** editor fixture family; `cargo test -p zedflow-tui --test editor --test autocomplete --test word-navigation --test keybindings --test input --test stdin-buffer`.
- **Acceptance:** behavior equals frozen Pi for deterministic traces and actual escape dispatch. No new completion architecture unless necessary to reproduce the frozen contract.

## P2.3 — Sessions, navigation, and command effects

- **Owned files:** `crates/zedflow-coding-agent/src/{core/slash-commands.rs,modes/interactive/interactive-mode.rs,modes/interactive/components/{session-selector.rs,session-selector-search.rs,tree-selector.rs,user-message-selector.rs}}`, existing session/interactive tests, `tools/tui-fidelity/fixtures/{commands,sessions,tree,fork,share}.json`.
- **Prerequisites:** P1.1 and P1.2; deterministic temporary session graph and fake offline `gh` executable.
- **Frozen behavior:** exact `/quit` only; path argument first-token/outer-quote parsing; `/share` HTML/gist auth/error/cancel/cleanup/URL flow; session/changelog/hotkeys transcript content; empty fork/tree guards, current-leaf/branch-summary/cancel navigation, rerender/editor restoration, normalized `/name` query/warnings.
- **Test first:** a complete command matrix fixture types every frozen command and malformed/argument form; separate session graph fixtures fail for each tree/fork/name/path branch; fake `gh` drives share success/failure/cancel and asserts cleanup.
- **Focused gates:** command/session fixture family; `cargo test -p zedflow-coding-agent --test interactive-builtins --test interactive-mode-import-command --test interactive-mode-clone-command --test interactive-session-selectors --test tree-selector --test agent-session-tree-navigation`.
- **Acceptance:** every [matrix row](../../docs/audits/pi-tui-functional-parity/command-matrix.md) is equal or has an explicit frozen-source disposition approved before closure; `/exit` is rejected as Pi does; no command is downgraded to status-only where Pi has content/effect.

## P3.1 — Terminal/common components and residual protocol matrix

- **Owned files:** affected `crates/zedflow-tui/src/{terminal.rs,tui.rs,utils.rs,components/*}`, matching tests, `tools/tui-fidelity/fixtures/{unicode-resize,terminal-protocol,components}.json`.
- **Prerequisites:** P1.1 and all feature-lane fixtures.
- **Frozen behavior:** shared terminal cell/layout semantics including ANSI/OSC, width/wrap/overlay, focus/clipboard/image/suspend behavior only where frozen source and reproducible fixture establish it.
- **Test first:** differential cases for CJK/emoji/combining/ANSI hyperlinks/OSC 133, resize/overlay boundaries, and each reproducible protocol behavior; record unsupported platform-specific behavior as unknown.
- **Focused gates:** fixture family; affected `cargo test -p zedflow-tui --all-targets` subsets.
- **Acceptance:** no remaining raw-PTY discrepancy is hidden in common code; platform-only unknowns are listed with reproduction constraints, not guessed.

## Final — Exact-SHA review and exit gate

- **Owned files:** audit reports, this plan, `tools/tui-fidelity` fixtures/results policy, and only review-discovered test/docs corrections. No broad implementation rewrite.
- **Prerequisites:** every unit acceptance green and no unresolved P0/P1.
- **Frozen behavior:** review the exact Pi gitlink/object again (`git -C references/pi rev-parse HEAD` must be `2b00dade...`) and the intended Rust commit; re-read all command matrix/source mappings rather than relying on stale line numbers.
- **Test first:** run the complete real-PTY suite from a clean isolated environment and require it to fail if Pi SHA, Rust CLI dispatch, cell/style/cursor comparison, or fixture artifact collection is bypassed.
- **Focused gates:** `python3 tools/tui-fidelity/run.py --all --artifacts /tmp/zedflow-tui-fidelity-final`; `python3 tools/tui-parity/run.py --all`; `cargo fmt --all --check`; `cargo check --workspace --all-targets`; `cargo test --workspace --all-targets --no-run`; relevant full tests as capacity permits; `git diff --check`.
- **Acceptance:** exact SHAs recorded, all differential fixtures equal, copied Pi tests remain supplementary, no P0/P1 unresolved, every P2/P3 has either a passing fixture or explicit approved disposition, and final review contains no invented parity claim.
