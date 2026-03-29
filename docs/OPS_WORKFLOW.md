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
diffship preview ./diffship_YYYY-MM-DD_HHMM_<head7> --list
diffship preview ./diffship_YYYY-MM-DD_HHMM_<head7> --part part_01.patch
```

Optional: export a replayable handoff plan:

```bash
diffship build --include-staged --include-unstaged --include-untracked --plan-out ./diffship_plan.toml
diffship build --plan ./diffship_plan.toml --out ./replayed_bundle
```

3) Send the handoff bundle to AI and receive an AI-produced patch bundle (`patch-bundle.zip`).

Optional: validate the returned patch bundle contract before running ops:

```bash
diffship validate-patch ./patch-bundle.zip
diffship validate-patch ./patch-bundle.zip --json
```

4) Apply+verify+promote in one step:

```bash
diffship loop ./patch-bundle.zip
diffship loop ./patch-bundle.zip --delete-input-zip
```

If ops.post_apply commands are configured, diffship runs them automatically in the sandbox immediately after apply succeeds and before verify starts.
When `--delete-input-zip` is set, diffship removes the original input `.zip` after copying it into the run directory; directory inputs are left untouched.

5) If you need reproducibility checks across two handoff outputs:

```bash
diffship compare ./bundle_a ./bundle_b.zip
```

For CI / automation, use JSON output:

```bash
diffship preview ./diffship_YYYY-MM-DD_HHMM_<head7> --list --json
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
- `.diffship/tmp/...` (repo-local temp dirs for diffship-spawned commands and previews)
- `.diffship/worktrees/...` (worktrees for sessions/sandboxes)

Inside each run directory, external command logs are grouped by phase:

- `commands.json`: machine-readable index
- `apply/`
- `post-apply/`
- `verify/`
- `promote/`

Phase summaries such as `apply.json`, `verify.json`, and `promotion.json` may also include a normalized `failure_category` when a phase stops early. These categories are intended for stable local follow-up logic and avoid depending on raw stderr wording.

---

## The main loop

If you have an AI-produced **patch bundle** (directory or `.zip`):

```bash
diffship loop path/to/patch-bundle.zip
```

What `loop` does:

1) **apply** the patch bundle in a sandbox
2) **post-apply** local commands, if configured
3) **verify** using a local profile (default: `standard`)
4) if verify passed, **promote** the result back to your target branch
5) record machine-readable logs under `.diffship/runs/<run-id>/`

---

## Common outcomes

### Verify failed → create a reprompt bundle

When verify fails, diffship writes a default reprompt zip at:

- `.diffship/runs/<run-id>/pack-fix_YYYY-MM-DD_HHMMSS_<head7>[_N].zip`

When workflow strategy mode is not `off`, the reprompt zip also includes `strategy.resolved.json`.
`PROMPT.md` points the AI at that file first and also summarizes the detected normalized failure category plus a selected strategy profile and deterministic alternatives before the detailed run evidence.
Known built-in strategy profiles may also expose machine-readable `tests_expected` and `preferred_verify_profile` hints there, including the fast path `no-test-fast`.

When local post-apply hooks ran, that reprompt zip also includes:

- `run/post_apply.json`
- `run/post-apply/*`

`run/post_apply.json` now also summarizes `changed_paths`, coarse `change_categories`, and a machine-readable normalization summary derived from sandbox state before/after the local normalization step.

The generated `PROMPT.md` points the AI at that post-apply evidence before the verify logs and repeats the changed-path summary inline.

You can also create or re-create it explicitly:

```bash
diffship pack-fix --run-id <run-id>
```

You can inspect the same failure-aware recommendation locally without opening the zip:

```bash
diffship strategy --run-id <run-id>
diffship strategy --latest --json
```

### Post-apply failed → create a reprompt bundle

When a local `ops.post_apply` command fails after the patch step succeeded, diffship also writes the default reprompt zip under the same run directory so you can send the failure context back to the AI immediately.

### Reclaim disk from finished/orphaned workspaces and artifacts

```bash
diffship cleanup --dry-run
diffship cleanup
diffship cleanup --include-runs
diffship cleanup --include-builds
diffship cleanup --all
```

`cleanup` also removes leftover diffship-owned temp artifacts under `.diffship/tmp/`.
With `--include-runs` or `--all`, cleanup also removes terminal run directories such as promoted runs, `promotion=none` runs, failed promotions, failed verifies, and orphaned runs, while leaving runs that are still waiting for follow-up promotion acknowledgement intact.

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

The human-readable `runs` and `status` outputs now show `commands=<n>` and phase names when a run recorded external command logs.
They also surface a derived `state=...` label and, when possible, a concrete `next=diffship ...` follow-up command.

Inspect that guidance directly for either a run or a handoff bundle:

```bash
diffship explain --latest
diffship explain --run-id <run-id> --json
diffship explain --bundle ./diffship_YYYY-MM-DD_HHMM_<head7>
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
