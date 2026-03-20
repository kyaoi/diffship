# diffship

**diffship** is an **AI-assisted development OS** for Git repositories.

It focuses on the *ops* side of an AI workflow:

- safely **apply** an AI-produced patch bundle in an isolated sandbox
- **verify** it with local quality gates
- **promote** the result back to your target branch (or skip promotion)
- record runs under the run directory (e.g. .diffship/runs/<run-id>/...) and generate a **reprompt bundle** when needed

> Note: The *handoff* (diff → AI bundle) workflow is **implemented for the current v1 core**.
> `diffship build` supports committed / staged / unstaged / untracked sources, `--split-by auto|file|commit`, fallback repacking/exclusion for packing limits, deterministic handoff manifest plus per-part context JSON and rendered XML view files, optional attachments.zip / excluded.md / secrets.md, .diffshipignore, secrets warnings (`--yes` / `--fail-on-secrets`), and a generated HANDOFF entry document with Start Here / TL;DR / Change Map / Parts Index.
> Binary content is excluded by default and can be opted-in via `--include-binary --binary-mode raw|patch|meta`.
> `diffship preview` / `diffship compare` are implemented for quick review and reproducibility checks; `preview --list` now surfaces canonical structured-context summary counts plus reading-order guidance when the bundle includes the manifest JSON, and `compare` now also surfaces manifest summary plus reading-order deltas alongside area/kind diff classification.
> The canonical manifest JSON now also carries deterministic reading-order guidance plus per-file semantic facts such as language hints, generated / lockfile / tooling flags, and related test candidates so downstream tooling can reuse navigation and prioritization hints without scraping markdown.
> That same canonical manifest JSON now also includes deterministic cross-part `task_groups` clustered by shared per-part intent labels, which gives hosted AI a stable first-pass notion of likely multi-part tasks.
> Those task groups now also include primary labels, related context/project files, suggested bounded read order, and risk hints, so hosted AI can reuse a deterministic task-level execution plan instead of improvising one.
> Those same task groups and per-part context JSON now also include deterministic review labels, so hosted AI can distinguish behavioral changes from mechanical or verification-heavy updates before reparsing full patches.
> Manifest task groups now also include deterministic `verification_targets`, so hosted AI can keep likely tests/config/policy surfaces bounded instead of inferring verification scope from raw paths alone.
> Those same task groups now also include deterministic `verification_labels`, so hosted AI can tell whether a task needs test follow-up, config/policy review, dependency validation, behavioral-regression watch, or only a lightweight sanity check.
> Those task groups now also include deterministic `widening_labels`, so hosted AI can tell whether to stay patch-only or widen into related tests/config/docs/repo rules before it starts reading extra context.
> Those task groups now also include deterministic `execution_labels`, so hosted AI can keep a coarse execution flow in view instead of improvising when to widen, review repo rules, or plan post-edit verification.
> Those task groups now also include deterministic `task_shape_labels`, so hosted AI can tell whether a task is single-area or cross-cutting and whether it likely deserves heavier review or verification attention.
> Those task groups now also include deterministic `edit_targets` and `context_only_files`, so hosted AI can separate bounded write scope from read-only supporting context.
> Per-part context JSON now also includes deterministic task-group linkage such as `task_group_ref`, `task_shape_labels`, `task_edit_targets`, and `task_context_only_files`, so a model that starts from one part context can still recover the bounded task contract.
> Canonical file semantic facts now also expose deterministic path-role hints such as `repo_rule_touch`, `dependency_policy_touch`, `build_graph_touch`, and `test_infrastructure_touch`, so hosted AI can distinguish rule/build/test-support surfaces without parser-heavy analysis.
> `diffship preview --list` now also surfaces those manifest task groups directly, so humans can inspect task clustering without opening raw manifest JSON.
> Those canonical file semantics now also carry deterministic coarse labels such as docs/config/test touches plus import/signature/API-like hints, which gives hosted AI consumers a cheaper first-pass intent signal before deeper analysis.
> Per-part context JSON now also includes deterministic scoped hints such as hunk-header text, symbol-like names, import-like references, and related test candidates derived from the patch part plus local repository facts, plus additive `intent_labels` that classify likely part roles such as source/docs/test updates, cross-area changes, and API/import-heavy touches.
> `diffship preview --list` now also reports lightweight coverage for those richer JSON facts so users can confirm semantic/coarse-label/change-hint/scoped hints exist without opening the raw JSON files first.
> Per-part scoped context now also keeps symbol/import hints attributable to concrete changed files, which makes multi-file parts easier for downstream AI tooling to interpret.
> Canonical file semantics now also make source/test navigation bidirectional by giving changed test-like files likely source candidates.
> Canonical file semantics now also point to likely docs and config/build files, giving AI consumers a better first-pass reading order across source, tests, docs, and setup.
> Canonical file entries now also expose deterministic `change_hints` for rename ancestry, attachment/exclusion routing, and reduced-context fallback, so downstream tooling does not need to scrape prose notes.
> `diffship build --project-context focused` now also emits a deterministic focused project-context pack with a canonical JSON index, a rendered markdown view, and bounded text snapshots under the `project_context` directory so hosted AI tools can inspect a bounded slice of repo context without diffship falling back to a whole-repo snapshot.
> That focused project-context JSON now also carries per-file semantic facts, `changed` markers, `context_labels`, inbound/outbound relationship refs, and summary counts by category/relationship kind, which makes it easier for hosted AI tools to understand why supplemental files are present instead of inferring everything from path names alone.
> Focused project-context files now also carry `usage_role`, `priority`, `edit_scope_role`, `verification_relevance`, `verification_labels`, `why_included`, and `task_group_refs`, so hosted AI can tell which supplemental files are direct write targets versus read-only support, rule, or verification context.
> Every new handoff bundle now also includes a deterministic bundle-local hosted-AI request scaffold covering reading order, output modes, and loop-safety constraints.
> When focused project context is present, that same AI request scaffold now also gives hosted AI tools deterministic changed-context and direct-relationship hints so they can widen scope in a bounded, repeatable way.
> That scaffold now also reuses canonical task-group and focused-context usage facts to emit a deterministic task-group execution recipe, so hosted AI can follow diffship’s bounded read order instead of improvising one.
> It now also carries canonical review labels for both task groups and patch parts, so hosted AI keeps behavioral vs mechanical vs verification-heavy strategy hints in scope while planning edits.
> It now also reuses canonical verification-focused facts from manifest task groups and focused project context, so hosted AI sees bounded verification-reading guidance before proposing local checks.
> It now also reuses task-group `verification_labels`, so that verification guidance carries both a bounded file list and a coarse strategy.
> It now also reuses task-group `widening_labels`, so context widening itself stays deterministic instead of being improvised from raw project-file lists.
> It now also reuses task-group `execution_labels`, so the bundle-local scaffold carries a deterministic coarse execution flow instead of leaving hosted AI to improvise it.
> It now also reuses task-group `task_shape_labels`, so the bundle-local scaffold carries a deterministic coarse task-shape signal instead of leaving hosted AI to improvise it.
> It now also reuses task-group `edit_targets` and `context_only_files`, so the bundle-local scaffold carries a deterministic bounded write scope instead of leaving hosted AI to improvise it.
> That same focused project-context pack now also carries per-file `edit_scope_role` plus summary `edit_scope_counts`, and the generated AI request scaffold reuses those roles so widened project context remains read-only unless diffship explicitly marked a file as a write target.
> That same AI request scaffold now also summarizes per-part intent labels, segments, and top files so hosted AI can pick the most relevant per-part context JSON files before reparsing every patch part in full.
> `pack-fix` reprompt zips now also include post-apply summaries and logs when local post-apply hooks ran, so hosted AI tools can see local normalization evidence before interpreting verify failures.
> The TUI now includes a handoff screen for range/sources/filters/split selection, internal diff preview, build launch, and equivalent CLI command display.
> The TUI handoff preview now prepends canonical structured-context summary counts plus manifest reading-order guidance when the temporary preview bundle includes the manifest JSON.
> The TUI now also includes a compare screen that wraps `diffship compare --json` and surfaces manifest summary / reading-order deltas interactively.
> The TUI handoff screen now shows a live edit buffer/help area and can edit plan path / packing limit overrides with `Tab` / `Shift+Tab` navigation.
> `diffship build` now supports repeatable `--include <glob>` / `--exclude <glob>` filters in addition to `.diffshipignore`.
> Packing fallback now attempts context reduction (`U3 -> U1 -> U0`) before excluding an oversized diff unit.
> `diffship build --plan-out <path>` and `diffship build --plan <path>` are implemented, and the TUI can export a replayable handoff plan.
> The exported handoff plan records the selected profile name plus resolved numeric limits; named profile catalogs remain config-driven via the handoff/profile config sections.
> Remaining handoff work is mainly future-extension territory (for example compare/TUI UX polish), not the current v1 handoff core. These are tracked as v1.1+ polish items rather than blockers for the current contract.
> Handoff output ordering and generated zip metadata are normalized so golden tests can compare stable bundle trees / zip bytes.
> The ops-focused TUI v0 is available: run `diffship` (in a TTY) or `diffship tui`.
> See `docs/SPEC_V1.md` and `docs/TRACEABILITY.md` for the contract and status.

