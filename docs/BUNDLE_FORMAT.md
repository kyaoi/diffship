# diffship Bundle Format (v1)

This document defines the **bundle contract** produced by `diffship build` and consumed by humans/LLMs (and by `diffship preview` / `diffship compare`).

---

## 1. Directory layout

```
diffship_YYYY-MM-DD_HHMM_<head7>/
  HANDOFF.md
  AI_REQUESTS.md
  WORKFLOW_CONTEXT.md
  workflow.context.json
  handoff.manifest.json
  handoff.context.xml
  project.context.json   # optional (when --project-context focused)
  PROJECT_CONTEXT.md     # optional rendered project-context view
  project_context/
    files/...            # optional focused text snapshots
  parts/
    part_01.patch
    part_01.context.json
    part_02.patch
    part_02.context.json
  excluded.md          # only when something is excluded
  attachments.zip      # only when raw attachments exist
  plan.toml            # optional (when exported)
```

A zip bundle (optional) contains the **same layout** at the root.

---

## 2. `HANDOFF.md` (primary entrypoint)

Human/LLM entrypoint: what the bundle represents and how to read it.

Must include:
- TL;DR + recommended reading order
- Included segments (committed/staged/unstaged/untracked) and bases (e.g., HEAD hash)
- Applied path filters (`.diffshipignore`, optional `--include`, optional `--exclude`) when present
- Change map:
  - changed tree
  - file table (path, status, segment, ins/del where available, bytes, part)
  - category summary (docs/config/src/tests/other)
- Parts index (part → top files, segment mix, approximate size)
- If split-by=commit: commit → parts mapping section

See `docs/HANDOFF_TEMPLATE.md` for a recommended structure.

## 2.1 `AI_REQUESTS.md`

Deterministic bundle-local hosted-AI request scaffold.

Must include:
- a deterministic reading order for the current bundle
- deterministic references to `WORKFLOW_CONTEXT.md` / `workflow.context.json` before deeper hosted-AI planning
- explicit output-mode guidance for analysis-only, plain text edits, and loop-ready `OPS_PATCH_BUNDLE` responses
- hard constraints such as patch-canonical handling and exact current-head / `base_commit` usage
- whether optional project-context artifacts are present and when to read them
- when focused project context is present, deterministic hints for how to read that bounded context (for example changed/supplemental counts and direct relationship hints for changed context files)
- deterministic patch-part guidance that points to `parts/part_XX.context.json` and summarizes part-local intent labels / segments / top files
- deterministic task-group execution guidance that reuses manifest task-group facts and focused project-context usage metadata when those facts exist
- that guidance may also reuse canonical task-group / part `review_labels` as strategy hints for behavioral vs mechanical vs verification-heavy work
- that guidance may also reuse canonical verification-focused facts such as task-group `verification_targets` / `verification_labels` and focused project-context `verification_relevance` / `verification_labels` to keep follow-up verification bounded
- that guidance may also reuse canonical task-group `widening_labels` so hosted AI can decide whether to stay patch-only or widen into related tests/config/docs/repo rules
- that guidance may also reuse focused project-context `edit_scope_role` facts so hosted AI can keep widened repo context read-only unless a file is explicitly marked as a write target

`AI_REQUESTS.md` is supplemental guidance. `HANDOFF.md` remains the primary human/LLM entrypoint, and patch parts remain canonical.

---

## 2.2 `WORKFLOW_CONTEXT.md` / `workflow.context.json`

Deterministic bundle-local workflow guidance derived from resolved workflow config and repo-local workflow sources.

Must include:
- the selected workflow profile
- the selected strategy mode
- the effective strategy default profile
- deterministic error-override mappings when configured
- deterministic repo-local source hints (for example `.diffship/WORKFLOW_PROFILE.md` or repo-local config stubs when present)
- profile-derived expectations for tests-first posture, docs/traceability sync, change scope, verify cadence, and things to avoid

`WORKFLOW_CONTEXT.md` is the rendered hosted-AI view.
`workflow.context.json` is the canonical machine-readable form for the same workflow facts.

These files are supplemental guidance. They do not replace `HANDOFF.md`, `AI_REQUESTS.md`, or patch parts.

---

## 3. `handoff.manifest.json`

Canonical machine-readable summary for the bundle.

