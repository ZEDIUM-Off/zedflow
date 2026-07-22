# REPLAN-TUI-RV-FID-V1-R4

At base `fefd42c02272af2430b36ea3847bd0009c5ced97`, the TUI fidelity review was superseded after the worker found that Rust OSC11 channel parsing remains narrower than frozen Pi behavior.

The originating reviewer `TUI-RV-FID-V1-R4` is removed. Repair `TUI-C1-R5-OSC11-CHANNEL-PARSING` is attached to its already-satisfied dependency `TUI-V1-R4`, followed by fresh validator `TUI-V1-R5` preserving the failed fidelity validation and reviewer `TUI-RV-FID-V1-R5`. The next checkpoint now depends on the fresh reviewer.
