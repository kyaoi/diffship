# diffship v1 Specification

This document is the **source of truth** for diffship v1 behavior.

**diffship is an AI-assisted development OS for Git repos.**

It supports two workflows:

1) **Handoff**: package Git diffs into an upload-friendly bundle with a single navigation document (`HANDOFF.md`).
2) **Ops**: safely apply an AI-produced **patch bundle** back onto a repo, run verification, and generate a reprompt bundle when something fails.

---

## 1. Goals

- **S-GOAL-001**: Produce a handoff bundle that stays within configurable upload limits (profile: max parts + max bytes per part).
- **S-GOAL-002**: Minimize context waste by shipping **diffs** (not full repo snapshots) plus a compact map.
- **S-GOAL-003**: Support both committed ranges and uncommitted work (staged/unstaged/untracked) as selectable sources.
- **S-GOAL-004**: Keep handoff output deterministic (same inputs → same ordering/parts).
- **S-GOAL-005**: Ensure TUI and CLI are equivalent; TUI can export a plan that CLI can replay.
- **S-GOAL-006**: Apply patch bundles **safely by default** (strict validation, lock, rollback).
- **S-GOAL-007**: Standardize run logs and reprompt bundles to support fast AI iteration loops.
- **S-GOAL-008**: Provide an **OS mode** that enables repeated apply/verify loops without requiring users to keep their main working tree clean.
- **S-GOAL-009**: Support configurable commit behavior (manual vs auto-commit) with both **global** and **project** configuration.
- **S-GOAL-010**: Provide `diffship init` to generate a “ChatGPT Project kit” file(s) that explains the expected workflow and contracts.

---

## 2. Non-goals (v1)

- **S-NONGOAL-001**: diffship does not generate patches or code; it only packages and applies bundles.
- **S-NONGOAL-002**: Perfect token counting (v1 uses heuristics).
- **S-NONGOAL-003**: Vendor-specific tuning (v1 uses generic profiles).
- **S-NONGOAL-004**: Remote automation (v1 does not push, open PRs, or manage remotes).

---

## 3. Terminology

- **handoff bundle**: output directory (default) and optional zip produced by `diffship build`.
- **part**: a split patch file `parts/part_XX.patch` inside a handoff bundle.
- **profile**: upload limits (e.g., 20 parts × 512 MiB).
- **segment**: diff source category:
  - committed range / staged / unstaged / untracked
- **unit**: the smallest packable diff chunk (file-level or commit-level depending on split mode).
- **plan**: serialized selection/options that can be replayed by CLI.

- **patch bundle**: input directory/zip consumed by `diffship apply` / `diffship loop`.
- **run**: a single ops invocation recorded under `.diffship/runs/<run-id>/`.
- **reprompt bundle**: a zip produced by `diffship pack-fix` containing logs + context to send back to an AI.
- **lock**: `.diffship/lock` used to prevent concurrent ops runs.

- **OS mode**: ops commands run in an **isolated worktree session** and promote results back to the user’s branch according to policy.
- **session**: a persistent local state for iterative ops runs (default: `default`).
- **sandbox worktree**: a per-run temporary worktree used to apply/verify safely.
- **promotion**: copying results from a session/sandbox back to the user’s working branch (e.g., `develop`).

---

## 3.1 CLI path handling

- **S-PATH-001**: Filesystem path arguments accepted by diffship CLI commands MUST treat a leading `~/` as the current user's `HOME` and MUST reject tilde-user shorthand.

---

## 4. Commands

### 4.1 `diffship` (no args) / `diffship tui`

