# diffship Usage Guide

This guide describes how to use the **current implementation** of diffship end to end.

It is intentionally practical:

- what to run
- when to run it
- which outputs to expect
- where to look when something fails

For the formal product contract, see `docs/SPEC_V1.md`.

---

## 1. What diffship does

diffship supports two connected workflows:

1. **handoff**: collect Git changes into an AI-friendly bundle
2. **ops**: apply an AI-produced patch bundle safely, verify it, and promote it

In short:

1. run `diffship build`
2. inspect with `diffship preview` or `diffship compare`
3. send the handoff bundle to AI
4. receive a patch bundle back
5. run `diffship loop`

For the human workflow of what to send to ChatGPT/Claude/Codex, which response format to request, and how to use the result afterward, see `docs/AI_HANDOFF_FLOW.md`.

---

## 2. Install and setup

### 2.1 Local install

```bash
cargo install --path .
```

Or run without installing:

```bash
cargo run -- <subcommand> ...
```

### 2.2 Developer setup for this repository

```bash
mise install
lefthook install
just ci
```

---

## 3. Command map

### 3.1 Handoff side

- `diffship build`
- `diffship preview`
- `diffship compare`

### 3.2 Ops side

- `diffship init`
- `diffship status`
- `diffship runs`
- `diffship apply`
- `diffship verify`
- `diffship promote`
- `diffship loop`
- `diffship pack-fix`

### 3.3 TUI

- `diffship`
- `diffship tui`

When running in a TTY, `diffship` starts the TUI by default.

---

## 4. Quickstart

### 4.1 Handoff quickstart

Build a bundle from your latest committed change:

```bash
diffship build
```

Inspect it:

```bash
diffship preview ./diffship_YYYY-MM-DD_HHMM_<head7> --list
diffship preview ./diffship_YYYY-MM-DD_HHMM_<head7> --part part_01.patch
```

Compare two bundles when checking reproducibility:

```bash
diffship compare ./bundle_a ./bundle_b.zip
diffship compare ./bundle_a ./bundle_b.zip --json
```

In the TUI handoff screen, preview now prepends the same canonical structured-context summary and manifest reading-order guidance that `diffship preview --list` shows when the temporary preview bundle includes `handoff.manifest.json`.

The TUI also includes a compare screen that wraps `diffship compare --json`, so interactive users can inspect canonical manifest summary / reading-order deltas without leaving the TUI.

### 4.2 Ops quickstart

Initialize a repository once:

```bash
diffship init
diffship init --zip
diffship init --zip --out ./.diffship/artifacts/rules/review-kit.zip
```

Run the full apply → verify → promote loop:

```bash
diffship loop ./patch-bundle.zip
diffship loop ./patch-bundle.zip --base-commit "$(git rev-parse HEAD)"
```

---

## 5. Handoff workflow in detail

### 5.1 Build from different sources

Only the latest committed change:

```bash
diffship build
```

Only staged / unstaged / untracked work:

```bash
diffship build --no-committed --include-staged --include-unstaged --include-untracked
```

Committed range with commit-oriented splitting:

```bash
diffship build --range-mode direct --from HEAD~3 --to HEAD --split-by commit
```

Create only a zip bundle:

```bash
diffship build --zip-only
diffship build --zip-only --out ./handoff.zip
```

### 5.2 Filter paths

Apply the same filters across committed, staged, unstaged, and untracked segments:

```bash
diffship build --include 'src/*.rs' --include '*.md' --exclude 'src/generated.rs'
```

Ignore rules from `.diffshipignore` are also applied.

### 5.3 Control untracked and binary handling

Untracked files can be represented as patch, raw attachment, or metadata:

```bash
diffship build --no-committed --include-untracked --untracked-mode meta
```

Binary content is excluded by default. To include it:

```bash
diffship build --include-binary --binary-mode raw
```

Supported binary modes:

- `raw`: store file bytes in `attachments.zip`
- `patch`: keep patch text when possible
- `meta`: record metadata / exclusion information instead of bytes

### 5.4 Packing limits and profiles

Use a built-in profile:

```bash
diffship build --profile 10x100
```

Override the resolved limits directly:

```bash
diffship build --max-parts 10 --max-bytes-per-part 104857600
```

