# diffship

**diffship** is an AI-assisted development OS for Git repos.

It covers both sides of an AI workflow:

- **Handoff**: package Git changes into a deterministic AI-ready bundle
- **Ops**: apply an AI-produced patch bundle in an isolated sandbox, verify it, and promote the result safely

## What diffship gives you

- Deterministic handoff bundles built from committed, staged, unstaged, and optional untracked changes
- Preview and compare commands for bundle review and reproducibility checks
- Safe patch-bundle apply, verify, promote, and reprompt loops under `.diffship/runs/`
- Repo-local project kit generation via `diffship init`
- Configurable verify profiles, promotion modes, and handoff packing profiles
- CLI and TUI support for the core workflows

## Install

Install from GitHub:

```bash
cargo install --git https://github.com/kyaoi/diffship.git --tag v0.6.4
```

Or work from source:

```bash
git clone https://github.com/kyaoi/diffship.git
cd diffship
cargo build
```

## Quick Start

Initialize the local project kit once per repo:

```bash
diffship init
```

Build a handoff bundle from your latest committed change:

```bash
diffship build
diffship preview ./diffship_YYYY-MM-DD_HHMM_<head7> --list
```

Run the main ops loop for an AI patch bundle:

```bash
diffship loop path/to/patch-bundle.zip
```

If verify or post-apply steps fail, diffship records the run under `.diffship/runs/`
and can generate a reprompt bundle with `diffship pack-fix --run-id <run-id>`.

## Common Commands

Handoff:

```bash
diffship build
diffship build --include-staged --include-unstaged --include-untracked --no-committed
diffship build --split-by commit --range-mode direct --from HEAD~3 --to HEAD
diffship preview ./bundle --list
diffship compare ./bundle_a ./bundle_b --json
```

Ops:

```bash
diffship init
diffship apply path/to/patch-bundle.zip
diffship verify
diffship promote
diffship loop path/to/patch-bundle.zip
diffship pack-fix --run-id <run-id>
```

Maintenance:

```bash
diffship status --json
diffship runs --json
diffship cleanup --dry-run --all
diffship doctor
diffship session --help
```

## Configuration

diffship resolves config with this precedence:

```text
CLI > manifest.yaml > project > global > default
```

See `docs/CONFIG.md` for supported keys, handoff profiles, verify profiles, and promotion settings.

## Development

This repo uses `mise`, `just`, and `lefthook`.

```bash
mise install
lefthook install
just ci
```

## Documentation

- Spec: `docs/SPEC_V1.md`
- Ops workflow: `docs/OPS_WORKFLOW.md`
- Patch bundle contract: `docs/PATCH_BUNDLE_FORMAT.md`
- Config reference: `docs/CONFIG.md`
- AI workflow: `docs/AI_WORKFLOW.md`
- AI handoff flow: `docs/AI_HANDOFF_FLOW.md`
- Traceability: `docs/TRACEABILITY.md`
