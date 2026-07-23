# Repair utility-complements ownership and module registration

At base `a0feb1c7e296dc0bc4a6429fbfd822643782b847`, the utility-complements worker modified `crates/zedflow-coding-agent/src/utils/mod.rs` outside its declared ownership and added a duplicate `pub mod utils` declaration in `src/lib.rs`.

The originating writer is removed from the active DAG. `CA-C21-R1-UTILITY-COMPLEMENTS-OWNERSHIP` attaches to its already-satisfied dependency `CA-NEXT-PORT-DAG-V17`, owns the two diagnosed files, and repairs only the duplicate declaration and module registration. `CA-C21-V1-UTILITY-COMPLEMENTS` is a fresh validator preserving the formatting gate and focused package validation; the fidelity reviewer remains downstream of it.
