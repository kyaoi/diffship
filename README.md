# diffship

**diffship** is an **AI-assisted development OS** for Git repositories.

It focuses on the *ops* side of an AI workflow:

- safely **apply** an AI-produced patch bundle in an isolated sandbox
- **verify** it with local quality gates
- **promote** the result back to your target branch (or skip / no-commit)
- record runs under the run directory (e.g. .diffship/runs/<run-id>/...) and generate a **reprompt bundle** when needed

> Note: The *handoff* (diff → AI bundle) workflow is **specified** but not yet implemented.
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

If verification fails, use `diffship pack-fix` to build a reprompt zip for the run and send it back to the AI.

---

## Commands

All commands below are implemented.

- `diffship` — start the interactive TUI when running in a TTY (same as `diffship tui`)
- `diffship tui` — start the interactive TUI (status/runs viewer + loop launcher)

- `diffship init` — generate `.diffship/` project kit files
- `diffship status` — show lock state and recent runs (`--json` available)
- `diffship runs` — list recent runs (`--json` available)
- `diffship apply <bundle>` — apply a patch bundle in an isolated sandbox (`--session`, `--keep-sandbox`)
- `diffship verify` — run verification in the latest sandbox (`--profile`, `--run-id`)
- `diffship pack-fix` — create a reprompt zip for a run (`--run-id`, `--out`)
- `diffship promote` — promote a verified run into a target branch
- `diffship loop <bundle>` — apply → verify → promote

### Promotion / commit switches

Both `promote` and `loop` accept overrides:

- `--promotion <none|working-tree|commit>`
- `--commit-policy <auto|manual>`

For details and examples, see `docs/OPS_WORKFLOW.md`.

---

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
