# Frozen Pi fidelity registry

These four TSVs are the canonical Stage-1 coverage registry decided in [Define the blocking Pi↔Zedflow coverage registry](https://github.com/ZEDIUM-Off/zedflow/issues/3):

- `behaviors.tsv` — frozen observable contracts and test declarations;
- `links.tsv` — one artifact or proof relation per row;
- `dependencies.tsv` — the fixture/environment/behavior DAG;
- `dispositions.tsv` — narrowly approved exceptions (currently none).

The registry is anchored to Zedflow baseline `9564b26e2afd66d1c28258487c6bc290bc3d7c6f` and Pi gitlink `2b00dade7cec918aefb025c8b7a4fa304a30acdd`. Red `planned://` targets are exact work still required; they are inventory entries, not passing evidence. Existing whole-boundary JSON fixtures are named by repository path and remain red. A behavior cannot become complete until those planned targets are replaced by real artifacts, including a GitHub `human-validation` issue.

The inventory is reproducible from immutable Git objects, so the known dirty Pi worktree cannot affect it. It includes every frozen package source/test artifact, package manifest, package README and package documentation file; every test declaration and documentation heading is recorded separately. Dynamic/parameterized tests are identified by stable declaration site rather than guessed runtime expansion.

```bash
python3 tools/pi-fidelity-registry/registry.py          # validate
python3 tools/pi-fidelity-registry/registry.py --write  # deterministic refresh
```

Validation fails on stale TSVs, unknown relations or dependency types, missing references, duplicate IDs/links, cycles, revision drift, missing proof relations, or any frozen Pi artifact covered in only one direction.
