# REPLAN-RECOVERY-TUI-KEYS-RV-FID-V6

At base `9777c30b0e21554effb001f782c2c30acdc09a93`, the TUI fidelity review found that Kitty modifier validation is not faithful to frozen Pi.

The originating reviewer `RECOVERY-TUI-KEYS-RV-FID-V6` is removed. Repair `RECOVERY-TUI-KEYS-R8` is attached to its already-satisfied dependency `RECOVERY-TUI-KEYS-R7`, followed by fresh fidelity reviewer `RECOVERY-TUI-KEYS-RV-FID-V7`. The downstream Rust reviewer now depends on that fresh reviewer.
