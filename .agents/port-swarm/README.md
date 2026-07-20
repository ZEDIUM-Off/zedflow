# Pi port control state

The committed DAG and seed state define intended work. `current.base` is an immutable audit-seed ancestor, not a mutable `HEAD`; controller startup verifies that ancestry before writing external runtime state at `$XDG_STATE_HOME/zedflow-pi-port/state.json`. Runtime state records immutable dispatch bases, result SHAs, attempts, worktrees, sessions, and command outcomes. Git and runnable checks are authoritative.

`tools/pi-port-swarm/controller.py` permits one active writer. It creates a fresh Pi context per unit, validates the candidate, and CAS-advances `refs/heads/automation/pi-port`. A worker may not change the plan; an evidenced `PLAN_CHANGE` invokes one fresh coordinator limited to DAG/state/docs control ownership.

No execution schedule is active. `monitor` is read-only and may be run by an external observer only; its manifest statistics are mechanical target-presence counts, not port-fidelity completion.

Archive refs are audit inputs only; they are never merged wholesale.
