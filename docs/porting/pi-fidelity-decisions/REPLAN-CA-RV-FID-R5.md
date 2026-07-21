# REPLAN-CA-RV-FID-R5

At base `c12cdce689fa57050af577bfe88905cdfa336337`, the CA-RV-FID-R5 review reports that frozen Pi exposes `LsToolOptions.operations` through both ls factories while Rust has no injectable ls operations.

The open DAG already represents the smallest safe repair: `CA-T2-R3-INJECTED-LS-OPERATIONS` depends on `CA-RV-FID-R5`, owns only `ls.rs` and its focused filesystem test, and requires injected existence, stat, and directory-read operations to reach both `createLsToolDefinition` and `createLsTool` while preserving local filesystem defaults. No dependency, ownership, validation, or supersession change is justified.