---

## Install

If you only want to install **diffship** as a CLI, you can install it directly from GitHub without manually cloning the repository:

```bash
cargo install --git https://github.com/kyaoi/diffship.git
```

For reproducible installs from Git, pin to a specific tag, branch, or commit.
Use `--tag` for released versions; `--version` does not select Git tags.

```bash
cargo install --git https://github.com/kyaoi/diffship.git --tag v0.6.0
# or
cargo install --git https://github.com/kyaoi/diffship.git --branch main
# or
cargo install --git https://github.com/kyaoi/diffship.git --rev <commit>
```

## Build from source

If you want to inspect the source, make changes, or work from a local checkout:

```bash
git clone https://github.com/kyaoi/diffship.git
cd diffship
cargo build
```

## Run from source

If you want to run it from a local checkout without installing:

```bash
git clone https://github.com/kyaoi/diffship.git
cd diffship
cargo run -- <subcommand> ...
```

---

## Quickstart (Ops)

### 1) Initialize project kit
Run this once per repo you want to operate on:

```bash
diffship init
```

It creates files under `.diffship/` (generated):

```text
.diffship/.gitignore
.diffship/PROJECT_KIT.md
.diffship/AI_GUIDE.md
.diffship/config.toml
```

### 2) Apply → verify → promote (the main loop)

