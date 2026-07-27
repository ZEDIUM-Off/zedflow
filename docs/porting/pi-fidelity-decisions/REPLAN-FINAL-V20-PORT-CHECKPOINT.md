# Final Stage-1 port checkpoint

- **Checkpoint:** `REPLAN-FINAL-V20-PORT-CHECKPOINT`
- **Accepted SHA:** `6d79ec553d5eb6dcb45ccd07f57ad51c1f37a1ef`
- **Prerequisites:** `REPLAN-FINAL-V20-FIDELITY-REVIEW`, `REPLAN-FINAL-V20-RUST-REVIEW`
- **Validation:** `git diff --check`

The corrected baseline and the fresh final fidelity and Rust reviews are accepted on this single immutable SHA. Stage 1 remains complete only under the exit gate recorded in `docs/porting/BASELINE.md`; Stage 2 remains deferred until that gate is promoted and revalidated on `main`.
