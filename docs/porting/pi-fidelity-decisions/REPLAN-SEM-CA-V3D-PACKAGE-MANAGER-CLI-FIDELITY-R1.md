# Replan package-manager CLI fidelity validation

`SEM-CA-V3D-PACKAGE-MANAGER-CLI-FIDELITY-VALIDATE-R1` reported two deterministic failures in the declared package-manager gates, beginning at `crates/zedflow-coding-agent/tests/package-manager.rs:48`. It is removed from the active DAG.

`SEM-CA-V3D-PACKAGE-MANAGER-CLI-FIDELITY-REPAIR-R2` attaches to the already-satisfied direct dependency `SEM-CA-V3D-PACKAGE-MANAGER-CLI-FIDELITY-REPAIR-R1`. It owns only the diagnosed package-manager implementation and focused behavior tests; it must preserve executable source-only Rust extension package operations and must not add dependencies, accept prebuilt artifacts, use TypeScript execution, or weaken tests.

Fresh validator `SEM-CA-V3D-PACKAGE-MANAGER-CLI-FIDELITY-VALIDATE-R2` repeats the same package gates. `SEM-CA-V5-CLI-MODES-R1` now depends on that validator; no repair depends on the removed terminal validator.
