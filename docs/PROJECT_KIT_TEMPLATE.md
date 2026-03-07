# DiffshipOS Project Kit (template)

This document is a template used by `diffship init`.

Its audience is the human developer operating diffship in a specific repository.
The goal is to keep the default workflow stable while making it easy to add repository-specific operating rules.

Use this template in two layers:

- Keep the **core workflow** sections aligned with actual diffship behavior.
- Replace the **customize this section** blocks with repository-specific guidance.

---

## 1. Core workflow: what diffship is

diffship is a local tool that:

- packages local changes into AI-readable handoff bundles
- applies AI-returned patch bundles safely in isolated sandboxes
- records deterministic run logs under `.diffship/runs/`
- can generate reprompt bundles for iteration via `pack-fix`

diffship does not:

- run arbitrary commands provided by the AI bundle
- push to remotes or open pull requests by itself

---

## 2. Core workflow: default operating loop

Typical loop:

1. Build a handoff bundle from local changes.
2. Send the bundle to an AI assistant.
3. Receive a patch bundle or concrete edit plan.
4. Apply it with diffship in a sandbox.
5. Run local verification.
6. Promote only after verification passes.

Before running `diffship loop`, do this preflight:

- keep the repository working tree clean (`git status --short` should not show unrelated files)
- store incoming patch bundle zips **outside the repo** or under `.diffship/` so the zip itself does not make the working tree dirty
- if you want an ops-compatible patch bundle, provide the current target repo HEAD SHA (`git rev-parse HEAD`) to the AI up front

Minimal commands:

```bash
git rev-parse HEAD
diffship build
diffship loop path/to/patch-bundle.zip
```

If a run fails and you want a reprompt bundle:

```bash
diffship pack-fix --run-id <run-id>
```

---

## 3. Customize this section: repository identity

Replace this section with project-specific facts.

Suggested shape:

- Repository name:
- Product / service summary:
- Main languages / frameworks:
- Primary apps or packages:
- Primary branch workflow:
- CI / deployment overview:

Example:

```md
- Repository name: example-service
- Product / service summary: internal order-processing API
- Main languages / frameworks: Rust backend, TypeScript admin UI
- Primary apps or packages: `crates/api`, `web/admin`
- Primary branch workflow: work on `develop`, merge to `main` after release checks
- CI / deployment overview: GitHub Actions for tests, internal deploy pipeline after merge
```

---

## 4. Customize this section: read-first files

List the files a contributor or AI assistant should read before touching code in this repository.

Suggested shape:

1. Core spec
2. Relevant architecture or decisions doc
3. Operational guide
4. Module-level README
5. Tests that define the intended behavior

Example:

```md
1. `docs/SPEC_V1.md`
2. `docs/DECISIONS.md`
3. `docs/OPS_WORKFLOW.md`
4. `src/README.md`
5. `tests/m2_apply_verify.rs`
```

---

## 5. Core workflow: patch bundle contract the AI must follow

Patch bundles should contain one top-level directory:

```text
patchship_YYYY-MM-DD_HHMM/
  manifest.yaml
  changes/
    0001.patch
  summary.md              # optional
  constraints.yaml        # optional
  checks_request.yaml     # optional
  commit_message.txt      # optional
  tasks/                  # optional
    USER_TASKS.md
    TASKS.yaml
    ENV_TEMPLATE.env
```

Key rules:

- `manifest.yaml` must include the exact current `base_commit` for the target repo; never leave placeholders such as `REPLACE_WITH_REPO_HEAD`
- `apply_mode` must be exactly `git-apply` or `git-am`
- if the exact `base_commit` is unavailable, ask for it or return a unified diff / file edits / review notes instead of an ops-compatible patch bundle
- paths must be repo-relative only
- do not touch `.git/` or `.diffship/`
- do not include secrets
- keep file ordering and output deterministic
- patches must not include binary patches, rename/copy metadata, file mode metadata (`old mode`, `new mode`, `new file mode`), or submodule changes
- do not use `tasks/USER_TASKS.md` to ask the user to repair an otherwise invalid patch bundle; tasks are for real user-owned follow-up work only

