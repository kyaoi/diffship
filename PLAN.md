# PLAN (diffship OS)

This file is the single source of truth for progress tracking as diffship evolves into an AI-assisted development OS.
It collects the current state, the next tasks, and the completion criteria so work can resume cleanly across chats.

## Related Documents

- Spec: `docs/SPEC_V1.md`
- Patch bundle contract: `docs/PATCH_BUNDLE_FORMAT.md`
- Project kit template: `docs/PROJECT_KIT_TEMPLATE.md`
- Config: `docs/CONFIG.md`
- Traceability: `docs/TRACEABILITY.md`
- Decision log: `docs/DECISIONS.md`

---

## Goals

The target state is that a user can run the following loop without needing to think about the internals:

```bash
# 1) handoff (diff -> AI bundle)
diffship build [options...]

# 2) ops (AI patch bundle -> apply/verify/promote)
diffship loop <patch-bundle.zip>
```

### Required outcomes on the ops side
- Keep the user's working tree clean via worktree/session/sandbox isolation.
- Run verify profiles (`fast`, `standard`, `full`).
- Perform promotion automatically on success.
- Stop with explicit warnings when secrets or required user actions are involved.

### Required outcomes on the handoff side
- Split Git diffs (committed/staged/unstaged/untracked) according to upload constraints and produce a bundle with an AI-readable entrypoint (`HANDOFF.md`).
- Respect `.diffshipignore` and secret warnings, and handle risky/large/binary files via exclusion or attachments.
- Produce the same bundle tree / zip bytes from the same inputs.

---

## Official Defaults (V1)

- OS mode: isolated worktrees (session + sandbox)
- Promotion: `commit`
- Commit policy: `auto`
- Verify profile: `standard`
- Safety: require a clean tree, require base commit match, enable path guards, enable locking

---

## Working Rules

- Always update this `PLAN.md` when progress changes.
- Record important decisions (default changes, safety policy changes) in `docs/DECISIONS.md`.
- If behavior changes, update `docs/SPEC_V1.md` and `docs/TRACEABILITY.md` in the same commit.
- After changes, always run:
  - `just docs-check`
  - `just trace-check`

---

## Status Definitions

- `todo`: not started
- `doing`: in progress
- `blocked`: blocked (record the reason)
- `done`: complete

---

## Milestones

### M0: OS spine (`init` / lock / runs)

| ID | Status | Description | Done Criteria |
|---|---|---|---|
| M0-01 | done | `diffship init` (project kit generation) | Creates `.diffship/`, safely skips existing `.diffship/PROJECT_KIT.md` / `.diffship/AI_GUIDE.md` / `config.toml`, overwrites them with `--force`, and ships guardrails that distinguish `OPS_PATCH_BUNDLE` / `NONOPS_EDIT_PACKAGE` / `ANALYSIS_ONLY`, explain missing-`base_commit` behavior, show the expected artifact trees explicitly, and standardize the default AI `git-am` author identity as `Diffship <diffship@example.com>` |
| M0-02 | done | Locking (prevent concurrent execution) | Creates `.diffship/lock` and refuses concurrent execution |
| M0-03 | done | Run persistence (run-id / logs) | Creates `.diffship/runs/<run-id>/run.json` and stores at least the `init` result (`init.json`); apply/verify extend this in M2 |
| M0-04 | done | M0 integration tests | `init` -> `status` -> `runs` succeeds on a temporary Git repo |

### M1: worktree / session / sandbox (keep the main tree clean)

| ID | Status | Description | Done Criteria |
|---|---|---|---|
| M1-01 | done | Session creation / reuse | Reliably reuses session worktrees under `.diffship/worktrees/` |
| M1-02 | done | Sandbox creation (per run) | Creates a sandbox associated with each run-id |
| M1-03 | done | Cleanup policy | Remains recoverable on failure/interruption and can be diagnosed via `status` |

### M2: apply -> verify -> promotion (commit)

| ID | Status | Description | Done Criteria |
|---|---|---|---|
| M2-01 | done | Patch bundle validation (structure / manifest / path) | Reliably rejects invalid bundles |
| M2-02 | done | `apply` (in sandbox) | Records apply success/failure under the run and rolls back on failure |
| M2-03 | done | `verify` (`standard`) | Runs profile checks and stores summaries under the run |
| M2-04 | done | `promotion=commit` | Creates a commit on verify success (message derived from the bundle) |
| M2-05 | done | `loop` (M2 integration) | `diffship loop` completes from success to commit |
| M2-06 | done | `pack-fix` (on verify failure) | `loop` automatically creates a reprompt zip when verify fails |

### M3: secrets / tasks (stop when it must stop)

