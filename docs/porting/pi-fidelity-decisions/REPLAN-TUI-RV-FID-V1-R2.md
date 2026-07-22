# REPLAN-TUI-RV-FID-V1-R2

At base `41ab5488d87ef2d7085d5d4bcf64c370f98d3b22`, the TUI fidelity review was superseded after the worker found that Rust word-navigation classification still diverges from frozen Pi `Intl.Segmenter` semantics.

The originating reviewer `TUI-RV-FID-V1-R2` is removed. Repair `TUI-C1-R3-UNICODE-SEGMENTER-CLASSIFICATION` is attached to its already-satisfied dependency `TUI-V1-R2`, followed by fresh validator `TUI-V1-R3` preserving the failed wave validation and reviewer `TUI-RV-FID-V1-R3`. The next checkpoint now depends on the fresh reviewer.
