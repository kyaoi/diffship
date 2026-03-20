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
| M2-07 | done | Post-apply normalization evidence | `post_apply.json` now records deterministic changed-path/category summaries and reprompt `PROMPT.md` surfaces them before verify-log instructions |

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
| M6-12 | done | Structured context file semantic facts | Canonical JSON file entries now expose deterministic language/path-semantic hints plus related test candidates so downstream tooling can reason about likely source/test/config relationships without parsing patch text |
| M6-13 | done | Structured context per-part scoped hints | Per-part context JSON now includes deterministic scoped hints such as hunk headers, symbol-like names, import-like references, and related test candidates derived from the part patch plus canonical file semantics |
| M6-14 | done | Preview inspection for richer JSON facts | `diffship preview --list` now surfaces lightweight semantic/scoped inspection signals so users can confirm richer canonical JSON facts are present without dumping the full JSON payload inline |
| M6-15 | done | Per-file scoped hints inside part context | Per-part scoped context now keeps symbol/import hints attributable to concrete changed paths instead of only bundle-level unions |
| M6-16 | done | Reverse source/test relationship hints | Canonical file semantics now also expose likely source candidates for changed test-like files, making source/test navigation bidirectional |
| M6-17 | done | Related docs/config hints | Canonical file semantics now also expose likely docs and config/build candidates for changed source/test files |
| M6-18 | done | Structured context change-hint facts | Canonical file entries now expose deterministic rename/attachment/exclusion/reduced-context hints so consumers do not need to scrape `note` strings or special `part` values |
| M6-19 | done | Preview inspection for change hints | `diffship preview --list` and `--list --json` now also report lightweight coverage for canonical `change_hints` facts |
| M6-20 | done | Focused project-context pack | `diffship build --project-context focused` now emits deterministic `project.context.json`, `PROJECT_CONTEXT.md`, and `project_context/files/...` snapshots seeded from changed files plus strongly related local files without changing the canonical patch payload |
| M6-21 | done | Preview inspection for project context | `diffship preview --list` and `--list --json` now also report focused project-context artifact presence and lightweight summary counts when those artifacts exist |
| M6-22 | done | Bundle-local AI request kit | `diffship build` now emits deterministic `AI_REQUESTS.md` so hosted AI requests can reuse bundle-local reading order, output-mode guidance, and loop-safety constraints without relying on copied boilerplate |
| M6-23 | done | Coarse semantic labels in canonical JSON | Canonical file semantic facts now also expose deterministic coarse labels such as docs/config/test touches plus import/signature/API-like hints so downstream AI tooling can triage change intent without parser-heavy extraction |
| M6-24 | done | Preview inspection for coarse semantic labels | `diffship preview --list` and `--list --json` now also report lightweight coverage for canonical coarse semantic labels |
| M6-25 | done | Richer per-file focused project context | `project.context.json` now exposes per-file semantic facts plus inbound/outbound relationship refs for each selected file so hosted AI can inspect surrounding repo context without flattening everything through the global relationships list |
| M6-26 | done | AI request guidance for focused project context | `AI_REQUESTS.md` now includes deterministic changed-context and direct-relationship hints from `project.context.json` so hosted AI can use the bounded repo context without inventing its own widening strategy |
| M6-27 | done | Part-level intent labels in per-part context | `parts/part_XX.context.json` now also exposes deterministic `intent_labels` derived from part-local categories, change hints, and canonical file semantics so hosted AI can classify patch-part role without reparsing the whole patch payload |
| M6-28 | done | Focused project-context role labels and graph summary | `project.context.json` now also exposes deterministic per-file `context_labels` plus summary counts by selected-file category and relationship kind so hosted AI can see why supplemental context files exist without inferring everything from raw paths |
| M6-29 | done | AI request guidance for patch-part intent | `AI_REQUESTS.md` now also summarizes per-part intent labels, segments, and top files so hosted AI can decide which `parts/part_XX.context.json` files to inspect before reparsing every patch part |
| M6-30 | done | Manifest-level cross-part task groups | `handoff.manifest.json` now also exposes deterministic `task_groups` that cluster patch parts by shared intent labels so hosted AI can find likely multi-part tasks without reclustering part contexts itself |
| M6-31 | done | Preview inspection for manifest task groups | `diffship preview --list` and `--list --json` now also surface canonical manifest `task_groups` so humans and downstream tooling can inspect likely multi-part task clusters without opening raw manifest JSON |
| M6-32 | done | Richer task-group execution hints | canonical manifest `task_groups` now also expose primary labels, related part-context paths, related focused-project-context files, suggested bounded read order, and deterministic risk hints so hosted AI can plan task-level reading without reclustering bundle facts |
| M6-33 | done | Focused project-context usage roles and priorities | focused `project.context.json` now also exposes deterministic file-level `usage_role`, `priority`, `why_included`, and `task_group_refs`, plus summary `priority_counts`, so hosted AI can read the bounded repo context by importance instead of as a flat file list |
| M6-34 | done | AI request execution recipe from canonical task graph | `AI_REQUESTS.md` now also reuses canonical task-group and focused project-context usage facts to emit a deterministic task-group execution recipe instead of only listing parts and generic reading advice |
| M6-35 | done | Review strategy labels in canonical JSON | per-part context JSON and manifest task groups now also expose deterministic `review_labels` so hosted AI can distinguish behavioral changes from mechanical updates and verification-heavy tasks before reparsing full patches |
| M6-36 | done | AI request review-strategy hints | `AI_REQUESTS.md` now also reuses canonical `review_labels` from task groups and part contexts so hosted AI keeps those structured strategy hints in scope while planning edits |
| M6-37 | done | Verification-focused canonical facts | manifest task groups now also expose deterministic `verification_targets`, and focused `project.context.json` file entries now also expose deterministic `verification_relevance` plus `verification_labels`, so hosted AI can keep likely tests/config/policy surfaces bounded |
| M6-38 | done | AI request verification guidance | `AI_REQUESTS.md` now also reuses canonical verification-focused facts so hosted AI sees deterministic verification-reading guidance before proposing local verification |
| M6-39 | done | Task-group verification strategy labels | canonical manifest `task_groups` now also expose deterministic `verification_labels` so hosted AI can tell whether a task needs test follow-up, config/policy follow-up, dependency validation, or only a lightweight sanity check |
| M6-40 | done | AI request verification-strategy hints | `AI_REQUESTS.md` now also reuses canonical task-group `verification_labels` so hosted AI sees coarse verification strategy alongside bounded verification targets |
| M6-41 | done | Task-group widening labels | canonical manifest `task_groups` now also expose deterministic `widening_labels` so hosted AI can tell whether to stay patch-only or widen into related tests/config/docs/repo rules |
| M6-42 | done | AI request widening-strategy hints | `AI_REQUESTS.md` now also reuses canonical task-group `widening_labels` so hosted AI sees a deterministic context-widening strategy alongside bounded read order |
| M6-43 | done | Task-group execution flow labels | canonical manifest `task_groups` now also expose deterministic `execution_labels` so hosted AI can tell whether to stay patch-only, widen before editing, review rules first, or bias toward post-edit verification |
| M6-44 | done | AI request execution-flow hints | `AI_REQUESTS.md` now also reuses canonical task-group `execution_labels` so hosted AI sees a deterministic coarse execution flow alongside review / verification / widening hints |
| M6-45 | done | Task-group shape labels | canonical manifest `task_groups` now also expose deterministic `task_shape_labels` so hosted AI can tell whether a task is single-area or cross-cutting and whether it likely needs heavier review / verification attention |
| M6-46 | done | AI request task-shape hints | `AI_REQUESTS.md` now also reuses canonical task-group `task_shape_labels` so hosted AI sees a deterministic coarse task-shape signal alongside execution / review / verification / widening hints |
| M6-47 | done | Task-group bounded write-scope hints | canonical manifest `task_groups` now also expose deterministic `edit_targets` and `context_only_files` so hosted AI can distinguish bounded write scope from read-only supporting context |
| M6-48 | done | AI request bounded write-scope hints | `AI_REQUESTS.md` now also reuses canonical task-group `edit_targets` and `context_only_files` so hosted AI sees deterministic write scope before proposing edits |
| M6-49 | done | Per-part task-group linkage | `parts/part_XX.context.json` now also exposes deterministic task-group linkage (`task_group_ref`, `task_shape_labels`, `task_edit_targets`, `task_context_only_files`) so per-part JSON still carries the bounded task contract |
| M6-50 | done | Richer file semantic path-role labels | canonical file semantic facts now also expose deterministic `repo_rule_touch`, `dependency_policy_touch`, `build_graph_touch`, and `test_infrastructure_touch` labels when those roles can be inferred from local paths alone |
| M6-51 | done | Focused project-context edit-scope roles | focused `project.context.json` now also exposes deterministic per-file `edit_scope_role` values plus summary `edit_scope_counts` so hosted AI can distinguish write targets from read-only support / rule / verification context inside the bounded focused pack |
| M6-52 | done | AI request focused edit-scope hints | `AI_REQUESTS.md` now also reuses focused project-context `edit_scope_role` facts so widened project context remains explicitly read-only unless diffship marked a selected file as a write target |

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
| M8-04 | done | Post-apply preset guidance in init | Generated `.diffship/config.toml` now frames `ops.post_apply` as a local normalizer and includes stack-oriented commented presets for common repo shapes |

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
- Canonical structured-context JSON file entries now also include deterministic semantic facts such as language classification, generated / lockfile / CI-tooling flags, and related test candidates derived from local repository facts.
- Per-part structured-context JSON now also includes deterministic scoped hints such as hunk headers, symbol-like names, import-like references, and related test candidates derived from the part patch plus canonical file semantics.
- `diffship preview --list` now also surfaces lightweight inspection signals for manifest file semantics and per-part scoped-context coverage.
- Per-part scoped context now also includes per-file symbol/import hints so downstream tooling can map scoped clues back to individual changed files.
- Canonical file semantics now also expose reverse source/test candidates so changed test-like files can point back to likely source paths.
- Canonical file semantics now also expose likely docs and config/build candidates derived from local repository facts.
- `diffship build --project-context focused` now also emits a deterministic focused project-context pack (`project.context.json`, `PROJECT_CONTEXT.md`, and `project_context/files/...`) seeded from changed files plus strongly related local files for hosted AI workflows.
- `diffship preview --list` now also reports focused project-context artifact presence and summary counts when those artifacts exist.
- `diffship build` now also emits deterministic `AI_REQUESTS.md` so each bundle carries its own hosted-AI request scaffold.
- Canonical file semantic facts now also include deterministic coarse labels such as docs/config/test touches plus import/signature/API-like hints derived only from path and patch heuristics.
- `diffship preview --list` now also reports whether those coarse semantic labels are present in the manifest and per-part context files.
- Focused project context now also carries per-file semantic facts, `changed` markers, and inbound/outbound relationship refs inside `project.context.json`, not just a flat file list plus a global relationships array.
- `AI_REQUESTS.md` now also reuses focused project-context counts and direct relationship hints so hosted AI can read changed context files first and widen scope in a bounded way.
- Per-part structured-context JSON now also includes deterministic `intent_labels` so hosted AI can classify likely patch-part roles before reparsing full patch payloads.
- Focused project context now also carries per-file `context_labels` plus summary counts by category and relationship kind so hosted AI can tell why supplemental files exist without guessing from path names alone.
- `AI_REQUESTS.md` now also includes deterministic patch-part guidance derived from per-part intent labels, so hosted AI can choose which `parts/part_XX.context.json` files to inspect first.
- `handoff.manifest.json` now also includes deterministic `task_groups` that cluster parts by shared intent labels, so hosted AI can identify likely multi-part tasks without reclustering canonical part facts itself.
- `diffship preview --list` and `--list --json` now also surface those manifest `task_groups` directly.
- manifest `task_groups` now also include deterministic primary labels, related context paths, related focused-project-context files, suggested bounded read order, and risk hints.
- focused `project.context.json` files now also include deterministic `usage_role`, `priority`, `why_included`, and `task_group_refs`, and the summary now includes `priority_counts`.
- focused `project.context.json` files now also include deterministic `edit_scope_role`, and the summary now includes `edit_scope_counts`; `AI_REQUESTS.md` reuses those roles so widened project context stays explicitly read-only unless diffship marked a file as a write target.
- `AI_REQUESTS.md` now also uses those richer task-group and focused-context facts to emit a deterministic task-group execution recipe for hosted AI.
- part contexts and manifest task groups now also include deterministic `review_labels` such as behavioral/mechanical/verification-focused hints.
- `AI_REQUESTS.md` now also surfaces those canonical `review_labels` directly as review/generation strategy hints for both task groups and patch parts.
- manifest task groups now also include deterministic `verification_targets`, and focused project-context file entries now also include `verification_relevance` / `verification_labels`.
- `AI_REQUESTS.md` now also reuses those canonical verification-focused facts so hosted AI can inspect likely verification surfaces before suggesting local checks.
- manifest task groups now also include deterministic `verification_labels` that summarize the coarse verification strategy for each task.
- `AI_REQUESTS.md` now also surfaces those task-group `verification_labels` next to bounded verification targets.
- manifest task groups now also include deterministic `widening_labels` that summarize whether hosted AI should stay patch-only or widen into related tests/config/docs/repo rules.
- `AI_REQUESTS.md` now also surfaces those task-group `widening_labels` next to bounded read order and related project files.
- manifest task groups now also include deterministic `execution_labels` that summarize whether hosted AI should stay patch-only, widen before editing, review rules first, or bias toward post-edit verification.
- `AI_REQUESTS.md` now also surfaces those task-group `execution_labels` next to review, verification, and widening hints.
- manifest task groups now also include deterministic `task_shape_labels` that summarize whether a task is single-area or cross-cutting and whether it likely deserves heavier review / verification attention.
- `AI_REQUESTS.md` now also surfaces those task-group `task_shape_labels` next to execution, review, verification, and widening hints.
- manifest task groups now also include deterministic `edit_targets` and `context_only_files` that separate bounded write scope from read-only supporting context.
- `AI_REQUESTS.md` now also surfaces those task-group `edit_targets` and `context_only_files` next to the execution recipe.
- part contexts now also expose deterministic task-group linkage (`task_group_ref`, `task_shape_labels`, `task_edit_targets`, `task_context_only_files`) so a hosted AI starting from one part can still recover bounded task scope.
- canonical file semantic facts now also expose deterministic `repo_rule_touch`, `dependency_policy_touch`, `build_graph_touch`, and `test_infrastructure_touch` labels when those roles can be inferred from local paths alone.
- `pack-fix` now also includes `post_apply.json` plus `post-apply/` logs when local post-apply hooks ran, and the reprompt instructions point to that evidence before verify logs.
- `post_apply.json` now also records deterministic changed-path/category summaries, and reprompt `PROMPT.md` repeats that evidence inline before verify-log guidance.
- `diffship init` now also generates stack-oriented commented `ops.post_apply` presets and explicitly frames post-apply as local normalization rather than AI-output repair.

