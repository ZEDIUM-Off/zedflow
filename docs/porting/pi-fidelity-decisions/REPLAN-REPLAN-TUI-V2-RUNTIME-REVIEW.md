# REPLAN-REPLAN-TUI-V2-RUNTIME-REVIEW

The reviewer identified a deterministic Pi-fidelity defect in `parse_key`: ordinary printable input, raw Ctrl keys, Alt sequences, and Kitty CSI-u decoding are missing. The originating reviewer is removed, the bounded repair chain is reattached to its satisfied validation dependency, and a fresh equivalent reviewer is scheduled after that validation. Downstream orchestration now waits for the fresh review.