- **S-TUI-001**: Running `diffship` with no args MUST start the TUI (same as `diffship tui`).
- **S-TUI-002**: TUI guides: range → sources → filters → split/profile → preview → build.
- **S-TUI-003**: TUI must preview diffs with an internal viewer (colored +/-).
- **S-TUI-004**: TUI must be able to export a plan file and show an equivalent CLI command.
- **S-TUI-005**: TUI edit mode MUST surface the current input buffer/help and allow editing handoff plan path and packing limit overrides with keyboard navigation.
- **S-TUI-006**: The handoff preview shown in TUI SHOULD surface canonical structured-context summary counts from `handoff.manifest.json` when that file is present in the temporary preview bundle.
- **S-TUI-007**: When the temporary preview bundle manifest provides `reading_order`, the TUI handoff preview SHOULD surface that canonical guidance in the prepended structured-context header before the first patch part.
- **S-TUI-008**: The TUI SHOULD provide a compare view that consumes `diffship compare --json` and surfaces canonical structured-context summary/reading-order deltas without inventing TUI-only comparison semantics.

### 4.2 `diffship build`

Builds a handoff bundle from a committed range and/or uncommitted sources.

#### 4.2.1 Sources (segments)

- **S-SOURCES-001**: Support selecting any combination of:
  - committed range (default ON)
  - staged
  - unstaged
  - untracked
- **S-SOURCES-002**: staged/unstaged/untracked are always based on the **current HEAD**; committed range uses the selected range.
- **S-SOURCES-003**: `HANDOFF.md` MUST clearly describe included segments and their bases (e.g., HEAD hash).

#### 4.2.2 Committed range modes

- **S-RANGE-001**: Support range modes:
  - `direct`: compare `from` and `to` directly (2-dot equivalent)
  - `merge-base`: compare `merge-base(a,b)` to `b` (3-dot equivalent)
  - `last`: `HEAD~1..HEAD`
  - `root`: empty tree → `to` (default `HEAD`)
- **S-RANGE-002**: Exactly one committed range mode is selected when committed is included.
- **S-RANGE-003**: Default committed range is `last`.

#### 4.2.3 Filters

- **S-FILTER-001**: Support repeatable `--include <glob>` and `--exclude <glob>`.
- **S-FILTER-002**: Support `.diffshipignore` (gitignore-like) as a default exclusion source.
- **S-FILTER-003**: Filters apply consistently to all included segments unless a segment-specific option is explicitly provided.

#### 4.2.4 Untracked handling

- **S-UNTRACKED-001**: Untracked is OFF by default; enabled via include-untracked (or TUI toggle).
- **S-UNTRACKED-002**: Support `--untracked-mode auto|patch|raw|meta`.
- **S-UNTRACKED-003**: In `auto`, text/small files become patch; large text files become raw attachment; binary files follow section 4.2.5 (default excluded unless `--include-binary`).
- **S-UNTRACKED-004**: In `patch`, untracked should be represented as add-diffs (e.g., `/dev/null → file`) when possible.
- **S-UNTRACKED-005**: In `raw`, untracked is bundled into `attachments.zip` under a stable path prefix.

#### 4.2.5 Binary handling

- **S-BINARY-001**: Binary content is excluded by default.
- **S-BINARY-002**: Support `--include-binary` with `--binary-mode raw|patch|meta` (default raw).
- **S-BINARY-003**: When included as raw, binary files are bundled into `attachments.zip`.

#### 4.2.6 Split mode

- **S-SPLIT-001**: Support `--split-by auto|file|commit`.
- **S-SPLIT-002**: `commit` split applies to committed range only; other segments remain file-level units.
- **S-SPLIT-003**: `auto` chooses commit split if committed range spans multiple commits; otherwise file split.

#### 4.2.7 Packing profiles

- **S-PROFILE-001**: Support named handoff packing profiles, including built-in `20x512` (default) and `10x100`.
- **S-PROFILE-002**: Support project/global config defaults and custom profile definitions for handoff packing limits. The profile catalog itself remains config-scoped rather than embedded into `plan.toml`.

#### 4.2.8 Output

