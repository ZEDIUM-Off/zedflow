# Port-swarm decisions

- The source snapshot is created by `GIT_INDEX_FILE`, `read-tree`, `add -A`, `write-tree`, and `commit-tree`; source HEAD, index, and worktree are not changed.
- `automation/pi-port` exists only in the automation clone. Slot branches are persistent recovery evidence.
- Crate choices require a documented comparison in the unit result before implementation. Existing workspace dependencies win; no dependency is added solely for convenience.
- Integration is CAS-protected by unit/base SHA/plan hash and exact-SHA review plus validation.