Current built-in profiles:

- `20x512`
- `10x100`

If packing overflows, diffship currently:

1. repacks deterministically
2. reduces diff context from `U3` to `U1` to `U0` when needed
3. records exclusions in `excluded.md` if units still do not fit

### 5.5 Secrets behavior

If secrets-like content is detected, diffship warns before completing the build.

Continue non-interactively:

```bash
diffship build --yes
```

Fail instead of continuing:

```bash
diffship build --fail-on-secrets
```

### 5.6 Inspect the result

List bundle contents:

```bash
diffship preview ./diffship_YYYY-MM-DD_HHMM_<head7> --list
```

If the bundle includes `handoff.manifest.json`, the list view also surfaces the canonical structured-context summary, including aggregate category / segment / status counts and the manifest's reading-order guidance.

That canonical manifest JSON now also includes deterministic cross-part `task_groups` that cluster parts by shared `intent_labels`, so downstream AI tooling can spot likely multi-part tasks without reclustering part contexts on its own.

Those canonical task groups now also carry `primary_labels`, `related_context_paths`, `related_project_files`, `suggested_read_order`, and `risk_hints`, which gives hosted AI a bounded task-level reading plan before it starts inventing one from raw files.

Those same task groups now also carry `review_labels`, so hosted AI can tell whether a multi-part task looks behavioral, mechanical, verification-heavy, or policy-sensitive before it reparses every part.

They now also carry deterministic `verification_targets`, so hosted AI can keep likely tests/config/policy surfaces bounded instead of improvising verification scope from raw filenames.

They now also carry deterministic `verification_labels`, so hosted AI can tell whether a task needs test follow-up, config/policy follow-up, dependency validation, behavioral-regression review, or only a lightweight sanity check.

They now also carry deterministic `widening_labels`, so hosted AI can tell whether to stay patch-only or widen into related tests/config/docs/repo rules before it starts reading extra context.

They now also carry deterministic `execution_labels`, so hosted AI can keep a coarse execution flow in view instead of improvising whether to stay patch-only, widen before editing, review rules first, or plan post-edit verification.

They now also carry deterministic `task_shape_labels`, so hosted AI can tell whether a task is single-area or cross-cutting and whether it likely deserves heavier review or verification attention before reparsing more of the bundle.

They now also carry deterministic `edit_targets` and `context_only_files`, so hosted AI can separate bounded write scope from read-only supporting context instead of guessing which related files are safe to edit.

Per-part `parts/part_XX.context.json` files now also carry deterministic task-group linkage such as `task_group_ref`, `task_shape_labels`, `task_edit_targets`, and `task_context_only_files`, so a model that starts from one part context can still recover the bounded task contract without reopening the whole manifest first.

Canonical file semantic facts now also expose deterministic path-role hints such as `repo_rule_touch`, `dependency_policy_touch`, `build_graph_touch`, and `test_infrastructure_touch`, so hosted AI can distinguish rule/build/test-support surfaces without parser-heavy analysis.

`diffship preview --list` and `--list --json` now also surface those manifest task groups directly, so you can confirm the bundle's task clustering without opening `handoff.manifest.json` by hand.

If the bundle also includes the optional focused project-context pack, the list view surfaces `project.context.json` / `PROJECT_CONTEXT.md` presence plus lightweight counts for selected files, included snapshots, omitted files, and total snapshot bytes.

The list view also reports whether `AI_REQUESTS.md` is present so you can confirm the bundle carries its deterministic hosted-AI request scaffold.

The canonical JSON file entries also include deterministic semantic facts such as `language`, generated / lockfile / CI-tooling flags, and related test candidates derived from local repository facts.

Those same canonical file entries now also include deterministic coarse semantic labels such as `docs_only`, `config_only`, `test_only`, `generated_output_touch`, `lockfile_touch`, `ci_or_tooling_touch`, `import_churn`, `signature_change_like`, and `api_surface_like`.

Per-part `parts/part_XX.context.json` files also carry a deterministic `scoped_context` section with hints such as hunk-header text, symbol-like names, import-like references, and related test candidates.

That `scoped_context` section now also includes per-file entries so symbol/import hints remain attributable to specific changed files when a patch part touches more than one path.

