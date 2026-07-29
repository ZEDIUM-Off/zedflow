# Replan startup provenance handoff

`SEM-CA-V4-INTERACTIVE-EXTENSIONS-STARTUP-PROVENANCE-REPAIR-R1` established that its default-loader/startup scope cannot persist the install receipt it must consume. The receipt persistence and registration boundary belongs to the already-implemented source-install layer.

The DAG therefore replaces the terminal startup writer with two ordered fresh writers:

1. `SEM-CA-V4-INTERACTIVE-EXTENSIONS-INSTALL-PROVENANCE-REPAIR-R1` owns only install/provenance persistence plus its installation regression. It must atomically persist the source directory, content-addressed artifact, and digest-bound receipt in the existing install location/schema; it must not loosen trust, accept prebuilt artifacts, or add dependencies.
2. `SEM-CA-V4-INTERACTIVE-EXTENSIONS-STARTUP-PROVENANCE-REPAIR-R2` owns the original default loader/main/test scope and consumes that persisted registration to construct the real interactive runner from trusted digest-bound artifacts.

A fresh `SEM-CA-V3G-RUST-EXTENSIONS-FIDELITY-R11` re-runs the original independent extension fidelity validation after both writers. `SEM-CA-V5-CLI-MODES` now depends on that fresh review. The terminal source writer and superseded R10 review are removed, so neither can deadlock the repair path.
