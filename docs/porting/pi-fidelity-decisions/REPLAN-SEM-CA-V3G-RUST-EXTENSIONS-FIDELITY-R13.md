# Replan persisted extension receipt trust

`SEM-CA-V3G-RUST-EXTENSIONS-FIDELITY-R13` established that arbitrary persisted receipt JSON can declare itself trusted: `provenance.rs` verifies only receipt-supplied source/artifact digests, `resource-loader.rs` unconditionally promotes resolved entries, and the default interactive startup consumes them. The terminal review is removed from the active DAG.

`SEM-CA-V4-INTERACTIVE-EXTENSIONS-RECEIPT-TRUST-REPAIR-R1` attaches to the already-satisfied `SEM-CA-V4-INTERACTIVE-EXTENSIONS-RUNTIME-DISPATCH-REPAIR-R2` dependency. It owns only provenance/resource loading and their focused regressions. It must make the managed source-install registration—not arbitrary receipt JSON—the trust authority, reject unregistered or receipt-path-substituted artifacts before default startup, and preserve source-only installation, provenance/digest atomicity, ABI v1 process safety, process-lifetime no-unload, approved dependencies, and TypeScript deferral.

Fresh validator `SEM-CA-V4-INTERACTIVE-EXTENSIONS-RECEIPT-TRUST-VALIDATE-R1` repeats the focused default-harness and loader gates. Fresh reviewer `SEM-CA-V3G-RUST-EXTENSIONS-FIDELITY-R14` repeats the prior independent fidelity gates after that validation. `SEM-CA-V5-CLI-MODES` now depends on R14; no repair depends on the removed terminal review.