If manual user work is required, the AI should use `tasks/USER_TASKS.md` and `tasks/ENV_TEMPLATE.env`.

---

## 6. Customize this section: directory map and ownership boundaries

Explain which directories are safe to edit, generated, sensitive, or user-owned.

Suggested shape:

| Path | Meaning | Normal changes | Avoid / escalate |
|---|---|---|---|
| `src/` | main implementation | feature and bugfix edits | unrelated refactors |
| `docs/` | specs and workflow docs | keep in sync with behavior | undocumented contract changes |
| `tests/` | regression coverage | add/update with behavior changes | removing coverage without cause |

Add rows for migrations, generated code, vendor directories, infra config, or anything with stronger safety requirements.

---

## 7. Customize this section: local commands and gates

Record the real commands contributors should run here.

Suggested shape:

- Format:
- Lint:
- Focused tests:
- Full tests:
- Full gate:
- Optional helpers:

Example:

```md
- Format: `cargo fmt --all`
- Lint: `cargo clippy --all-targets --all-features -- -D warnings`
- Focused tests: `cargo test --test m0_integration`
- Full tests: `cargo test`
- Full gate: `just ci`
- Optional helpers: `just docs-check`, `just trace-check`
```

If some commands are slow, flaky, or require local services, note that explicitly.

---

## 8. Core workflow: commit and promotion behavior

Patch transport and commit behavior are separate:

- the AI may include `commit_message.txt`
- diffship promotion behavior is decided by local config
- promotion may create a commit, leave changes in the working tree, or do nothing depending on config

If `commit_message.txt` is missing, diffship may use a deterministic fallback.

---

## 9. Customize this section: project-specific operating rules

Use this section for rules that are specific to this repository but important for every task.

Suggested topics:

- branch naming or merge rules
- changelog or release note expectations
- migration approval rules
- generated file policy
- design system or API compatibility constraints
- when a contributor must stop and ask instead of proceeding

Example:

```md
- Keep commit messages in Conventional Commit style.
- Do not edit `api/generated/` by hand; regenerate it from the schema.
- If a database migration is required, stop and ask before writing migration files.
- Frontend changes must preserve the design tokens in `web/src/theme/`.
```

---

## 10. Core workflow: handling manual user tasks

If the AI cannot complete the change safely on its own, it should leave explicit user tasks.

Typical examples:

- creating or updating `.env`
- rotating credentials
- running one-off migrations
- performing actions in an external console

Expected shape for `tasks/USER_TASKS.md`:

```md
## Required user tasks

- [ ] Create `.env` from `tasks/ENV_TEMPLATE.env`
- [ ] Set `API_KEY` locally
- [ ] Re-run `diffship verify --profile standard`
```

Do not use manual tasks to patch over missing manifest fields or other bundle-contract violations.

---

## 11. Customize this section: ready-to-run workflows

Add a few project-specific command recipes that contributors can copy directly.

Suggested examples:

- build only the relevant handoff bundle
- apply and verify a returned patch bundle
- inspect runs or failed tasks
- prepare a reprompt bundle

Example:

```bash
git rev-parse HEAD
diffship build --include 'src/*.rs' --include 'docs/*.md'
diffship preview ./.diffship/handoffs/diffship_2026-03-07_1118
diffship loop ~/.cache/diffship/patchship_fix.zip
# or keep incoming bundles under .diffship/ if you want them inside the repo
diffship loop ./.diffship/incoming/patchship_fix.zip
diffship runs --json
diffship pack-fix --run-id <run-id>
```

---

## 12. Core workflow: what to include when sending work to AI

When asking an AI assistant for help, include only the files that matter:

- the handoff bundle or patch bundle
- the relevant spec / contract docs
- the generated local guides under `.diffship/` when they add repository context
- the current target repo HEAD SHA when you want an ops-compatible patch bundle
- any specific failure logs or run IDs the AI needs

Avoid sending:

- unrelated generated artifacts
- secrets
- broad copies of the repository when a focused bundle is enough