- **S-OUT-001**: Default output is a directory `./diffship_<timestamp>_<head7>/`, where `<timestamp>` is formatted in the local system timezone as `YYYY-MM-DD_HHMM` and `<head7>` is the current repo `HEAD` short SHA; if that path already exists, diffship MUST choose the next available suffixed path (`_2`, `_3`, ...). `--out-dir <dir>` or `[handoff].output_dir` MAY change the parent directory of that generated bundle name, while `--out <path>` continues to set the exact output path. For these path-like options, a leading tilde-slash form such as `~/handoffs` MUST resolve against the user's `HOME`; tilde-user shorthand is rejected.
- **S-OUT-002**: `--zip` optionally produces a zip bundle with the same layout.
- **S-OUT-003**: The handoff bundle layout is defined in `docs/BUNDLE_FORMAT.md`.
- **S-OUT-004**: `HANDOFF.md` MUST be the primary entrypoint and contain a deterministic map to parts.
- **S-OUT-005**: `--zip-only` MUST produce only a `.zip` bundle. When `--out <path>` is used with `--zip-only`, the path MUST end in `.zip`; otherwise diffship MUST auto-name `diffship_<timestamp>_<head7>.zip` under the selected parent directory and avoid collisions with existing diffship bundle stems.
- **S-OUT-006**: `diffship build` MUST emit a deterministic root-level `handoff.manifest.json` that serves as the canonical machine-readable bundle summary (entrypoint, selected sources, filters, packing/warnings, parts/files index, and structured warnings/artifact metadata). Patch parts remain canonical for executable changes, and `HANDOFF.md` remains the primary human/LLM entrypoint.
- **S-OUT-007**: `diffship build` MUST emit a deterministic `parts/part_XX.context.json` alongside each patch part. These per-part JSON files MUST remain supplemental to the patch parts, identify the corresponding patch/context paths, summarize the part scope/files/stats/constraints, and be derivable only from local deterministic repository facts.
- **S-OUT-008**: `diffship build` MUST emit a deterministic root-level `handoff.context.xml` as a rendered view of the canonical JSON structured context. This XML view MUST remain supplemental to `handoff.manifest.json` and patch parts, and MUST be derived only from the same local deterministic repository facts.
- **S-OUT-009**: The canonical JSON structured-context outputs (`handoff.manifest.json` and `parts/part_XX.context.json`) MUST include aggregate category/segment/status counts derived from the selected rows so downstream tooling can reason about scope without parsing rendered text views.
- **S-OUT-010**: `handoff.manifest.json` MUST include a deterministic reading-order list derived from the selected rows so downstream tooling can reuse the same guidance without reparsing `HANDOFF.md`.

#### 4.2.9 Packing algorithm and fallback

- **S-PACK-001**: Packing is deterministic for the same inputs.
- **S-PACK-002**: Units are sorted by (1) bytes desc, (2) path/commit asc.
- **S-PACK-003**: Pack uses First-Fit Decreasing under profile constraints.
- **S-PACK-004**: If a unit cannot fit within `max_bytes_per_part`, fallback MUST attempt lower unified diff context levels (`U1`, then `U0`) before excluding it.
- **S-PACK-005**: Exclusions must be recorded in `excluded.md` with reasons and guidance.

#### 4.2.10 Plan export / replay

- **S-PLAN-001**: `diffship build --plan <file>` MUST replay a serialized handoff plan.
- **S-PLAN-002**: `diffship build --plan-out <file>` MUST export the resolved handoff plan in a replayable `plan.toml` format, including the selected `profile` name plus resolved numeric limits (but not the full profile catalog).

### 4.3 `diffship preview <handoff-bundle>`

- **S-PREVIEW-001**: Provide a simple viewer to browse `HANDOFF.md` and open parts/attachments references.
- **S-PREVIEW-002**: Support `--json` output for bundle summary (`--list`) and entry text (`HANDOFF.md` / `--part`) so CI can consume preview results.
- **S-PREVIEW-003**: When `handoff.manifest.json` is present, `diffship preview --list` and `--list --json` MUST surface the canonical structured-context summary (artifact presence plus aggregate category/segment/status counts) without reparsing rendered text views.
- **S-PREVIEW-004**: When `handoff.manifest.json` provides `reading_order`, `diffship preview --list` and `--list --json` MUST surface that canonical guidance alongside the structured-context summary.

