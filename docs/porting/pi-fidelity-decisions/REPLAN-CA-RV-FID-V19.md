# Replan missing utility-complements port

The `CA-RV-FID-V19` review found that the required coding-agent utility-complements modules were absent, not merely unregistered. The originating reviewer is removed from the active DAG.

`CA-C22-UTILITY-COMPLEMENTS` attaches to the reviewer's already-satisfied dependency `CA-C21-V1-UTILITY-COMPLEMENTS` and owns the missing Pi utility modules plus `utils/mod.rs` for registration. A fresh validator and Pi-fidelity reviewer run after the repair, followed by the existing successor checkpoint. The failed review is not bypassed.
