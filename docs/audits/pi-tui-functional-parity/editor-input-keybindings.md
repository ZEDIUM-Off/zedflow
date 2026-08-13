# Editor, input, and keybindings

**Scope.** TUI editor editing/completion/word and visual-line navigation, input decoding, and registered interactive keybindings.

## Tests/source evidence

Pi `packages/tui/src/components/editor.ts` has async, debounced trigger-character completion. Pi `autocomplete.ts` includes `triggerCharacters`, cancellation, and `SlashCommand.getArgumentCompletions`. Rust `crates/zedflow-tui/src/components/editor.rs` calls synchronous suggestions after ordinary insertion; Rust `autocomplete.rs` lacks those contracts. Pi word/navigation code is marker-aware and editor movement/scroll follows wrapped visual lines. Rust editor uses plain `find_word_*` and logical-line movement, leaving marker-heavy paste and wrapped arrows/page navigation divergent.

Saved lane `3d3dc44e` reports `tools/tui-parity/run.py --all` passed seven component-oracle fixtures and focused Rust editor/autocomplete/word-navigation/keybindings tests passed. That lane correctly records that these fixtures omit trigger completion, cancellation, command arguments, marker words, and visual movement; a coding-agent test segment timed out and was not used as passing evidence.

## Matching behavior

Both trees include one-to-one editor/autocomplete/keybindings/keys/input/undo/kill-ring modules and broad unit coverage. The component oracle covers ordinary input/history, bracketed paste, Unicode/resize, and basic command routing.

## Mismatches

- **P1 — completion contract.** Async debounce, trigger chars, cancellation, and slash-command argument completions are absent.
- **P1 — navigation/render contract.** Marker-aware word operations and visual wrapped-line arrows/page scrolling are absent.
- **P2 — keybinding behavior needs actual interactive dispatch proof.** Matching registries/components do not prove that all bound actions reach the root editor under focus/overlay states.
- **P3 — IME/native-modifier boundary unknown.** No frozen differential fixture covers composition/native modifier edge cases.

## Missing fidelity fixtures

Create independent differential component fixtures for delayed trigger completion/cancellation/arguments and marker/visual-line operations, then raw-PTY fixtures that send the actual escape sequences to both CLIs through editor and overlay focus. Include wide/combining graphemes, bracketed paste markers, resize, history, and page movement. Copied Pi tests are supporting port tests only; comparisons must be new fidelity tests.

## Fix boundary

Own `zedflow-tui` editor/autocomplete/word-navigation/keybinding wiring and their fidelity fixtures. Do not change Pi command syntax or add a completion framework not demanded by frozen behavior.
