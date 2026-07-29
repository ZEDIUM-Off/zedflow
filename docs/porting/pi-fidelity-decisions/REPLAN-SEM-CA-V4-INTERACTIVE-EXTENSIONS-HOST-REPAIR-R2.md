# Replan default interactive native-extension host repair

`SEM-CA-V4-INTERACTIVE-EXTENSIONS-HOST-REPAIR-R2` could not expose the default host pipeline because its ownership excluded `crates/zedflow-coding-agent/src/core/extensions/mod.rs`. It is removed from the active DAG. `SEM-CA-V4-INTERACTIVE-EXTENSIONS-HOST-REPAIR-R3` attaches to the already-satisfied `SEM-CA-V3G-RUST-EXTENSIONS-NATIVE-EVENTS-REPAIR-R1` dependency and adds only that module root. Fresh reviewer `SEM-CA-V3G-RUST-EXTENSIONS-FIDELITY-R9` retains the declared package gates before `SEM-CA-V5-CLI-MODES`.
