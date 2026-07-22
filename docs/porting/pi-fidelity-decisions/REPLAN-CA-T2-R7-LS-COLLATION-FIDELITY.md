# REPLAN-CA-T2-R7-LS-COLLATION-FIDELITY

At base `79041eec86505674d4e64c16738b8983235d022c`, `CA-T2-R7-LS-COLLATION-FIDELITY` reports that faithful JavaScript `localeCompare` ordering requires a Unicode collation dependency, but its ownership excluded dependency manifests.

The unit now also owns `crates/zedflow-coding-agent/Cargo.toml` and `Cargo.lock`, allowing the required dependency to be added without splitting implementation and dependency selection across workers. Its dependency chain and fresh reviewer remain unchanged.
