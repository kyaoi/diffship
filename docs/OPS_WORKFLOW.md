# Ops workflow (apply / verify / promote)

This document describes the **implemented** workflow of diffship today.

It is intentionally human-oriented (what to run, what happens, where to look).

---

## End-to-end flow (build → AI → loop)

When you start from local Git diffs (not from a pre-made patch bundle), use this sequence:

1) Build a handoff bundle for AI:

```bash
diffship build --include-staged --include-unstaged --include-untracked
```

2) Inspect before sharing:

```bash
diffship preview ./diffship_YYYY-MM-DD_HHMM --list
diffship preview ./diffship_YYYY-MM-DD_HHMM --part part_01.patch
```

3) Send the handoff bundle to AI and receive an AI-produced patch bundle (`patch-bundle.zip`).

4) Apply+verify+promote in one step:

```bash
diffship loop ./patch-bundle.zip
```

5) If you need reproducibility checks across two handoff outputs:

```bash
diffship compare ./bundle_a ./bundle_b.zip
```

For CI / automation, use JSON output:

```bash
diffship preview ./diffship_YYYY-MM-DD_HHMM --list --json
diffship compare ./bundle_a ./bundle_b.zip --json
```

---

## Concepts

- **session**: a persistent worktree used to avoid touching your main working tree
- **sandbox**: a per-run worktree where the patch bundle is applied
- **run_id**: an identifier for a single apply/verify/promote attempt
- **promotion**: reflecting the verified result back to your target branch

Runtime artifacts are stored under:

- `.diffship/runs/<run-id>/...`
- `.diffship/worktrees/...` (worktrees for sessions/sandboxes)

---

## The main loop

If you have an AI-produced **patch bundle** (directory or `.zip`):

```bash
diffship loop path/to/patch-bundle.zip
```

What `loop` does:

1) **apply** the patch bundle in a sandbox
2) **verify** using a local profile (default: `standard`)
3) if verify passed, **promote** the result back to your target branch
4) record machine-readable logs under `.diffship/runs/<run-id>/`

---

## Common outcomes

### Verify failed → create a reprompt bundle

When verify fails, diffship writes a default reprompt zip at:

- `.diffship/runs/<run-id>/pack-fix.zip`

You can also create or re-create it explicitly:

```bash
diffship pack-fix --run-id <run-id>
```

### Promotion blocked: secrets

If secrets-like strings were detected, promotion is refused.
After confirming it is safe (and fixing the patch bundle if needed), rerun with:

```bash
diffship loop path/to/patch-bundle.zip --ack-secrets
# or, promote an already-verified run
# diffship promote --run-id <run-id> --ack-secrets
```

### Promotion blocked: required user tasks

If the patch bundle includes `tasks/USER_TASKS.md`, promotion is refused until you:

1) complete the tasks
2) rerun with:

```bash
diffship loop path/to/patch-bundle.zip --ack-tasks
# or: diffship promote --run-id <run-id> --ack-tasks
```

---

## Promotion modes and commit policy

You can override promotion behavior per run:

- `--promotion none`:
  - do not change your target branch
  - useful for “apply + verify only” or debugging

- `--promotion working-tree`:
  - apply results onto the target branch working tree **without creating a commit**
  - useful when you want to inspect/edit before committing manually

- `--promotion commit` (default):
  - apply results onto the target branch and create a commit

Commit creation is further controlled by:

- `--commit-policy auto` (default): commit automatically
- `--commit-policy manual`: refuse to auto-commit (expects a commit already exists in the sandbox)

Examples:

```bash
# verify only
$ diffship loop bundle.zip --promotion none

# no-commit promotion (working tree only)
$ diffship loop bundle.zip --promotion working-tree

# require the AI to craft the commit inside the sandbox (advanced)
$ diffship loop bundle.zip --promotion commit --commit-policy manual
```

---

## Useful commands when debugging

### Show recent state

```bash
diffship status
```

### Inspect recent runs

```bash
diffship runs
```

### Verify a specific run again

```bash
diffship verify --run-id <run-id> --profile standard
```

### Promote a specific run

```bash
diffship promote --run-id <run-id>
```

---

## Configuration (supported keys)

diffship merges config sources with precedence:

**CLI > manifest.yaml > project > global > default**

Supported (ops) keys are documented in `docs/CONFIG.md`.
