# Code Context

## Files Retrieved
1. `tools/pi-port-swarm/README.md` (lines 1-42) — documented runtime location and cleanup commands/guarantees.
2. `tools/pi-port-swarm/controller.py` (lines 838-853, 1029-1069, 1244-1264) — automatic cleanup and the exact eligibility/removal checks.
3. `/home/zedium/.local/state/zedflow-pi-port/state.json` — runtime record inventory (read-only inspection): 661 units: 452 `ACCEPTED`, 209 `SUPERSEDED`.
4. `git worktree list --porcelain` from `/home/zedium/workspaces/zedflow` — authoritative currently registered worktree inventory.

## Key Code
- Runtime root: `$XDG_STATE_HOME/zedflow-pi-port` (here `/home/zedium/.local/state/zedflow-pi-port`), documented at `README.md:22`.
- `cleanup_candidates` permits removal only for an `ACCEPTED` record with complete candidate/worktree/session evidence, a worktree under the managed root, durable state/log, exact registered unit ref, clean Git status, and a candidate reachable from `automation/pi-port` (`controller.py:1029-1063`).
- Removal is specifically `git worktree remove <path>` followed by exact expected ref deletion (`controller.py:1066-1069`); no broad runtime deletion.
- On acceptance, the controller attempts to remove that newly accepted eligible worktree automatically (`controller.py:844-853`).

## Architecture
The Git common directory registers every worktree. The controller treats only `/home/zedium/.local/state/zedflow-pi-port/worktrees/` as managed unit worktrees; `state.json` supplies durable outcome/session evidence and `automation/pi-port` supplies the reachability condition. Consequently, a directory being registered or old-looking is not, by itself, controller-approved for deletion.

## Findings

### Worktree list (read-only snapshot)
- Registered total: **933** worktrees.
- Managed unit worktrees under `/home/zedium/.local/state/zedflow-pi-port/worktrees/`: **917**.
- Other registered worktrees (**16**):
  - `/home/zedium/workspaces/zedflow` — `main`
  - `/home/zedium/.local/share/zedflow-worktrees/pi-port-v2` — `revise/pi-port-orchestration-v2`
  - `/home/zedium/.local/share/zedflow-worktrees/pi-port-worker-v2` — `port/worker-v2`
  - `/home/zedium/.local/share/zedflow-worktrees/zedflow-main` — detached
  - `/home/zedium/.local/share/zedflow-worktrees/zedflow-port-control` — `control/recovery-e2big-20260809`
  - `/home/zedium/.local/share/zedflow-worktrees/zedflow-port-extension-replan` — `control/pi-port-extension-replan`
  - `/home/zedium/.local/share/zedflow-worktrees/zedflow-port-semantic` — `control/pi-port-semantic`
  - `/home/zedium/.local/state/zedflow-pi-port/control/p2-r2-1784558290` — `automation/pi-port-control/p2-r2-1784558290`
  - `/home/zedium/.local/state/zedflow-pi-port/control/p2-review-1784556985` — `automation/pi-port-control/p2-review-1784556985`
  - `/home/zedium/workspaces/zedflow-main-audit` — `audit/pi-tui-functional-parity`
  - `/tmp/zedflow-obsidian-human-journeys` — `prototype/obsidian-human-journeys`
  - `/tmp/zedflow-pi-tui-control` — `automation/pi-port-control/pi-tui-parity`
  - `/tmp/zedflow-wayfinder-16` — `prototype/wayfinder-16-registry-audit`
  - `/tmp/zedflow-wayfinder-17` — `wayfinder/issue-17-matrix`
  - `/tmp/zedflow-wayfinder-18` — `prototype/wayfinder-18-harness-controller-audit`
  - `/tmp/zedflow-wayfinder-4` — `prototype/real-differential-harness`