## Next (priority order)

1. Consider additional canonical JSON consumers outside preview/compare/TUI only when a concrete workflow need appears.
2. If semantic extraction expands further, prioritize deterministic relationship/task facts over new rendered views.
3. Treat additional compare/TUI polish as a v1.1 backlog item rather than a current blocker.

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
- 2026-03-19: canonical JSON file entries now include deterministic semantic facts derived from path/repo-local heuristics, including language hints, generated/lockfile/tooling classification, and related test candidates.
- 2026-03-19: per-part context JSON now includes deterministic scoped hints derived from part patch text, including hunk-header text, symbol-like names, import-like references, and unioned related test candidates.
- 2026-03-19: `diffship preview --list` now reports whether manifest file semantics are present and how many part contexts expose semantic/scoped hints, keeping the human-readable output lightweight.
- 2026-03-19: per-part scoped context now also includes per-file symbol/import hints keyed by changed path.
- 2026-03-19: canonical file semantics now also include reverse source candidates for changed test-like files.
- 2026-03-19: canonical file semantics now also include related docs and config/build candidates for changed source/test files.
- 2026-03-20: canonical file semantics now also include deterministic coarse labels such as docs/config/test touches plus import/signature/API-like hints derived from path and patch heuristics.
- 2026-03-20: `diffship preview --list` now also reports lightweight coverage for canonical coarse semantic labels.
- 2026-03-20: focused project context now also includes per-file semantic facts, changed markers, and inbound/outbound relationship refs for each selected file.
- 2026-03-20: per-part context JSON now also includes deterministic `intent_labels` derived from part-local categories, change hints, and canonical file semantics.
- 2026-03-20: `AI_REQUESTS.md` now also includes focused project-context reading hints derived from `project.context.json`.
- 2026-03-20: focused project context now also includes per-file `context_labels` plus summary counts by selected-file category and relationship kind.
- 2026-03-20: `AI_REQUESTS.md` now also includes deterministic patch-part guidance derived from part intent labels, segments, and top files.
- 2026-03-21: `handoff.manifest.json` now also includes deterministic cross-part `task_groups` derived from shared per-part intent labels.
- 2026-03-21: `diffship preview --list` and `--list --json` now also surface manifest `task_groups`.
- 2026-03-21: task groups now also include deterministic `verification_targets`, and focused project-context file entries now also include `verification_relevance` / `verification_labels`.
- 2026-03-21: `AI_REQUESTS.md` now also reuses those verification-focused facts so hosted AI keeps likely tests/config/policy surfaces bounded while planning local verification.
- 2026-03-21: task groups now also include deterministic `verification_labels` that summarize the coarse verification strategy for each task.
- 2026-03-21: `AI_REQUESTS.md` now also surfaces those task-group `verification_labels` next to bounded verification targets.
- 2026-03-21: task groups now also include deterministic `widening_labels` that summarize whether hosted AI should stay patch-only or widen into related tests/config/docs/repo rules.
- 2026-03-21: `AI_REQUESTS.md` now also surfaces those task-group `widening_labels` next to bounded read order and related project files.
- 2026-03-21: task groups now also include deterministic `execution_labels` that summarize whether hosted AI should stay patch-only, widen before editing, review rules first, or bias toward post-edit verification.
- 2026-03-21: `AI_REQUESTS.md` now also surfaces those task-group `execution_labels` next to review, verification, and widening hints.
- 2026-03-21: task groups now also include deterministic `task_shape_labels` that summarize whether a task is single-area or cross-cutting and whether it likely deserves heavier review / verification attention.
- 2026-03-21: `AI_REQUESTS.md` now also surfaces those task-group `task_shape_labels` next to execution, review, verification, and widening hints.
- 2026-03-21: task groups now also include deterministic `edit_targets` and `context_only_files` that separate bounded write scope from read-only supporting context.
- 2026-03-21: `AI_REQUESTS.md` now also surfaces those task-group `edit_targets` and `context_only_files` next to the execution recipe.
- 2026-03-21: part contexts now also expose deterministic task-group linkage (`task_group_ref`, `task_shape_labels`, `task_edit_targets`, `task_context_only_files`).
- 2026-03-21: canonical file semantic facts now also expose deterministic `repo_rule_touch`, `dependency_policy_touch`, `build_graph_touch`, and `test_infrastructure_touch` labels.
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