Those same per-part JSON files now also expose deterministic `intent_labels` such as `source_update`, `docs_update`, `cross_area_change`, `api_surface_touch`, `import_churn`, `rename_or_copy`, and `reduced_context`.

They now also expose deterministic `review_labels` such as `behavioral_change_like`, `mechanical_update_like`, `verification_surface_touch`, `needs_related_test_review`, and `repo_policy_touch`.

File semantics are now also bidirectional for source/test navigation: source files can point to likely tests, and changed test-like files can point back to likely source files.

Those same file semantics can also point to likely docs and config/build files such as `README.md`, `docs/*.md`, `Cargo.toml`, `pyproject.toml`, `package.json`, or `tsconfig.json` when matching local files exist.

Canonical file entries also expose deterministic `change_hints` for common handling outcomes such as rename ancestry, `attachments.zip` routing, `excluded.md` routing, and reduced-context fallback.

### 5.7 Optional focused project context for hosted AI handoff

Enable a bounded supplemental context pack:

```bash
diffship build --project-context focused
```

This adds:

- `project.context.json` as the canonical machine-readable index
- `PROJECT_CONTEXT.md` as the rendered view
- `project_context/files/...` as deterministic text snapshots of selected local files

Selection is intentionally bounded. diffship seeds from changed files and adds only strongly related local files such as likely tests, likely source files for changed tests, likely docs/config files, root `README.md`, and generated `.diffship/*` guide files when they exist.

Each selected file entry in `project.context.json` now also carries a `changed` marker, deterministic semantic facts, and inbound/outbound relationship refs. That lets hosted AI consumers inspect the focused context file-by-file instead of relying only on the flat top-level relationship list.

Focused project-context file entries now also expose `usage_role`, `priority`, `edit_scope_role`, `verification_relevance`, `verification_labels`, `why_included`, and `task_group_refs`, and the summary includes `priority_counts` plus `edit_scope_counts`. That lets hosted AI read the bounded repo context by importance, write-scope, and verification relevance instead of treating every supplemental file as equally relevant.

The context pack is supplemental. Patch parts remain canonical, and `none` remains the default so diff-first handoff is unchanged unless you opt in.

### 5.8 Bundle-local AI request scaffold

Every new handoff bundle now includes `AI_REQUESTS.md`.

Use it when forwarding the bundle to a hosted AI. It gives you:

- the current bundle's deterministic reading order
- explicit output-mode guidance for analysis-only, plain text edits, and loop-ready patch bundles
- hard constraints such as exact current-head usage for `base_commit`
- whether optional project-context artifacts are present and when to read them
- patch-part guidance that points to `parts/part_XX.context.json` with intent labels, segments, and top files

When focused project context exists, `AI_REQUESTS.md` now also includes changed/supplemental counts plus direct relationship hints for changed context files. That gives the hosted AI a bounded widening strategy before it starts inventing its own repo walk.

Focused project context now also includes per-file `context_labels` such as `changed_target`, `supplemental_context`, `source_context`, `doc_context`, `related_context`, `repo_guide_context`, `relationship_source`, and `relationship_target`, plus summary counts by selected-file category and relationship kind.

`AI_REQUESTS.md` now also includes deterministic patch-part guidance such as ``parts/part_01.patch`` plus its matching context path, `intent_labels`, segments, and top files. That lets the hosted AI inspect the most relevant per-part JSON first instead of reparsing every patch part in full.

It now also includes a deterministic task-group execution recipe derived from manifest `task_groups` plus focused project-context usage metadata. That recipe points the hosted AI at the right part-context files first, then the matching patch parts, then only the focused project-context snapshots that diffship marked as relevant.

That same scaffold now also reuses canonical `review_labels` from both task groups and patch parts, so the hosted AI keeps behavioral/mechanical/verification strategy hints in view while deciding how deeply to reason about each change.

It now also reuses canonical `verification_targets` plus focused project-context `verification_relevance` / `verification_labels`, so the hosted AI sees a bounded verification-reading plan before it proposes local checks or follow-up fixes.

It now also reuses task-group `verification_labels`, so that bounded verification-reading plan comes with a coarse verification strategy instead of just a flat list of files.

It now also reuses task-group `widening_labels`, so that same scaffold carries a deterministic context-widening strategy instead of leaving the model to improvise how far it should walk beyond the patch.