Must be:
- UTF-8, LF, deterministic JSON
- rooted at the bundle top level as `handoff.manifest.json`
- supplemental to patch parts rather than a replacement for them

Must include at least:
- schema version
- `patch_canonical=true`
- entrypoint (`HANDOFF.md`)
- current workspace `HEAD`
- selected sources / split mode / binary + untracked policy
- committed range summary when committed input is present
- applied filters (`.diffshipignore`, include, exclude)
- packing profile / limits / reduced-context warnings
- artifact paths (`parts/*`, optional `attachments.zip`, optional `excluded.md`, optional `secrets.md`)
- optional project-context artifact paths (`project.context.json`, `PROJECT_CONTEXT.md`, `project_context/files/...`) when focused project context is enabled
- `AI_REQUESTS.md` as the deterministic bundle-local hosted-AI request scaffold
- `WORKFLOW_CONTEXT.md` / `workflow.context.json` as deterministic workflow-guidance artifacts
- parts index and file index
- aggregate row counts by category / segment / status
- deterministic reading-order guidance derived from the selected rows
- deterministic cross-part `task_groups` that cluster parts by shared intent-label sets
- task-group-level execution hints such as primary labels, related part-context paths, related focused-project-context files, suggested bounded read order, and deterministic risk hints
- task-group-level deterministic `review_labels` that help downstream AI choose a review/generation strategy before reparsing every patch
- task-group-level deterministic `verification_targets` that point to likely tests/config/policy surfaces using only canonical file and project-context facts
- task-group-level deterministic `verification_labels` that summarize the coarse verification strategy (for example test follow-up, config/policy follow-up, dependency validation, behavioral-regression watch, or lightweight sanity checks)
- task-group-level deterministic `widening_labels` that summarize whether hosted AI should stay patch-only or widen into related tests/config/docs/repo rules
- task-group-level deterministic `execution_labels` that summarize the coarse execution flow (for example patch-only vs widen-before-edit, rules-before-edit, or post-edit verification follow-up)
- task-group-level deterministic `task_shape_labels` that summarize whether a task is single-area vs cross-cutting and whether it likely deserves heavier review or verification attention
- task-group-level deterministic `edit_targets` and `context_only_files` that distinguish bounded write scope from read-only supporting context
- deterministic per-file semantic facts for each file entry (for example `language`, generated / lockfile / CI-tooling flags, and related test candidates derived from local repository facts)
- deterministic per-file coarse semantic labels for each file entry (for example `docs_only`, `config_only`, `test_only`, `generated_output_touch`, `lockfile_touch`, `ci_or_tooling_touch`, `import_churn`, `signature_change_like`, or `api_surface_like`)
- those coarse semantic labels may also include richer deterministic path-role hints such as `repo_rule_touch`, `dependency_policy_touch`, `build_graph_touch`, and `test_infrastructure_touch`
- deterministic per-file `change_hints` for each file entry (for example rename ancestry, attachment / exclusion routing, and reduced-context fallback flags derived from canonical row metadata)
- those file semantics may also expose reverse source/test relationship candidates when the changed path itself is test-like, plus related documentation/configuration candidates when local repository facts strongly suggest them
- structured warning summaries (for example exclusions and secret hits)

JSON is the canonical machine-readable structured-context format for v1. Rendered views MAY be added on top, but they do not replace patch parts or `HANDOFF.md`.

---

## 4. `handoff.context.xml`

Rendered XML view for the bundle-level structured context.

Must be:
- UTF-8, LF, deterministic XML
- rooted at the bundle top level as `handoff.context.xml`
- rendered from the same local deterministic facts as `handoff.manifest.json`

Typical contents:
- entrypoint and rendered-view references
- source/range/filter summary
- packing / warning summary
- artifact references
- part-level summary references (`patch_path`, `context_path`, top files)

This file is an AI-friendly view layer. `handoff.manifest.json` remains the canonical machine-readable source, and patch parts remain the canonical executable changes.

---

## 5. `parts/part_XX.patch`

- UTF-8, LF
- Deterministic ordering (see `docs/DETERMINISM.md`)
- Each part MUST contain clear segment markers (headers) so a reader can see which segment a hunk belongs to.
- When packing fallback is active, diff context MAY be reduced (`U1` / `U0`) to keep a unit inside the configured byte limit.

