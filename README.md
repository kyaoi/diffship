# diffship

**diffship** is an **AI-assisted development OS** for Git repositories.

It focuses on the *ops* side of an AI workflow:

- safely **apply** an AI-produced patch bundle in an isolated sandbox
- **verify** it with local quality gates
- **promote** the result back to your target branch (or skip promotion)
- record runs under the run directory (e.g. .diffship/runs/<run-id>/...) and generate a **reprompt bundle** when needed

> Note: The *handoff* (diff → AI bundle) workflow is **implemented for the current v1 core**.
> `diffship build` supports committed / staged / unstaged / untracked sources, `--split-by auto|file|commit`, fallback repacking/exclusion for packing limits, optional attachments.zip / excluded.md / secrets.md, .diffshipignore, secrets warnings (`--yes` / `--fail-on-secrets`), and a generated HANDOFF entry document with Start Here / TL;DR / Change Map / Parts Index.
> Binary content is excluded by default and can be opted-in via `--include-binary --binary-mode raw|patch|meta`.
> `diffship preview` / `diffship compare` are implemented for quick review and reproducibility checks, and `compare` now classifies diffs by area/kind.
> The TUI now includes a handoff screen for range/sources/filters/split selection, internal diff preview, build launch, and equivalent CLI command display.
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
cargo install --git https://github.com/kyaoi/diffship.git --tag v0.4.1
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

---

## Commands

All commands below are implemented.

- `diffship` — start the interactive TUI when running in a TTY (same as `diffship tui`)
- `diffship tui` — start the interactive TUI (status/runs viewer + loop launcher + handoff screen)

- `diffship init` — generate `.diffship/` project kit files, including an AI-facing guide
  - optional: `--template-dir <dir>` to override `docs/PROJECT_KIT_TEMPLATE.md` and `docs/AI_PROJECT_TEMPLATE.md`
- `diffship status` — show lock state and recent runs (`--json` available)
- `diffship runs` — list recent runs (`--json` available)
- `diffship apply <bundle>` — apply a patch bundle in an isolated sandbox (`--session`, `--keep-sandbox`)
- `diffship verify` — run verification in the latest sandbox (`--profile`, `--run-id`)
- `diffship pack-fix` — create a reprompt zip for a run (`--run-id`, `--out`)
- `diffship promote` — promote a verified run into a target branch
- `diffship build` — generate a handoff bundle (`--profile`, HANDOFF.md, parts/, optional attachments.zip, excluded.md, secrets.md, optional plan.toml via `--plan-out`)
- `diffship preview <bundle>` — show HANDOFF.md / parts from a bundle (`--list`, `--part`, `--json`)
- `diffship compare <bundle-a> <bundle-b>` — compare bundles (`--strict` = extracted entry bytes without normalization, `--json`) and classify differences by area/kind
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
- parts/part_XX.patch
- attachments.zip (when raw attachments exist)
- excluded.md (when files are intentionally omitted)
- secrets.md (when secrets-like content is detected; paths + reasons only)
- .diffshipignore is respected when present

Default output naming:
- `--out <path>` sets the exact output directory path
- `--out-dir <dir>` changes the parent directory while preserving the auto-generated `diffship_<timestamp>_<head7>` bundle name
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
