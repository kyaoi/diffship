# diffship

**diffship** is an **AI-assisted development OS** for Git repositories.

It focuses on the *ops* side of an AI workflow:

- safely **apply** an AI-produced patch bundle in an isolated sandbox
- **verify** it with local quality gates
- **promote** the result back to your target branch (or skip promotion)
- record runs under the run directory (e.g. .diffship/runs/<run-id>/...) and generate a **reprompt bundle** when needed

> Note: The *handoff* (diff → AI bundle) workflow is **partially implemented**.
> `diffship build` supports committed / staged / unstaged / untracked sources, `--split-by auto|file|commit`, fallback repacking/exclusion for packing limits, optional attachments.zip / excluded.md / secrets.md, .diffshipignore, secrets warnings (`--yes` / `--fail-on-secrets`), and a generated HANDOFF entry document with Start Here / TL;DR / Change Map / Parts Index.
> Binary content is excluded by default and can be opted-in via `--include-binary --binary-mode raw|patch|meta`.
> `diffship preview` / `diffship compare` are implemented for quick review and reproducibility checks.
> The TUI now includes a handoff screen for range/sources/split selection, internal diff preview, build launch, and equivalent CLI command display.
> Remaining handoff gaps are mainly include/exclude filter flags, JSON output for preview/compare, context-reduction fallback, and plan export/replay.
> Handoff output ordering and generated zip metadata are normalized so golden tests can compare stable bundle trees / zip bytes.
> The ops-focused TUI v0 is available: run `diffship` (in a TTY) or `diffship tui`.
> See `docs/SPEC_V1.md` and `docs/TRACEABILITY.md` for the contract and status.

---

## Install

This repository is currently intended for local use.

```bash
# in this repo
cargo install --path .

# or, without installing
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
.diffship/PROJECT_KIT.md
.diffship/config.toml
```

### 2) Apply → verify → promote (the main loop)

```bash
diffship loop path/to/patch-bundle.zip
```

If promotion is blocked:

- secrets were detected → rerun with `--ack-secrets`
- required user tasks exist → complete them, then rerun with `--ack-tasks`

If verification fails, diffship writes a default reprompt zip under `.diffship/runs/` in the run directory.
You can also run `diffship pack-fix --run-id <run-id>` manually.

---

## Commands

All commands below are implemented.

- `diffship` — start the interactive TUI when running in a TTY (same as `diffship tui`)
- `diffship tui` — start the interactive TUI (status/runs viewer + loop launcher + handoff screen)

- `diffship init` — generate `.diffship/` project kit files
- `diffship status` — show lock state and recent runs (`--json` available)
- `diffship runs` — list recent runs (`--json` available)
- `diffship apply <bundle>` — apply a patch bundle in an isolated sandbox (`--session`, `--keep-sandbox`)
- `diffship verify` — run verification in the latest sandbox (`--profile`, `--run-id`)
- `diffship pack-fix` — create a reprompt zip for a run (`--run-id`, `--out`)
- `diffship promote` — promote a verified run into a target branch
- `diffship build` — generate a handoff bundle (HANDOFF.md, parts/, optional attachments.zip, excluded.md, secrets.md)
- `diffship preview <bundle>` — show HANDOFF.md / parts from a bundle (`--list`, `--part`)
- `diffship compare <bundle-a> <bundle-b>` — compare bundles (`--strict` for raw byte comparison)
- `diffship loop <bundle>` — apply → verify → promote

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

# tighten packing limits for CI/runtime checks
diffship build --max-parts 10 --max-bytes-per-part 104857600

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
diffship preview ./diffship_2026-03-06_1200 --list

# compare two bundles for reproducibility checks
diffship compare ./bundle_a ./bundle_b.zip
```

Output layout:
- HANDOFF.md (entry document: Start Here / TL;DR / Change Map / Parts Index)
- parts/part_XX.patch
- attachments.zip (when raw attachments exist)
- excluded.md (when files are intentionally omitted)
- secrets.md (when secrets-like content is detected; paths + reasons only)
- .diffshipignore is respected when present

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
- **Patch bundle contract:** `docs/PATCH_BUNDLE_FORMAT.md`
- **Ops workflow guide:** `docs/OPS_WORKFLOW.md`
- **Config:** `docs/CONFIG.md`
- **Traceability:** `docs/TRACEABILITY.md`
- **Definition of Done:** `docs/DEFINITION_OF_DONE.md`
- **Working with AI:** `docs/AI_WORKFLOW.md`