| ID | Status | Description | Done Criteria |
|---|---|---|---|
| M3-01 | done | Secret detection -> stop promotion | Promotion always stops on risky findings unless explicitly acknowledged |
| M3-02 | done | Tasks bundle contract | `tasks/USER_TASKS.md` remains in the run and shows the user-required actions |

### M4: configuration (global / project / CLI / bundle)

| ID | Status | Description | Done Criteria |
|---|---|---|---|
| M4-01 | done | Config load precedence | Resolves settings in the order CLI > manifest > project > global > default |
| M4-02 | done | Promotion / commit-policy switching | Supports `--promotion` / `--commit-policy` and verifies `none` / `working-tree` / `commit` behavior separately |

### M5: TUI (visibility + execution support)

| ID | Status | Description | Done Criteria |
|---|---|---|---|
| M5-01 | done | TUI skeleton (start / exit / navigation) | `diffship` with no args starts the TUI, `q` / `Esc` exits safely, and non-TTY still shows help |
| M5-02 | done | Read-only status / runs viewer | Shows `status` / `runs` information, run details, apply/verify/promotion state, and errors / exit codes |
| M5-03 | done | Run artifact navigation (paths / tasks) | Surfaces run-dir and `tasks/USER_TASKS.md` paths clearly enough to copy/reference them |
| M5-04 | done | Launch `loop` from the TUI | Lets the user choose a bundle, start `loop`, and see progress / result / stop reason |
| M5-05 | done | CLI parity / tests (CI green) | Keeps the TUI as a thin CLI wrapper, adds smoke tests, and passes `clippy -D warnings` and `just ci` |

### M6: Handoff (diff -> AI bundle)

| ID | Status | Description | Done Criteria |
|---|---|---|---|
| M6-01 | done | `diffship build` (handoff bundle generation) | Supports `diffship build --help`, produces a minimal bundle, and matches `docs/BUNDLE_FORMAT.md` |
| M6-02 | done | Diff collection (committed / staged / unstaged / untracked) | Lets the user select segments and records each segment base in `HANDOFF.md` |
| M6-03 | done | Splitting (profiles) + excluded / attachments | Implements split / attachments / excluded and stops with `EXIT_PACKING_LIMITS` when `--max-parts` / `--max-bytes-per-part` are exceeded |
| M6-04 | done | `HANDOFF.md` generation (entrypoint) | Generates TL;DR / change map / parts index using `docs/HANDOFF_TEMPLATE.md` |
| M6-05 | done | Ignore + secrets warning (handoff side) | Respects `.diffshipignore`, reports secret-like content without leaking values, and can fail when needed |
| M6-06 | done | Determinism + tests | Produces deterministic ordering / splitting, ships golden tests, and passes `just ci` |
| M6-07 | done | Structured context Phase 1 manifest | `diffship build` emits deterministic `handoff.manifest.json` as the canonical machine-readable bundle summary while keeping patch parts canonical and `HANDOFF.md` as the primary entrypoint |
| M6-08 | done | Structured context Phase 1 per-part JSON | `diffship build` emits deterministic `parts/part_XX.context.json` files that summarize each patch part while remaining supplemental to the canonical patch payload |
| M6-09 | done | Structured context rendered XML view | `diffship build` emits deterministic `handoff.context.xml` as a rendered view of the canonical JSON structured context without changing the canonical patch payload |
| M6-10 | done | Structured context richer JSON facts | Canonical JSON outputs now expose aggregate category / segment / status counts so tools can reason about scope without parsing rendered text views |
| M6-11 | done | Structured context reading-order hint | `handoff.manifest.json` now carries the same deterministic reading-order guidance that `HANDOFF.md` renders so downstream tooling does not need to scrape the markdown entrypoint |

### M7: Ops ergonomics / recovery

| ID | Status | Description | Done Criteria |
|---|---|---|---|
| M7-01 | done | Base commit override for apply/loop | `diffship apply/loop --base-commit <rev>` can correct a bad manifest base only when the resolved SHA matches the current session head, and the effective base is recorded in run artifacts |
| M7-02 | done | Head-focused status / runs views | `diffship status --heads-only` and `diffship runs --heads-only` show repo/session/sandbox heads concisely without regressing `--json` output |
| M7-03 | done | Session repair command | A dedicated diffship command can reseed a session from the current repo HEAD without requiring manual `git update-ref`, and it refuses unsafe repair when live sandboxes still depend on the session |
| M7-04 | done | Doctor diagnostics + safe fixes | `diffship doctor` reports stale or missing session/sandbox state, prints exact recovery commands, and `--fix` only applies safe, explainable repairs |
| M7-05 | done | Human-readable run ids | New ops runs use timestamp-based run ids that remain collision-safe and compatible with existing UUID-based run directories |
| M7-06 | done | External command logs | apply/promote/verify/post-apply preserve argv/stdout/stderr/duration for diffship-spawned external commands under each run directory so hook output is inspectable |

