# Operational Pi fidelity registry

These four editable TSVs are the canonical Stage-1 evidence ledger decided in [Define the blocking Pi↔Zedflow coverage registry](https://github.com/ZEDIUM-Off/zedflow/issues/3) and revised in [Audit and revise the Pi↔Zedflow consolidation registry](https://github.com/ZEDIUM-Off/zedflow/issues/16):

- `behaviors.tsv` — hand-authored atomic observable behaviors only;
- `links.tsv` — one artifact or proof relation per row;
- `dependencies.tsv` — the fixture/environment/behavior DAG;
- `dispositions.tsv` — narrowly approved exceptions.

The ledger starts empty for the fresh Linux campaign. The former generated rows treated files, headings, exports, and test declarations as behaviors; Git history retains that mechanical snapshot, but it is not fidelity evidence. The validator now inventories those frozen Pi anchors independently and reports how many are linked to a behavior or covered by an approved `source` disposition.

The campaign is anchored to Zedflow `e91b44be9c897aef63c84c34b4e14b387a8141a7` and Pi `2b00dade7cec918aefb025c8b7a4fa304a30acdd`. Red `planned://` targets are allowed work items. Passing relations must point to an existing repository object at the exact Zedflow SHA; a passing `run_evidence` therefore cannot drift from the implementation it tested. Dependency states are only `pending` or `satisfied`.

Status is calculated, never stored:

`inventoried → red → implemented → differential_green → reviewed → human_validated → complete`

```bash
python3 tools/pi-fidelity-registry/registry.py                  # validate an incremental ledger
python3 tools/pi-fidelity-registry/registry.py --uncovered      # list unlinked Pi anchors
python3 tools/pi-fidelity-registry/registry.py --require-complete # final blocking gate
python3 tools/pi-fidelity-registry/registry.py --self-check
```

`--require-complete` fails until every behavior is complete and all 777 frozen Pi anchors are covered. This is intentional: the next fresh matrix campaign populates evidence without any generator overwriting reviewed TSV edits.
