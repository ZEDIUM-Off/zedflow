# Pi TUI functional-parity audit

**Audit baseline:** Zedflow `913488c2022a6ed595e65d19394953d566d4edbc`; frozen Pi submodule `references/pi@2b00dade7cec918aefb025c8b7a4fa304a30acdd`. This is an evidence report, not a product-fix proposal. Source references below name paths at those exact objects.

## Result and priority

| Priority | Blocking finding | Evidence | Repair report |
|---|---|---|---|
| P0 | Unknown-provider/model response provenance and post-login model selection must not silently leave an unauthenticated/unknown model active. | Pi guards `unknown/unknown/unknown` and attempts a verified default-model selection after authentication; Rust has an unknown-provider error stream but the interactive login selector has no login-provider selection action. | [auth-model-provenance](auth-model-provenance.md) |
| P0 | Visible chat provenance/layout is not faithful: unstyled user messages and a footer initialized as `no-model`. | `user-message` and footer source comparison; component oracle has only synthetic lifecycle coverage. | [chat-transcript-streaming](chat-transcript-streaming.md) |
| P1 | No passing whole-CLI differential PTY gate; the existing real-PTY fixtures differ. | `tools/tui-fidelity/run.py --all`: `login.json` and `settings.json` differ. | [terminal-components](terminal-components.md) |
| P1 | Editor completion, marker-aware words, and visual-line movement differ. | Editor/autocomplete source comparison. | [editor-input-keybindings](editor-input-keybindings.md) |
| P1 | `/share` is a status-only placeholder. | Interactive command switch comparison. | [builtin-commands](builtin-commands.md) |
| P2 | Theme chrome/footer, settings/selectors, session navigation, and several commands are incomplete. | Lane-specific source comparisons. | linked reports below |

The component oracle in `tools/tui-parity` is useful but is **not** raw-PTY parity: its frozen side uses a bounded seam rather than a complete Pi interactive runtime. `tools/tui-fidelity` does dispatch the actual frozen Pi CLI and Rust CLI, but it currently differs on both supplied fixtures. In particular, Rust login provider selection is nonfunctional: `SelectorOverlay::select` only performs an `Auth` action for logout, not login.

## Audit lanes

- [Authentication and model provenance](auth-model-provenance.md)
- [Chat, transcript, and streaming](chat-transcript-streaming.md)
- [Theme, chrome, and footer](theme-chrome-footer.md)
- [Editor, input, and keybindings](editor-input-keybindings.md)
- [Settings, model selectors, and login](settings-model-selectors.md)
- [Sessions and navigation](sessions-navigation.md)
- [Built-in commands](builtin-commands.md) and the complete [command matrix](command-matrix.md)
- [Terminal and common components](terminal-components.md)
- [Relevant one-to-one file/test map](one-to-one-file-map.md)

## Evidence method and limitations

The eight saved audit outputs and transcripts in `.pi/subagents/artifacts/` were read as leads: `2fb1079c`, `31f01311`, `efb77538`, `3d3dc44e`, `e7adda82`, `84e1369a`, `3c8414b2`, and `c0fa6b31`. Claims were then checked against the frozen TypeScript and the Rust head named above. Reported test results are historical audit evidence, not a claim that this documentation commit reran them.

No network/credential flow was exercised. Unknown behavior is called out as unknown; it is not inferred from similarly named code. The executable repair order, owned files, and test-first acceptance gates are in [the repair plan](../../../.agents/plans/pi-tui-functional-parity-repair.md).
