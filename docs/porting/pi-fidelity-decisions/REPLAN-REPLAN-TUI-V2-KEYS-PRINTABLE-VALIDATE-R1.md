# REPLAN-REPLAN-TUI-V2-KEYS-PRINTABLE-VALIDATE-R1

The printable-key validator failed deterministically in `crates/zedflow-tui/tests/primitives.rs:4` (`parses_legacy_navigation_keys`), although the isolated test passed. The originating validator is superseded by a fresh keys repair and identical validation; the downstream printable-key review is reconnected to that validator.
