# REPLAN-TUI-RV-FID-V1-R5

At base `5582ebb3a6b2b22b550716e94eaeb0cba547b77d`, the TUI fidelity review found that OSC11 hex payloads are sliced before ASCII-hex validation, allowing non-ASCII input to panic instead of matching frozen Pi's undefined result.

The originating reviewer `TUI-RV-FID-V1-R5` is removed. Repair `TUI-C1-R6-OSC11-HEX-SAFETY` is attached to its already-satisfied dependency `TUI-V1-R5`, followed by fresh validator `TUI-V1-R6` and reviewer `TUI-RV-FID-V1-R6`. The next checkpoint now depends on the fresh reviewer.
