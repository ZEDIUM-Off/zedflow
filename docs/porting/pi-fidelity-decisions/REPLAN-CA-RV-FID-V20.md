# Replan changelog link normalization fidelity

The `CA-RV-FID-V20` review found that changelog link normalization is not Pi-faithful. The originating reviewer is removed from the active DAG.

`CA-RV-FID-V20-R1-CHANGELOG-LINK-NORMALIZATION` attaches to the reviewer's already-satisfied dependency `CA-C22-V1-UTILITY-COMPLEMENTS` and owns only `crates/zedflow-coding-agent/src/utils/changelog.rs`. A fresh Pi-fidelity reviewer runs after the repair, followed by the existing successor checkpoint. The failed review is not bypassed.

The follow-up review still found changelog normalization gaps. `CA-RV-FID-V20-R1` is removed from the active DAG; `CA-RV-FID-V20-R1-CHANGELOG-LINK-NORMALIZATION-R1` attaches to its already-satisfied direct dependency and owns only the changelog implementation. `CA-RV-FID-V20-R2` is the fresh equivalent reviewer before the successor checkpoint.