It now also reuses task-group `execution_labels`, so that same scaffold carries a deterministic coarse execution flow instead of leaving the model to improvise when to widen, review rules, or bias toward post-edit verification.

It now also reuses task-group `task_shape_labels`, so that same scaffold carries a deterministic coarse task-shape signal instead of leaving the model to improvise whether a task is single-area, cross-cutting, review-heavy, or verification-heavy.

It now also reuses task-group `edit_targets` and `context_only_files`, so that same scaffold tells the model which files are the bounded write scope and which related files should remain read-only context unless the task explicitly broadens scope.

When focused project context exists, that same scaffold now also reuses per-file `edit_scope_role` values from `project.context.json`, so widened repo context stays explicitly read-only unless diffship marked a selected file as a `write_target`.

Show a patch part:

```bash
diffship preview ./diffship_YYYY-MM-DD_HHMM_<head7> --part part_01.patch
```

Machine-readable preview:

```bash
diffship preview ./diffship_YYYY-MM-DD_HHMM_<head7> --list --json
```

The JSON list output includes a `structured_context` object when canonical structured-context files are present, including the manifest `reading_order` when available. It now also reports lightweight semantic/coarse-label/change-hint/scoped inspection coverage, while the canonical bundle JSON files themselves keep the full richer facts for downstream tooling.

The TUI handoff preview uses the same temporary bundle and now prepends the canonical structured-context summary before the first patch part when the manifest JSON is present.

### 5.7 Export and replay a plan

Export:

```bash
diffship build --include-untracked --plan-out ./diffship_plan.toml
```

Replay:

```bash
diffship build --plan ./diffship_plan.toml --out ./replayed_bundle
```

Current `plan.toml` behavior:

- stores the selected profile name
- stores resolved numeric limits
- stores handoff selection such as range, sources, filters, split mode, and binary mode
- does not store runtime output routing such as `--out` or `--out-dir`
- does not store the entire named profile catalog

Named profile definitions stay in config, not in the plan export.

### 5.8 Handoff output layout

Typical output:

- `HANDOFF.md`
- `handoff.manifest.json`
- `handoff.context.xml`
- `parts/part_XX.patch`
- `parts/part_XX.context.json`
- `attachments.zip` when raw attachments are included
- `excluded.md` when diff units are intentionally omitted
- `secrets.md` when secrets-like content is detected
- `plan.toml` when exported

The manifest JSON also includes deterministic reading-order guidance derived from the selected rows, so automation can reuse the same navigation hints without scraping `HANDOFF.md`.

Default output naming:

- `--out <path>` sets the exact output directory path
- `--out-dir <dir>` places the auto-generated bundle name under a custom parent directory
- `--zip` keeps the directory output and also writes a sibling zip bundle with the same generated base name
- `--zip-only` writes only a zip bundle; with `--out`, the path must use a zip filename
- filesystem path arguments such as bundle paths, `--out`, `--out-dir`, `--plan`, and `pack-fix --out` accept leading tilde-slash
- if `--out` is omitted, diffship uses a `diffship_YYYY-MM-DD_HHMM_<head7>` directory name
- the timestamp is rendered in the local system timezone
- if the base path already exists, diffship creates a suffixed name such as `diffship_YYYY-MM-DD_HHMM_<head7>_2`, then `_3`, and so on

Example:

```bash
diffship build --out-dir ./.diffship/artifacts/handoffs
```

This produces a bundle under `./.diffship/artifacts/handoffs/` while keeping the generated bundle name.

Project or global config can also set this default:

```toml
[handoff]
output_dir = "./.diffship/artifacts/handoffs"
```

Tilde-slash paths are also accepted here:

```toml
[handoff]
output_dir = "~/ghq/github.com/kyaoi/diffship/.diffship/handoffs"
```

diffship expands that path against the current user's `HOME`. Tilde-user shorthand is intentionally unsupported.

See `docs/BUNDLE_FORMAT.md` for the bundle contract.

---

## 6. Ops workflow in detail

### 6.1 Initialize a repository

```bash
diffship init
```

This writes:

