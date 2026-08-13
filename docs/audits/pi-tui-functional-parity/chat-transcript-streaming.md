# Chat transcript and streaming

**Scope.** User/assistant transcript rendering, streamed/tool/abort/compaction updates, and visible model/provider provenance.

## Tests/source evidence

Pi `packages/coding-agent/src/modes/interactive/components/user-message.ts` wraps Markdown in a `Box` styled with `userMessageBg` and Markdown text styled with `userMessageText`; it emits OSC 133 A, B, and C zones. Rust `crates/zedflow-coding-agent/src/modes/interactive/components/user-message.rs` uses an unstyled `TuiBox`, preserves Markdown flags, and emits B+C together. `assistant-message.rs`, `interactive-mode.rs` transcript state, and `ThemedTranscriptView` were inspected: the themed transcript applies global foreground, not the per-user background/text contract.

Saved lane `31f01311` reports passing Rust `interactive-enduser-flow` and `interactive-transcript` stateful tests for message/tool/queue/compaction updates. `tools/tui-parity` has synthetic `streaming`, `tools-compaction`, and `abort-error` fixtures; its README documents that this is not a complete Pi CLI lifecycle. No raw-PTY chat fixture exists.

## Matching behavior

Both implementations render Markdown user content with ordered-list/backslash preservation, maintain transcript state, and have assistant/tool/compaction components. Rust emits OSC prompt zones and has component-level lifecycle coverage.

## Mismatches

- **P0 — user-message visual contract.** Rust lacks Pi's `userMessageBg` and `userMessageText`, so a themed user turn cannot match.
- **P0 — model/provider provenance visible in chat/chrome.** Rust assistant rendering has no discovered visible attribution and footer starts with empty model/`no-model`; live update from active session was not found. Exact identity must be carried rather than guessed.
- **P1 — streaming UI is not whole-CLI proven.** Current passing fixtures inject lifecycle events; actual Pi/Rust PTYs have no comparable message, token stream, tool, abort, compaction, or persistence trace.
- **P2 — ANSI-zone fidelity needs a cell/escape capture.** Rust's combined B+C placement may be equivalent to Pi's adjacent emissions, but this audit did not prove it across multiline content.
- **P3 — transcript persistence/reopen visual behavior unknown.** State tests do not demonstrate persisted transcript reconstruction in a raw terminal.

## Missing fidelity fixtures

Add deterministic actual-CLI PTY flows for user turn, chunked assistant response, tool start/update/end, abort/error, compaction, and persisted-session reopen. Capture cells, cursor, styles, and OSC sequences where decoder support permits; keep model/provider responses offline with a test provider. Include multiline user Markdown proving user background/text and prompt zones.

## Fix boundary

Limit repair to transcript composition, user/assistant components, session-to-footer identity wiring, and differential fixtures. Do not alter model response semantics or normalize frames to conceal a mismatch.
