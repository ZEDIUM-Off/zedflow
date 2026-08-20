# Pi v0.84.2 normative inventory

This directory records the published-package graph and the complete tracked-file inventory used by the Pi fidelity campaign.

- Pi revision: `v0.84.2@914cf1472e715297caa30db4b9535d534a9eb718`
- Scope: every non-private package under `references/pi/packages`, plus the root package/build manifests, lockfile, and README.
- `normative-files.tsv` includes every tracked file owned by that scope. `kind` is a navigation aid; it does not decide whether a file defines a behavior.
- `package-graph.json` records every internal dependency edge declared by a published package, including development edges.
- Git blob IDs are read from the pinned commit, never from the working tree. Historical diffs are not used to select files.

Rebuild or verify from the repository root:

```bash
python3 tools/pi-reference-inventory.py
python3 tools/pi-reference-inventory.py --check
```