### 4.3.1 `diffship compare <bundle-a> <bundle-b>`

- **S-COMPARE-001**: Compare two handoff bundles and report structural/content differences.
- **S-COMPARE-002**: Support normalized comparison mode for determinism checks and `--strict` extracted-entry byte mode (without text normalization). Raw zip container metadata equality is out of scope for the current v1 contract.
- **S-COMPARE-003**: Support `--json` output for machine-readable compare results while preserving non-zero exit on differences.
- **S-COMPARE-004**: Classify compare differences by area/kind in both human-readable and JSON output.
- **S-COMPARE-005**: When both bundles contain `handoff.manifest.json`, compare output MUST surface structured-context summary deltas derived from the canonical manifest JSON (for example file/category/segment/status count changes) in both human-readable and JSON output.
- **S-COMPARE-006**: When both bundles contain manifest `reading_order`, compare output MUST surface canonical reading-order deltas in both human-readable and JSON output.

### 4.4 Patch bundle format (input contract)

- **S-PBUNDLE-001**: A patch bundle is a directory or zip consumed by ops commands; its layout is defined in `docs/PATCH_BUNDLE_FORMAT.md`.
- **S-PBUNDLE-002**: `manifest.yaml` MUST exist and include: `protocol_version`, `task_id`, `base_commit`, `apply_mode`, `touched_files`.
- **S-PBUNDLE-003**: Patch bundle paths MUST be repo-relative and must not be absolute or contain path traversal (`..`).
- **S-PBUNDLE-004**: `changes/*.patch` MUST be text patches (UTF-8, LF) and MUST be ordered deterministically.
- **S-PBUNDLE-005**: Optional files (`summary.md`, `constraints.yaml`, `checks_request.yaml`, `commit_message.txt`) may be included and should be copied into run logs when present.
- **S-PBUNDLE-006**: Patch bundles MAY include a `tasks/` directory describing required user actions (see `docs/PATCH_BUNDLE_FORMAT.md`).
- **S-PBUNDLE-007**: Patch bundles MAY add new repo-relative files in standard `OPS_PATCH_BUNDLE` flow when the patch uses `/dev/null -> b/<path>` with `new file mode 100644|100755`; no separate mode is required.

### 4.5 `diffship apply <patch-bundle>`

Applies a patch bundle safely.

- **S-APPLY-001**: Must acquire an exclusive lock (see section 7) before applying.
- **S-APPLY-002**: Must refuse to run outside a Git repository.
- **S-APPLY-003**: In OS mode, apply MUST run in an isolated sandbox worktree; the user’s main working tree MUST NOT be mutated during apply/verify.
- **S-APPLY-004**: By default, apply MUST require `base_commit` to match the session HEAD (or the sandbox base) before applying.
- **S-APPLY-010**: `diffship apply` and `diffship loop` MAY accept a CLI-only `--base-commit <rev>` override that corrects a bad manifest base for that invocation, but the resolved override MUST still match the session HEAD before apply proceeds, and the effective base MUST be recorded in run artifacts.
- **S-APPLY-005**: Must enforce strict path guards (section 7) and refuse forbidden targets.
- **S-APPLY-006**: Must run a preflight check before mutating (e.g., `git apply --check` or an equivalent dry-run).
- **S-APPLY-007**: If apply fails after any mutation, must rollback automatically (safe defaults only).
- **S-APPLY-008**: Must write run logs under `.diffship/runs/<run-id>/` including apply result and errors.
- **S-APPLY-009**: If locally configured post-apply commands exist, apply MUST run them in the sandbox after the patch is applied, record logs under the run directory, and fail the apply/loop flow if any configured command fails. These commands are local config only and MUST NOT be loaded from the patch bundle.

### 4.6 Commit policy (manual / auto)