### M8: Rules export / forbid policy

| ID | Status | Description | Done Criteria |
|---|---|---|---|
| M8-01 | done | Repo-configurable forbid patterns | Project/global config can declare extra forbidden patch targets (for example lockfiles), apply/loop enforce them, and generated guides mention the policy |
| M8-02 | done | Stronger project rules wording | `diffship init` generates deterministic `PROJECT_RULES.md` text for external AI project-rule UIs and keeps the longer guides (`PROJECT_KIT.md` / `AI_GUIDE.md`) alongside repo metadata |
| M8-03 | done | Rules kit zip export | `diffship init --zip` emits a minimal rules kit zip named from the current HEAD (or run-id fallback) containing `PROJECT_KIT.md`, `PROJECT_RULES.md`, `AI_GUIDE.md`, and metadata |

---

## Inventory Notes (2026-03-16)

- The ops core (`init` / `status` / `runs` / `apply` / `verify` / `promote` / `loop`, secrets/tasks/ack, config precedence) is operational.
- `pack-fix` is implemented with dedicated integration coverage.
- handoff covers build + source collection + split-by + packing fallback + `HANDOFF.md` generation + attachments/excluded/secrets + determinism.
- handoff now also emits deterministic `handoff.manifest.json` as the minimal Phase 1 structured context layer.
- handoff now also emits deterministic per-part context JSON files next to each patch part.
- handoff now also emits deterministic `handoff.context.xml` as a rendered bundle-level XML view.
- Packing fallback already implements context reduction (`U3 -> U1 -> U0`).
- Handoff `preview` / `compare` are implemented.
- `diffship preview --list` now surfaces canonical structured-context summary counts when `handoff.manifest.json` is present.
- `diffship preview --list` now also surfaces canonical manifest reading-order guidance when that field is present.
- `diffship compare` now also surfaces canonical manifest reading-order deltas when both bundles provide that field.
- `diffship compare` now surfaces canonical manifest-summary deltas when both bundles include `handoff.manifest.json`.
- Explicit handoff path filters (`--include` / `--exclude`) are implemented and editable from the TUI handoff screen.
- Handoff plan export / replay (`--plan-out` / `--plan`) is implemented and exportable from the TUI.
- Named handoff packing profiles (built-in `20x512` / `10x100` plus config default/custom profiles) are implemented.
- Verify supports custom command profiles via `[verify.profiles.*]`.
- The TUI includes a handoff screen (range / sources / filters / split / preview / build + equivalent CLI command) with plan export and improved input UX (edit buffer/help, plan path/max limits, Tab navigation).
- The TUI handoff preview now prepends canonical structured-context summary counts before the first patch part when the preview bundle includes `handoff.manifest.json`.
- The TUI handoff preview now also surfaces canonical manifest `reading_order` guidance when that field is present in the preview bundle manifest.
- The TUI now also includes a compare screen that wraps `diffship compare --json` and surfaces canonical manifest summary / reading-order deltas interactively.
- `diffship init` now writes a dedicated `.diffship/PROJECT_RULES.md` snippet for external AI project-rule UIs and supports `--lang en|ja` for that generated file.
- `diffship init` now also writes `.diffship/forbid.toml`, ops config loading merges that dedicated file as project-local forbid policy, and `init --refresh-forbid` can refresh only that file from current repo detections.
- `diffship runs`, `diffship status`, and the TUI run detail now surface external command-log counts/phases, and the CLI views also print direct run/log artifact paths so `apply` / `post-apply` / `verify` / `promote` logs are easier to trace from the run directory.
- `cleanup` now treats missing/invalid `run.json` as orphaned run metadata, so eligible run logs do not survive solely because the sandbox still exists.
- Canonical structured-context JSON now includes aggregate category / segment / status counts at both bundle and part level.
- Canonical structured-context JSON now also includes deterministic reading-order guidance at the bundle level.

## Next (priority order)

1. Treat additional compare/TUI polish as a v1.1 backlog item rather than a current blocker.
2. Revisit raw zip-container byte equality only if a concrete need appears in v1.1+.
3. If structured context expands again, prefer new machine-readable facts or consumers before adding more rendered views.

## Notes