### Obvious stale managed worktrees
- **High — `/home/zedium/.local/state/zedflow-pi-port/worktrees/` (72G apparent):** all 917 managed worktrees remain despite a terminal-only state (452 accepted, 209 superseded). Exact state/path reconciliation: **430 accepted recorded paths**, **209 superseded recorded paths**, and **278 registered managed paths not recorded by current `state.json`**.
- **High — 429 worktrees are controller-eligible now** according to the implemented `cleanup_candidates` predicate. This is the safe, explicit historical-cleanup set; sample units are `AG-H2-R1-SAVE-POINT-OPTIONS`, `AG-P2`, `AG-P2-R1-PROCESS-TREE`, `AG-P2-R2-CANCEL-GROUP`, and `CA-C1-MESSAGE-COMPACTION-UTILS`.
- **Medium — 487 paths are plainly historical/orphan candidates but are not safe to remove through current cleanup without separate investigation:** 209 `SUPERSEDED` paths plus 278 unrecorded paths. Of the 661 state records, cleanup retains 232: 209 not accepted, 22 missing candidate/worktree/session evidence, and 1 dirty accepted worktree. Do not manually remove these based solely on this report.

### Approximate apparent disk usage
- `/home/zedium/workspaces/zedflow`: **88G**.
- `/home/zedium/workspaces/zedflow-main-audit`: **2.4G**.
- `/home/zedium/.local/state/zedflow-pi-port`: **73G** total; managed `worktrees/`: **72G** across 917 directories.
- Largest managed examples: `sem-ca-v8-test-batch-3-validate-r8-1-936eaa2a00e6` **4.8G**, `...validate-r5-1-880dc837a651` **4.8G**, and three validation directories at **4.6G** each.
- Runtime artifacts/state: `sessions/` **159M** (2,334 files), `control/` **17M**, `recovery/` **7.7M**, `monitor/` **988K**, cutover archives **832K** and **1.1M**. `state.json` is ~1.3M; `controller-service.log` is ~17.4M.

### Existing cleanup behavior/commands
- Documented preview (read-only by default): `python3 tools/pi-port-swarm/controller.py cleanup` or `cleanup --dry-run` (`README.md:13`).
- Documented mutating historical cleanup: `python3 tools/pi-port-swarm/controller.py cleanup --accepted` (`README.md:14`); it only removes candidates meeting the code predicate above.
- New accepted units are automatically cleaned only after durable state/logs and integration reachability (`README.md:30`, `controller.py:844-853`). Failed, blocked, running, dirty, and incomplete attempts are retained (`README.md:30`).

## Start Here
Open `tools/pi-port-swarm/README.md` first: lines 22 and 30 define the intended runtime ownership and cleanup contract before any cleanup decision.

```acceptance-report
{
  "criteriaSatisfied": [{"id":"criterion-1","status":"satisfied","evidence":"Read-only Git/runtime inspection produced concrete paths, counts, severity, disk use, and documented cleanup behavior."}],
  "changedFiles": ["context.md"],
  "testsAddedOrUpdated": [],
  "commandsRun": [
    {"command":"git worktree list --porcelain; filesystem/state/du inspection","result":"passed","summary":"933 registered worktrees; runtime and apparent usage inventoried."},
    {"command":"Python read-only cleanup_candidates inspection","result":"passed","summary":"429 eligible; 232 retained with classified reasons."}
  ],
  "validationOutput": ["No deletion or runtime mutation was performed."],
  "residualRisks": ["Apparent du usage can double-count shared/reflinked content.","487 historical/orphan paths are not controller-approved cleanup candidates and require separate evidence."],
  "noStagedFiles": true,
  "diffSummary": "Added requested read-only findings report in context.md.",
  "reviewFindings": ["high: /home/zedium/.local/state/zedflow-pi-port/worktrees/ retains 917 managed worktrees (72G apparent), including 429 currently eligible cleanup candidates.","medium: 278 registered managed paths are absent from current state.json; do not manually remove without recovery/audit review."],
  "manualNotes": "The main repository status was clean at initial inspection; zedflow-main-audit had untracked .pi/subagents/."
}
```