- `.diffship/.gitignore`
- `.diffship/PROJECT_KIT.md`
- `.diffship/PROJECT_RULES.md`
- `.diffship/AI_GUIDE.md`
- `.diffship/forbid.toml`
- `.diffship/config.toml`

To generate a Japanese paste-ready rules snippet for an external AI project UI:

```bash
diffship init --lang ja
```

`PROJECT_RULES.md` is the shortest generated file and is intended for direct copy/paste into project-rule or custom-instructions fields.

To use project-specific init templates:

```bash
diffship init --template-dir ./templates/diffship
```

The directory may contain either or both of:

- `PROJECT_KIT_TEMPLATE.md`
- `AI_PROJECT_TEMPLATE.md`

Missing files fall back to the repository templates and then to built-in defaults.

`AI_PROJECT_TEMPLATE.md` is intentionally split into:

- core contract sections that should stay aligned with diffship behavior
- "Customize this section" blocks for repository-specific rules, commands, directory ownership, and ready-to-send prompts

That makes it practical to keep one stable diffship contract while still generating a repo-specific `.diffship/AI_GUIDE.md`.

`PROJECT_KIT_TEMPLATE.md` follows the same pattern for the human-facing guide:

- core workflow sections describe the default diffship loop
- "Customize this section" blocks hold repo-specific commands, ownership boundaries, and operating rules

That keeps `.diffship/PROJECT_KIT.md` useful as a local onboarding document instead of a copy of generic product docs.

`PROJECT_RULES.md` is generated separately so the short external-AI rules text stays concise even when `PROJECT_KIT.md` and `AI_GUIDE.md` grow more detailed.

`forbid.toml` is the dedicated local file for `[ops.forbid]` patterns. If lockfiles or similar fragile files already exist in the repo, `diffship init` can prefill matching entries there.
If new fragile files appear later, run `diffship init --refresh-forbid` to rewrite only `.diffship/forbid.toml` from the latest detections.

The generated `.diffship/config.toml` now follows the same idea:

- core defaults stay close to the repository's actual diffship workflow
- "Customize this section" comments show where to set repo-specific defaults such as verify profile, handoff profile, output directory, promotion mode, and post-apply commands
- the generated handoff `output_dir` defaults to `./.diffship/artifacts/handoffs` so diffship-owned outputs stay under `.diffship/` after `diffship init`
- the generated config stub now also includes stack-oriented commented `ops.post_apply` presets and explicitly frames post-apply as local normalization rather than AI-output repair

### 6.2 Full loop

```bash
diffship loop ./patch-bundle.zip
```

What happens:

1. acquire the repo lock
2. create or reuse a session
3. create a sandbox worktree for the run
4. apply the patch bundle
5. run configured ops.post_apply commands, if any
6. run verification
7. promote if verification succeeds
8. persist run logs under `.diffship/runs/<run-id>/`

When a run executes external commands, `diffship runs` and `diffship status` show `commands=<n>` plus the recorded phases, and they also print direct `run_dir`, `commands.json`, and phase-directory paths so you can open the relevant logs immediately. The run directory still keeps the detailed `commands.json` index and per-phase log folders.

### 6.3 Use individual ops commands

Apply only:

```bash
diffship apply ./patch-bundle.zip
diffship apply ./patch-bundle.zip --base-commit "$(git rev-parse HEAD)"
```

Verify a specific run:

```bash
diffship verify --run-id <run-id> --profile standard
```

Promote a specific run:

```bash
diffship promote --run-id <run-id>
```

List runs:

```bash
diffship runs
diffship runs --heads-only
diffship runs --json
```

Show overall status:

```bash
diffship status
diffship status --heads-only
diffship status --json
```

Repair a stale session after manual commits:

```bash
diffship session repair --session default
diffship doctor --session default
diffship doctor --session default --fix
```

Clean up unused diffship workspaces and artifacts:

```bash
diffship cleanup --dry-run
diffship cleanup
diffship cleanup --include-runs
diffship cleanup --include-builds
diffship cleanup --all
diffship cleanup --json
```

`--include-runs` and `--all` remove terminal run directories such as promoted runs, `promotion=none` runs, failed promotions, failed verifies, and orphaned runs, but keep runs that still require an explicit follow-up acknowledgement before promotion.

### 6.4 Promotion modes

Available promotion modes:

