---
description: Read before you open, deliver or integrate a Zedflow mission.
---

# Gitflow

## Purpose

The operator isolates each [mission](/home/zedium/workspaces/docs/domains/governance/terms/mission/TERM.md) until its reviewed changes reach the assigned parent.
The tracked `.gitflow` owns repository routing.

## Preconditions

Read the [workspace Git rules](/home/zedium/workspaces/docs/domains/architecture/protocols/local-git/PROTOCOL.md).
Read the [mission lifecycle](/home/zedium/workspaces/docs/domains/architecture/protocols/mission-lifecycle/PROTOCOL.md).
Use the native Gitflow version required by those rules.
Assign one writing owner per worktree.
Record the objective, scope, parent, checks, budget and stop condition before opening a mission.
Require a clean parent checkout.
Keep historical branches unchanged unless the operator authorizes their modification.

## Operations

### Inspect routing

Run from the repository checkout:

```sh
git branch --show-current
git status --short --untracked-files=all
git worktree list --porcelain
git config --file .gitflow --list
git flow config status
git config --show-origin --show-scope --get-regexp '^gitflow\.'
```

Resolve any routing mismatch before a lifecycle command.
Serialize shared configuration changes.
Use `git flow config sync` only to apply an approved repository configuration.
Use command-scoped bindings for [contributions](/home/zedium/workspaces/docs/domains/architecture/protocols/local-git/PROTOCOL.md#contribution-routing).

### Open a mission

Set `mission_id` from the approved assignment.
Use identifiers matching `[a-z][a-z0-9-]*`.
Check physical containment and an unused destination through the workspace lifecycle.
Read the parent instead of assuming `main`:

```sh
parent_branch=$(git config --file .gitflow --get gitflow.branch.mission.parent)
test "$(git branch --show-current)" = "$parent_branch" || exit 1
parent_head=$(git rev-parse HEAD)
mission_path="$(pwd -P)/.worktrees/$mission_id"
git flow mission start "$mission_id" \
  --worktree-path "$mission_path" --no-fetch --no-cd
```

Stop if creation fails.
Verify the registered path, `mission/$mission_id`, starting commit and recorded parent.
Keep temporary outputs inside the active worktree's `.artifacts/`.

### Deliver and integrate

Commit only assigned durable files after inspecting the staged diff.
Return the source commit, starting commit, immediate parent, changed paths, checks, risks and artifact disposition.
Keep experimental code isolated from [product](../../../product/DOMAIN.md) code.
Require a review of the delivered revision before integration.
Serialize integrations.
Verify both heads, clean checkouts and the recorded `.base` before finishing.

From the mission checkout, use the verified assignment:

```sh
git flow mission finish "$mission_id" \
  --ff-only --keep --no-fetch --no-push --notag
```

Verify that the assigned parent equals the delivered source commit.
Verify retained source refs and worktree registration.
Resolve divergence explicitly before another integration attempt.
Require separate operator approval for publication, promotion to `main` and cleanup.
Follow the workspace lifecycle for authorized closure.

## Failure and recovery

Stop after a nonzero exit or an unexpected ref, path or status change.
Inspect refs, worktrees and the index before recovery.
A failed finish can advance its parent before cleanup fails.
Do not blindly retry finish.
Do not stash, reset, force-remove or delete unrelated work to make a command succeed.
