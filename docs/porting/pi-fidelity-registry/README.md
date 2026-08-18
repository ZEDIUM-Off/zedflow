# Operational Pi fidelity registry

These four editable TSVs are the Stage-1 evidence ledger for [issue 17](https://github.com/ZEDIUM-Off/zedflow/issues/17):

- `behaviors.tsv` — hand-authored atomic observable behaviors only;
- `links.tsv` — frozen Pi sources, baseline implementations, and evidence relations;
- `dependencies.tsv` — the acyclic pending/satisfied evidence dependency DAG;
- `dispositions.tsv` — narrowly approved exceptions (none in this campaign).

The ledger is anchored to Zedflow `e91b44be9c897aef63c84c34b4e14b387a8141a7` and frozen Pi `2b00dade7cec918aefb025c8b7a4fa304a30acdd`. A green or present link must name an object that exists at that immutable baseline. Red links are not passing evidence; no approval or normalization is implied by a red fixture.

## Current truthful partial matrix

The fresh Linux pass records 14 atomic behaviors, 112 links, and 14 dependency edges. It covers 12 of 777 normative frozen-Pi anchors; 765 remain unverified. Package/status counts are `ai: red=1`, `agent: red=1`, `tui: red=1`, `coding-agent: implemented=3`, and `orchestrator: implemented=8`. There are no differential-green, reviewed, human-validated, or complete behaviors and no green evidence links.

The 11 implemented entries are confirmed divergences, not fidelity passes. Their baseline-source probes live in `tools/differential-harness/confirmed_divergences.py`; the probes intentionally remain red because they compare historical source contracts rather than bless a product result. The remaining three entries are representative unverified behaviors, with pending fixtures. The old generated file/heading/export inventory was removed in `4cc58cfd`; it was never a behavior matrix.

## Re-run

```bash
# Initializes the frozen submodule only when this worktree has no checkout.
git -c protocol.file.allow=always submodule update --init references/pi
python3 tools/pi-fidelity-registry/registry.py --self-check
python3 tools/pi-fidelity-registry/registry.py
python3 tools/pi-fidelity-registry/registry.py --uncovered
python3 tools/differential-harness/confirmed_divergences.py
```

`--require-complete` is the final gate and must fail until every behavior is complete and all 777 anchors are covered. The exact blocker is semantic inventory: the frozen surface has 777 file/test/document anchors, while creating truthful atomic observable contracts and executable evidence for the remaining 765 requires direct per-capability Pi↔Rust investigation. This registry deliberately does not convert those anchors into file, heading, or export “behaviors,” and it does not fabricate approvals or green evidence.