```bash
diffship loop path/to/patch-bundle.zip
```

If promotion is blocked:

- secrets were detected → rerun with `--ack-secrets`
- required user tasks exist → complete them, then rerun with `--ack-tasks`

If verification fails, diffship writes a default reprompt zip under the run directory inside `.diffship/`.
You can also run `diffship pack-fix --run-id <run-id>` manually.
If ops.post_apply commands are configured, diffship runs them in the sandbox right after apply succeeds.
The post-apply summary JSON now also records deterministic changed paths, coarse categories, and a machine-readable normalization summary, and reprompt bundles surface that evidence before verify logs.

---

## Commands

All commands below are implemented.

- `diffship` — start the interactive TUI when running in a TTY (same as `diffship tui`)
- `diffship tui` — start the interactive TUI (status/runs viewer + loop launcher + handoff screen)

- `diffship init` — generate `.diffship/` project kit files, including an AI-facing guide
  - optional: `--template-dir <dir>` to override `docs/PROJECT_KIT_TEMPLATE.md` and `docs/AI_PROJECT_TEMPLATE.md`
  - optional: `--refresh-forbid` to rewrite only the dedicated forbid file from current repo detections
- `diffship status` — show lock state and recent runs, including direct run/log artifact paths when present (`--json` available)
- `diffship runs` — list recent runs, including direct run/log artifact paths when present (`--json` available)
- `diffship cleanup` — remove unused diffship-owned workspaces, eligible runs, and build artifacts (`--dry-run`, `--include-runs`, `--include-builds`, `--all`, `--json`)
- `diffship apply <bundle>` — apply a patch bundle in an isolated sandbox (`--session`, `--keep-sandbox`)
- `diffship verify` — run verification in the latest sandbox (`--profile`, `--run-id`)
- `diffship pack-fix` — create a reprompt zip for a run (`--run-id`, `--out`)
- `diffship promote` — promote a verified run into a target branch
- `diffship build` — generate a handoff bundle (`--profile`, HANDOFF.md, handoff manifest JSON, rendered XML view, per-part context JSON, parts/, optional attachments.zip, excluded.md, secrets.md, optional plan.toml via `--plan-out`)
- `diffship preview <bundle>` — show HANDOFF.md / parts from a bundle (`--list`, `--part`, `--json`); `--list` also surfaces structured-context summary counts when available
- `diffship compare <bundle-a> <bundle-b>` — compare bundles (`--strict` = extracted entry bytes without normalization, `--json`), classify differences by area/kind, and surface manifest-summary deltas when available
- `diffship loop <bundle>` — apply → verify → promote

Filesystem path arguments accept leading tilde-slash and resolve it against the current user's `HOME`. Tilde-user shorthand is rejected.

### Promotion / commit switches

Both `promote` and `loop` accept overrides:

- `--promotion <none|working-tree|commit>`
- `--commit-policy <auto|manual>`

Current note:
- `none` is implemented and tested.
- `working-tree` is implemented as no-commit promotion (applies patch result onto target working tree).

For details and examples, see `docs/OPS_WORKFLOW.md`.

---


### Handoff build