- **S-COMMIT-001**: diffship MUST support a commit policy: `manual` or `auto` (configurable).
- **S-COMMIT-002**: When `auto`, diffship MUST create a commit after a successful apply+verify using `commit_message.txt` if present, otherwise a deterministic fallback template.
- **S-COMMIT-003**: Commit policy MUST be configurable globally and per project, with CLI flags taking precedence.
- **S-COMMIT-004**: Patch format (`git-apply` vs `git-am`) MUST be independent from commit policy.
- **S-COMMIT-005**: If promotion mode is `commit`, commit policy MUST be `auto` (otherwise the run must be refused).

### 4.6 `diffship verify`

Runs verification commands (profiles) and records logs.

- **S-VERIFY-001**: Must support profiles `fast|standard|full` (built-in names) and allow selecting a profile.
- **S-VERIFY-002**: Verification runs only locally configured commands (not commands embedded in the patch bundle).
- **S-VERIFY-003**: Must record stdout/stderr per command and produce a machine-readable summary.
- **S-VERIFY-004**: Must exit non-zero if any command in the profile fails.

### 4.7 `diffship pack-fix`

Creates a reprompt bundle from the latest run.

- **S-PACKFIX-001**: Must bundle the latest run logs, the applied diff (if any), and the original patch bundle metadata.
- **S-PACKFIX-002**: Output must be a single zip that is safe to upload to an AI, and the default filename SHOULD include the run timestamp plus a short `HEAD`/base label so multiple bundles are distinguishable by basename alone.

### 4.8 `diffship loop <patch-bundle>`

Orchestrates apply → verify → (on failure) pack-fix.

- **S-LOOP-001**: Must run apply then verify; if verify fails, must run pack-fix automatically.
- **S-LOOP-002**: Must keep the same lock for the full loop.

### 4.9 `diffship status`

- **S-STATUS-001**: Must show lock state and recent runs (human-readable by default).
- **S-STATUS-002**: Must support `--json` output.
- **S-STATUS-003**: `diffship status --heads-only` MUST provide a concise human-readable view centered on repo/session/sandbox/run head information without changing the `--json` schema family.
- **S-STATUS-004**: `diffship runs --heads-only` MUST provide a concise human-readable run view centered on effective base and promoted head information without changing the `--json` schema family.

### 4.10 `diffship session repair`

- **S-SESSION-005**: diffship MUST provide a session repair command that can reseed a named session from the current repo HEAD without requiring the user to call `git update-ref` directly.
- **S-SESSION-006**: Session repair MUST refuse to proceed when live sandboxes still depend on that session.

### 4.11 `diffship doctor`

- **S-DOCTOR-001**: diffship MUST provide a doctor command that diagnoses stale or missing session/worktree state and prints concrete recovery commands.
- **S-DOCTOR-002**: `diffship doctor --fix` MAY apply only safe, explainable repairs; if issues remain after safe fixes, doctor MUST still report them.

### 4.12 `diffship cleanup`

- **S-CLEANUP-001**: diffship MUST provide a cleanup command that removes diffship-owned orphan workspaces under `.diffship/worktrees/`, including orphan sandboxes and orphan session worktrees.
- **S-CLEANUP-002**: cleanup MAY remove sandbox worktrees for runs that have already been promoted, but it MUST also remove the corresponding run-local `sandbox.json` so later auto-selection does not treat that sandbox as still available.
- **S-CLEANUP-003**: cleanup MUST support `--dry-run` preview mode and `--json` machine-readable output.
- **S-CLEANUP-004**: cleanup MUST support an opt-in mode that removes eligible run logs under `.diffship/runs/` when those runs are already promoted or orphaned. Removing such runs MUST NOT update the repository `HEAD` or diffship session refs.
- **S-CLEANUP-005**: cleanup MUST support an opt-in mode that removes diffship-owned build artifacts under `.diffship/artifacts/` while leaving user-selected outputs outside `.diffship/` untouched.

---

## 5. Handoff document requirements