- `commit`
- `working-tree`
- `none`

Examples:

```bash
diffship loop ./patch-bundle.zip --promotion none
diffship loop ./patch-bundle.zip --promotion working-tree
diffship loop ./patch-bundle.zip --promotion commit --commit-policy manual
```

### 6.5 Verification profiles

Built-in names:

- `fast`
- `standard`
- `full`

You can also define custom local profiles in config and run them by name.

### 6.6 When verify fails

diffship writes a reprompt bundle under the run directory:

- `.diffship/runs/<run-id>/pack-fix_YYYY-MM-DD_HHMMSS_<head7>[_N].zip`

You can recreate it manually:

```bash
diffship pack-fix --run-id <run-id>
```

You can inspect the same failure-aware guidance locally:

```bash
diffship strategy --run-id <run-id>
diffship strategy --latest --json
```

When local `ops.post_apply` hooks ran, the reprompt zip also includes `run/post_apply.json` plus `run/post-apply/*`, and the generated `PROMPT.md` points the AI at that evidence before the verify logs.

`run/post_apply.json` now also records deterministic `changed_paths`, coarse `change_categories`, and a machine-readable normalization summary derived from sandbox state before and after post-apply. The reprompt `PROMPT.md` repeats that summary inline so the AI sees local normalization effects before it starts interpreting verify failures.

### 6.7 Acknowledgement gates

Promotion may require explicit acknowledgement:

- `--ack-secrets`
- `--ack-tasks`

Examples:

```bash
diffship loop ./patch-bundle.zip --ack-secrets
diffship promote --run-id <run-id> --ack-tasks
```

See `docs/OPS_WORKFLOW.md` for the ops-focused walkthrough.

---

## 7. TUI usage

Start the TUI:

```bash
diffship
```

or:

```bash
diffship tui
```

Current screens:

- Runs
- Status
- Loop
- Handoff

Current handoff screen capabilities:

- range selection
- source toggles
- include / exclude filters
- split mode selection
- named profile cycling
- packing limit overrides
- plan path editing
- preview
- build
- equivalent CLI command display
- plan export

The TUI and CLI are intended to stay equivalent. If a handoff option matters, there should be a CLI representation for it.

---

## 8. Configuration

Resolution order:

1. built-in defaults
2. global config
3. project config
4. patch bundle manifest, when applicable
5. CLI flags

In practical terms:

- ops settings may be influenced by patch bundle `manifest.yaml`
- handoff packing profiles are resolved from config and CLI
- CLI flags always win

Current config files:

- global: HOME config under the standard diffship config path
- project: `.diffship.toml`
- project: `.diffship/config.toml`

Use config for:

- default verify profile
- custom verify profile commands
- default promotion mode / target branch / commit policy
- named handoff packing profiles

See `docs/CONFIG.md` for concrete TOML examples.

---

## 9. CI and automation patterns

Bundle preview for CI:

```bash
diffship preview ./bundle --list --json
```

Bundle comparison for CI:

```bash
diffship compare ./bundle_a ./bundle_b --json
```

When both bundles include the canonical manifest JSON, compare output also includes structured-context summary deltas such as file/category/segment/status count changes, plus manifest reading-order deltas when that guidance differs.

Repository validation before finishing local work:

```bash
just docs-check
just trace-check
just ci
```

---

## 10. Common files and directories

Important repository paths:

- `docs/SPEC_V1.md`
- `docs/BUNDLE_FORMAT.md`
- `docs/PATCH_BUNDLE_FORMAT.md`
- `docs/CONFIG.md`
- `docs/OPS_WORKFLOW.md`
- `.diffship/config.toml`
- `.diffship/PROJECT_KIT.md`
- `.diffship/AI_GUIDE.md`
- `.diffship/runs/<run-id>/`

---

## 11. Current scope

The current v1 core includes:

- end-to-end handoff bundle generation
- preview / compare
- plan export / replay
- named handoff packing profiles
- TUI handoff flow
- end-to-end ops loop with safety defaults

Still treated as future-extension territory:

- extra compare/TUI polish
- raw zip container byte equality as a separate compare contract
- dedicated profile import/export commands

For exact status tracking, see `docs/IMPLEMENTATION_STATUS.md` and `PLAN.md`.