```bash
# last committed change
diffship build

# staged + unstaged + untracked (no committed range)
diffship build --no-committed --include-staged --include-unstaged --include-untracked

# commit-oriented split for a multi-commit committed range
diffship build --range-mode direct --from HEAD~3 --to HEAD --split-by commit

# keep only selected paths across all segments
diffship build --include 'src/*.rs' --include '*.txt' --exclude 'src/generated.rs'

# tighten packing limits for CI/runtime checks
diffship build --max-parts 10 --max-bytes-per-part 104857600

# select a named packing profile
diffship build --profile 10x100

# keep untracked files as metadata only
diffship build --no-committed --include-untracked --untracked-mode meta

# binary content is excluded by default
# include binary files as raw attachments
diffship build --include-binary --binary-mode raw

# continue after a secrets warning (non-interactive)
diffship build --yes

# fail in CI when secrets-like content is detected
diffship build --fail-on-secrets

# inspect a generated bundle
diffship preview ./diffship_2026-03-06_1200_abcdef1 --list

# compare two bundles for reproducibility checks
diffship compare ./bundle_a ./bundle_b.zip

# CI-friendly machine-readable checks
diffship preview ./diffship_2026-03-06_1200_abcdef1 --list --json
diffship compare ./bundle_a ./bundle_b.zip --json

# export and replay a build plan
diffship build --include-untracked --plan-out ./diffship_plan.toml
diffship build --plan ./diffship_plan.toml --out ./replayed_bundle

# place the auto-generated bundle under a custom parent directory
diffship build --out-dir ./.diffship/artifacts/handoffs

# create only a zip bundle
diffship build --zip-only

# or set the same default in config
# [handoff]
# output_dir = "./.diffship/artifacts/handoffs"
# output_dir = "~/ghq/github.com/kyaoi/diffship/.diffship/handoffs"

# share named profiles via config, not via plan.toml
# project: .diffship/config.toml or repo-root .diffship.toml
# global:  ~/.config/diffship/config.toml
```

Output layout:
- HANDOFF.md (entry document: Start Here / TL;DR / Change Map / Parts Index)
- AI_REQUESTS.md (bundle-local hosted-AI request scaffold)
- handoff.manifest.json (deterministic machine-readable bundle summary)
- handoff.context.xml (deterministic rendered XML view)
- project.context.json (optional canonical focused project-context index)
- PROJECT_CONTEXT.md (optional rendered focused project-context view)
- project_context/files/... (optional focused text snapshots)
- parts/part_XX.patch
- parts/part_XX.context.json (deterministic per-part machine-readable summary)
- attachments.zip (when raw attachments exist)
- excluded.md (when files are intentionally omitted)
- secrets.md (when secrets-like content is detected; paths + reasons only)
- .diffshipignore is respected when present

Default output naming:
- `--out <path>` sets the exact output directory path
- `--out-dir <dir>` changes the parent directory while preserving the auto-generated `diffship_<timestamp>_<head7>` bundle name
- `--zip` keeps the directory output and also writes a sibling zip bundle with the same generated base name
- `--zip-only` writes only a zip bundle; with `--out`, the path must use a zip filename
- `--project-context focused` adds a bounded supplemental project-context pack for hosted AI workflows; `none` remains the default
- the handoff config can set the default parent directory via `output_dir`
- leading tilde-slash paths such as ~/handoffs are expanded against the current user's `HOME` for `--out`, `--out-dir`, `--plan`, `--plan-out`, and `output_dir`
- when `--out` is omitted, diffship uses a `diffship_YYYY-MM-DD_HHMM_<head7>` directory name in the user's local timezone
- if that directory already exists, diffship appends `_2`, `_3`, ... instead of failing

## Configuration

diffship merges config sources with precedence:

**CLI > manifest.yaml > project > global > default**

See `docs/CONFIG.md` for supported keys and examples.

---

## Development

This repo uses **mise** + **just** + **lefthook**.

```bash
mise install
lefthook install
just ci
```

---

## Documentation

- **Spec (v1, source of truth):** `docs/SPEC_V1.md`
- **Detailed usage guide:** `docs/USAGE_GUIDE.md`
- **AI handoff flow:** `docs/AI_HANDOFF_FLOW.md`
- **Patch bundle contract:** `docs/PATCH_BUNDLE_FORMAT.md`
- **Ops workflow guide:** `docs/OPS_WORKFLOW.md`
- **Config:** `docs/CONFIG.md`
- **Traceability:** `docs/TRACEABILITY.md`
- **Definition of Done:** `docs/DEFINITION_OF_DONE.md`
- **Working with AI:** `docs/AI_WORKFLOW.md`