- **S-HANDOFF-001**: `HANDOFF.md` MUST include a TL;DR (<= 10 lines) and a recommended reading order.
- **S-HANDOFF-002**: `HANDOFF.md` MUST include a “Change Map”:
  - changed tree
  - file table with part mapping
  - category summary (docs/config/src/tests/other)
- **S-HANDOFF-003**: `HANDOFF.md` MUST include a Parts Index (part → top files/segment/size).
- **S-HANDOFF-004**: If `split-by=commit`, include a commit-oriented section that maps commits → parts.

---

## 6. Secrets warnings (handoff build)

- **S-SECRETS-001**: Detect likely secrets and emit warnings (paths + reason; do not print secret values).
- **S-SECRETS-002**: Interactive flows must request confirmation to continue.
- **S-SECRETS-003**: Support `--yes` to continue non-interactively; support `--fail-on-secrets` for CI.

---

## 7. Ops safety policy

- **S-OPS-001**: Lock path is `.diffship/lock` (configurable) and must prevent concurrent ops runs.
- **S-OPS-002**: Lock must include enough metadata to diagnose stale locks (PID, start time, command).
- **S-OPS-003**: Forbidden prefixes must include `.git/` and `.diffship/` by default.
- **S-OPS-004**: Path checks must not allow absolute paths or `..` traversal, and must not rely on following symlinks.
- **S-OPS-005**: MVP must refuse by default: binary patches, submodule changes, existing-file mode changes, unsupported new-file modes, and rename/copy metadata.
- **S-OPS-006**: Configuration values MUST be resolved with precedence: CLI > patch bundle manifest > project config > global config > built-in defaults.
- **S-OPS-007**: Project/global config MAY define additional forbidden repo-relative path/glob patterns, and apply/loop MUST enforce them against both manifest paths and patch headers.
- **S-OPS-008**: Project-local forbid rules MAY live in a dedicated `.diffship/forbid.toml` file in addition to the main project config, and `diffship init` SHOULD generate that file as a starter template without overwriting existing content unless `--force`.

### 7.1 OS mode sessions & worktrees

- **S-SESSION-001**: Ops commands MUST operate on a named session (default: `default`).
- **S-SESSION-002**: diffship MUST store session state under `.diffship/` and MUST NOT pollute normal Git branches; use non-`refs/heads/*` refs (e.g., `refs/diffship/sessions/<name>`).
- **S-SESSION-003**: Each apply/loop run MUST use a sandbox worktree created from the session HEAD, and MUST remove it on success or failure (best-effort cleanup).
- **S-SESSION-004**: After a successful run, diffship MUST advance the session state to the new result.

### 7.2 Promotion policy

- **S-PROMOTE-001**: diffship MUST support promotion modes: `none`, `working-tree`, `commit`.
- **S-PROMOTE-002**: The promotion target (branch name) MUST be configurable (default: `develop`).
- **S-PROMOTE-003**: Promotion MUST be explicit and deterministic; it MUST never modify unrelated files.

### 7.3 Ops-side secrets & user tasks

- **S-OPS-SECRETS-001**: Ops MUST scan patch bundle contents and produced diffs/logs for likely secrets and MUST block promotion by default until user acknowledges.
- **S-OPS-SECRETS-002**: Ops MUST never print secret values; only paths and reasons.
- **S-OPS-TASKS-001**: If a patch bundle declares required user tasks, diffship MUST surface them prominently and MUST block promotion by default until the user acknowledges (use --ack-tasks).

---

## 8. Runs & logs

- **S-RUN-001**: Ops commands must create a new run directory under `.diffship/runs/<run-id>/`.
- **S-RUN-002**: Run directory must contain machine-readable summaries for apply and verify.
- **S-RUN-003**: pack-fix must be able to reconstruct a reprompt bundle using only the run directory.
- **S-RUN-004**: `status --json` and `runs --json` SHOULD surface effective base and promoted head information for runs when that data exists.
- **S-RUN-005**: New ops run directories MUST use a human-readable timestamp-plus-`HEAD` `run_id` (`run_YYYY-MM-DD_HHMMSS_<head7>`), with a deterministic suffix when collisions occur, while older run directories remain readable.
- **S-RUN-006**: Run directories MUST retain argv/stdout/stderr/duration metadata for diffship-spawned external commands in a machine-readable index plus per-command log files.
- **S-RUN-007**: Human-readable `diffship runs` / `diffship status` and their JSON summaries SHOULD surface the run directory plus direct `commands.json` / phase-directory paths when command logs exist, so users can inspect hook/verify output without manually reconstructing paths.

