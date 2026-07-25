# REPLAN-RECOVERY-TUI-KEYS-RV-RUST-V1

At base `a5c698ec3e5c4e58377db14ec5fd0b7402678944`, the Rust-quality reviewer found that `crates/zedflow-tui/src/keys.rs` rejects non-printable Kitty codes before its existing Enter, Tab, and Backspace mappings, leaving those mappings unreachable when no base-layout code is present.

The originating reviewer `RECOVERY-TUI-KEYS-RV-RUST-V1` is removed. Repair `RECOVERY-TUI-KEYS-R2-NONPRINTABLE-CODES` is attached to its already-satisfied direct dependency `RECOVERY-TUI-KEYS-RV-FID-V1`, followed by fresh equivalent reviewer `RECOVERY-TUI-KEYS-RV-RUST-V2`. `NEXT-PORT-PLAN-V20` now depends on the fresh reviewer.