- Add blockers, investigation logs, and design notes here when needed.
- 2026-03-07: default handoff output naming now uses local time and auto-suffixes collisions when `--out` is omitted.
- 2026-03-07: `--out-dir` can redirect the generated handoff bundle under a custom parent directory without replacing the auto-generated bundle name.
- 2026-03-07: `[handoff].output_dir` can set the default parent directory for auto-generated handoff bundles.
- 2026-03-07: leading `~/` is accepted for handoff output and plan paths; tilde-user shorthand remains unsupported.
- 2026-03-07: `ops.post_apply` can run local sandbox commands immediately after apply succeeds; failures stop `apply` / `loop` before promotion.
- 2026-03-07: leading `~/` is now accepted across filesystem path arguments (`build` / `preview` / `compare` / `apply` / `pack-fix`); tilde-user shorthand remains unsupported.
- Extracting a zip overlay can restore old mtimes, which may cause Cargo to skip rebuilds.
  - If a subcommand appears missing or similarly stale, try `cargo clean` and then `just ci`.
- In traceability, `Partial` should only be used when `TBD` remains on either the Tests or Code side.
- Reserved handoff exit codes should keep `#[allow(dead_code)]` until they are actually used.
- The M6-06 golden normalizer must preserve UTF-8. Hash placeholder replacement should operate on character boundaries, not raw bytes.

- 2026-03-07: `diffship init` templates now reserve `patchship_...` for valid ops bundles, use `DO_NOT_LOOP_nonops_...` for non-ops archives, and tell AIs to prefer `ANALYSIS_ONLY` over a misleading fallback zip when `base_commit` is missing.

- 2026-03-07: `diffship init` guide templates now include explicit tree examples for loop-ready patch bundles, non-ops packages, and analysis-only responses so humans and AIs can classify artifacts by structure before calling `loop`.
- 2026-03-08: `diffship init` templates now standardize AI-generated `git-am` author headers on `Diffship <diffship@example.com>` and tell repositories that prefer human commit authorship to use `git-apply` or an explicit author-reset step.
- 2026-03-16: `diffship init` now generates `.diffship/PROJECT_RULES.md` as a short copy/paste snippet for external AI project-rule UIs, and `--lang en|ja` selects that snippet language while the rules zip records the resolved language.
- 2026-03-16: `diffship init` now generates `.diffship/forbid.toml` as a dedicated local forbid-policy file, and ops config loading merges it automatically with `.diffship/config.toml`.
- 2026-03-16: `diffship init --refresh-forbid` now lets users refresh only `.diffship/forbid.toml` from current repo detections without forcing unrelated generated files.
- 2026-03-16: run summaries now surface `commands.json` coverage and logged phases, and the TUI run detail lists `commands.json` plus existing phase directories under the run.
- 2026-03-16: `diffship runs` and `diffship status` now also print direct `run_dir`, `commands.json`, and phase-directory paths for recent runs when command logs exist, and the JSON summaries expose the same paths.
- 2026-03-16: `cleanup` now classifies missing/invalid `run.json` as orphaned run metadata so `cleanup --include-runs` removes those run logs reliably.
- 2026-03-16: `diffship build` now emits deterministic `handoff.manifest.json` at the bundle root as the Phase 1 structured context summary; JSON is the canonical machine-readable layer while patch parts remain canonical for executable changes.
- 2026-03-16: canonical structured-context JSON now includes aggregate category / segment / status counts in `handoff.manifest.json` and `parts/part_XX.context.json`.
- 2026-03-16: `diffship build` now emits deterministic `parts/part_XX.context.json` files next to each patch part so part-level scope, stats, constraints, and warnings remain machine-readable without changing the canonical patch payload.
- 2026-03-16: `diffship build` now emits deterministic `handoff.context.xml` as a rendered view of the same structured context facts; JSON remains canonical and XML is view-only.
- 2026-03-17: `diffship preview --list` and `--list --json` now reuse `handoff.manifest.json` to surface structured-context artifact presence and aggregate category / segment / status counts without reparsing `HANDOFF.md`.
- 2026-03-17: `diffship preview --list` and `--list --json` now also surface manifest `reading_order` guidance directly from canonical JSON.
- 2026-03-17: `diffship compare` now reuses `handoff.manifest.json` to report structured-context summary deltas in both human-readable and JSON output when both bundles provide the manifest.
- 2026-03-17: `diffship compare` now also reports manifest `reading_order` deltas in both human-readable and JSON output when both bundles provide that guidance.
- 2026-03-17: the TUI handoff preview now reads the temporary bundle manifest and prepends the same structured-context summary counts before showing the first patch part.
- 2026-03-17: the TUI handoff preview now also prepends manifest `reading_order` guidance from canonical JSON when that field is present.
- 2026-03-17: the TUI now includes a compare screen that consumes `diffship compare --json` and renders canonical manifest summary / reading-order deltas without adding TUI-only compare logic.
- 2026-03-17: `handoff.manifest.json` now also carries deterministic reading-order hints so downstream tooling can reuse the same navigation guidance without reparsing `HANDOFF.md`.