---

## 9. Exit codes (v1)

- **S-EXIT-000**: `0` success
- **S-EXIT-001**: `1` general error (invalid args, missing files)
- **S-EXIT-002**: `2` not a git repository / invalid range
- **S-EXIT-003**: `3` packing failed due to limits
- **S-EXIT-004**: `4` refused due to secrets warnings (when fail-on-secrets)

Ops-specific codes:
- **S-EXIT-005**: `5` refused due to dirty working tree
- **S-EXIT-006**: `6` refused due to base commit mismatch
- **S-EXIT-007**: `7` refused due to forbidden/invalid paths
- **S-EXIT-008**: `8` apply failed
- **S-EXIT-009**: `9` verify failed
- **S-EXIT-010**: `10` lock busy / concurrent run detected
- **S-EXIT-011**: `11` refused due to ops-side secrets warning (ack required)
- **S-EXIT-012**: `12` refused due to required user tasks not acknowledged
- **S-EXIT-013**: `13` promotion failed

---

## 10. `diffship init` (ChatGPT Project kit)

`diffship init` generates files that you can attach to a ChatGPT Project so the AI reliably follows the diffship contracts.

- **S-INIT-001**: `diffship init` MUST create `.diffship/` if missing.
- **S-INIT-002**: It MUST write a human-readable workflow guide derived from `docs/PROJECT_KIT_TEMPLATE.md` (written into `.diffship/` for user attachment) and a separate short `PROJECT_RULES.md` file that is suitable for direct copy/paste into external AI project-rule UIs.
- **S-INIT-003**: It MUST write a project config stub (e.g., `.diffship/config.toml`) without overwriting existing files unless `--force`.
- **S-INIT-004**: It MUST write an AI-targeted guide derived from `docs/AI_PROJECT_TEMPLATE.md` that explains diffship's workflow, expected artifacts, input file meanings, and non-file deliverables such as commit messages and user-task files, and it MUST append current repo metadata such as branch, HEAD, and active local forbid patterns.
- **S-INIT-005**: `diffship init --template-dir <dir>` MAY override template sources by reading `PROJECT_KIT_TEMPLATE.md` and `AI_PROJECT_TEMPLATE.md` from the specified directory before falling back to repository templates or built-in defaults.
- **S-INIT-006**: It MUST write `.diffship/.gitignore` so diffship-managed local state (such as handoffs, runs, worktrees, sessions, and lock files) stays under `.diffship/` without being committed by default, unless the user edits that ignore file explicitly.
- **S-INIT-007**: `diffship init --zip` MUST export a minimal rules kit zip containing `PROJECT_KIT.md`, `PROJECT_RULES.md`, `AI_GUIDE.md`, and machine-readable metadata. The default output path MUST be under `.diffship/artifacts/rules/`, based on the current `HEAD` when available and falling back to the current `run_id` otherwise. `--out <path>` MAY override that destination.
- **S-INIT-008**: Generated guides and config stub MUST include an immediately usable repository snapshot when discoverable, including repository identity, preferred promotion target, suggested read-first files, and starter commands.
- **S-INIT-009**: `diffship init --lang <en|ja>` MUST select the language of the generated `PROJECT_RULES.md` snippet and record the resolved language in the rules-kit metadata. The default language is `en`.
- **S-INIT-010**: `diffship init --refresh-forbid` MUST allow rewriting only `.diffship/forbid.toml` from current repo detections without requiring `--force` for unrelated generated files.
