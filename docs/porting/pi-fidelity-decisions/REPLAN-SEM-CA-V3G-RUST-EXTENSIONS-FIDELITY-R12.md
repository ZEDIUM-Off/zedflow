# Replan default interactive extension runtime dispatch

`SEM-CA-V3G-RUST-EXTENSIONS-FIDELITY-R12` established that the prior dispatch repair left the real default `main`/`InteractiveMode` path unwired: it invokes only startup and shutdown while input, tool, command, provider, and session dispatch remain callable only outside that harness. The terminal review is removed from the active DAG.

`SEM-CA-V4-INTERACTIVE-EXTENSIONS-RUNTIME-DISPATCH-REPAIR-R2` attaches to the already-satisfied `SEM-CA-V4-INTERACTIVE-EXTENSIONS-RUNTIME-DISPATCH-REPAIR-R1` dependency and owns only `main.rs`, `interactive-mode.rs`, and the default-harness extension regression. It must route the retained trusted runner through actual interactive flow without weakening trust, ABI/process-safety, source-only installation, or the TypeScript deferral.

Fresh reviewer `SEM-CA-V3G-RUST-EXTENSIONS-FIDELITY-R13` repeats the original declared package gates after that repair. `SEM-CA-V5-CLI-MODES` now depends on R13; no repair depends on the removed terminal review.
