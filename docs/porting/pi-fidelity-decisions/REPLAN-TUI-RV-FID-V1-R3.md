# REPLAN-TUI-RV-FID-V1-R3

At base `4dd436717ccdab37c4da8863b00ac230a55e2819`, the TUI fidelity review was superseded after the worker found that Rust OSC11 terminal background-color parsing is not Pi-faithful.

The originating reviewer `TUI-RV-FID-V1-R3` is removed. Repair `TUI-C1-R4-OSC11-PARSING` is attached to its already-satisfied dependency `TUI-V1-R3`, followed by fresh validator `TUI-V1-R4` preserving the failed fidelity validation and reviewer `TUI-RV-FID-V1-R4`. The next checkpoint now depends on the fresh reviewer.
