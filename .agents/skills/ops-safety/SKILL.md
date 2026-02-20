---
name: ops-safety
description: How to evolve apply/verify/loop safely (locks, rollback, path guards, and deterministic logs).
---

# Ops safety

diffship ops commands (`apply`, `verify`, `loop`, `pack-fix`, `status`) are designed to be **boringly safe**.

## Non-negotiables

### 1) Clean worktree by default (OS mode)
- In OS mode, ops runs MUST use isolated worktrees (session + sandbox).
- Default behavior MUST keep the session/sandbox worktree clean; the user’s main working tree should not be mutated during apply/verify.
- Only allow bypassing isolation via an explicit escape hatch flag (discouraged).

### 2) Base commit match by default
- Patch bundles MUST declare `base_commit`.
- Default behavior MUST require `base_commit` to match the session HEAD (or the sandbox base) before applying.

### 3) Locking
- Prevent concurrent ops runs using `.diffship/lock`.
- `status` must surface lock state and how to recover from stale locks.

### 4) Preflight before mutating
- Always run `git apply --check` (or equivalent) before applying.

### 5) Automatic rollback
- If apply fails after any mutation, rollback automatically:
  - `git apply` flow → `git reset --hard HEAD`
  - `git am` flow → `git am --abort`

### 6) Strict path guards
- Refuse paths that are absolute, contain `..`, or target forbidden prefixes (e.g., `.git/`, `.diffship/`).
- Do not follow symlinks when enforcing allowed/forbidden rules.

## MVP: refuse tricky patch features
Refuse by default unless/until explicitly supported:
- `GIT binary patch`
- submodule changes
- file mode changes
- rename/copy metadata

## Logging contract
- Every ops command writes a run directory under `.diffship/runs/<run-id>/`.
- `pack-fix` should bundle logs + diff + context into a single zip ready to send back to an AI.


## Promotion
- Promotion (reflecting results back to `develop` etc.) should be policy-driven: `none`, `working-tree`, or `commit`.
- Promotion MUST be blocked by default when secrets are suspected or required user tasks are present (unless explicitly acknowledged).


## Secrets & user tasks
- Ops MUST scan for likely secrets and MUST never print secret values (only paths + reasons).
- If the patch bundle includes `tasks/USER_TASKS.md`, diffship must surface it prominently and may block promotion until acknowledged.