---

## 6. `parts/part_XX.context.json`

Supplemental machine-readable context for each patch part.

Must be:
- UTF-8, LF, deterministic JSON
- emitted next to the matching patch file (`parts/part_01.patch` → `parts/part_01.context.json`)
- derived from local deterministic repository facts only

Must include at least:
- schema version
- `patch_canonical=true`
- matching patch path and context path
- deterministic title / summary / intent text
- deterministic `intent_labels` for that part (for example source/docs/test/config updates, cross-area changes, reduced-context routing, or API/import-heavy touches)
- deterministic `review_labels` for that part (for example behavioral-change-like, mechanical-update-like, verification-surface-touch, related-test-review-needed, or repo-policy-touch)
- deterministic task-group linkage for that part (for example `task_group_ref`, `task_shape_labels`, `task_edit_targets`, and `task_context_only_files`) so the bounded task contract is still available from the per-part JSON alone
- selected segments for that part
- file list and basic diff stats for that part
- deterministic per-file semantic facts for each file entry (matching the canonical root manifest view for those paths)
- deterministic per-file coarse semantic labels for each file entry (matching the canonical root manifest view for those paths)
- deterministic per-file `change_hints` for each file entry (matching the canonical root manifest view for those paths)
- deterministic `scoped_context` heuristics for that patch part (for example hunk headers, symbol-like names, import-like references, and related test candidates)
- when available, deterministic per-file scoped hints inside `scoped_context` so symbol/import cues remain attributable to concrete changed paths
- aggregate row counts for that part by category / segment / status
- scope / constraints / warning metadata (for example reduced-context paths)

These files help AIs understand each patch part, but they do not replace the patch payload.

---

## 7. `excluded.md`

Must list excluded units with:
- identifier (path or commit)
- segment
- reason
- guidance (e.g., adjust profile, disable include, widen ignore, etc.)

---

## 8. `attachments.zip`

- Stores raw attachments (untracked/binary) under stable prefixes:
  - `untracked/<path>`
  - `binary/<path>`
  - `snapshot/<path>` (only if enabled)
- Binary entries are opt-in (`--include-binary`); default policy excludes binary content.
- `HANDOFF.md` MUST list what was attached and why.

## 8.1 `project.context.json` / `PROJECT_CONTEXT.md` / `project_context/files/...`

- Optional focused project-context pack for hosted AI handoff.
- `project.context.json` is the canonical machine-readable index for that supplemental pack.
- `PROJECT_CONTEXT.md` is the rendered human/AI view of the same deterministic selection.
- `project_context/files/...` mirrors selected repo-relative text snapshots under a stable prefix.
- `project.context.json` summary may also expose deterministic counts by selected-file category, priority, edit-scope role, verification relevance, and relationship kind.
- Each selected `project.context.json` file entry may also expose whether the file was directly changed, deterministic `usage_role`, `priority`, `edit_scope_role`, `verification_relevance`, `verification_labels`, `why_included`, `task_group_refs`, deterministic `context_labels`, deterministic semantic facts, and inbound/outbound relationship refs so the focused context can be inspected file-by-file as well as through the global relationship list.
- Selection stays bounded and deterministic:
  - seed from changed files already present in the handoff
  - add strongly related local files (for example source/test/docs/config candidates, root `README.md`, and generated `.diffship/*` guide files when present)
  - omit generated-like, binary/non-UTF-8, missing, oversized, or budget-exceeding files with explicit reasons recorded in `project.context.json`
- These artifacts remain supplemental. Patch parts stay canonical for executable changes.

---

## 9. `plan.toml` (optional)

- A replayable description of the handoff selection/options used to build the bundle.
- Export with `diffship build --plan-out <path>` (for example `<bundle>/plan.toml`).
- Replay with `diffship build --plan <path>`.
- Output path / output parent directory / zip emission are CLI-time concerns and may be supplied when replaying the plan.
- Current plan payload includes the selected `profile` name plus resolved numeric limit fields, so replay remains stable if config later changes.
- Named profile definitions themselves stay in config (`[handoff.profiles.*]` / `[profiles.*]`); `plan.toml` is an export of the chosen selection, not a profile catalog dump.
