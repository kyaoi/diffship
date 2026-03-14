# DiffshipOS Project Kit (template)

This document is a template used by `diffship init`.

Its audience is the human developer operating diffship in a specific repository.
The goal is to keep the default workflow stable while making it easy to add repository-specific operating rules.

Use this template in two layers:

- Keep the **core workflow** sections aligned with actual diffship behavior.
- Replace the **customize this section** blocks with repository-specific guidance.

---

## 0. Attachment-ready summary for external AI tools

If you need short project rules text for an external AI workspace or project UI, attach this file together with `AI_GUIDE.md` and keep these points intact:

- read the listed spec and workflow files before changing code
- keep edits minimal, reviewable, and deterministic
- use `diffship loop` only with a valid `OPS_PATCH_BUNDLE`
- ask for the exact repo HEAD SHA before producing a loop-ready patch bundle
- respect local safety rules such as `[ops.forbid]` patterns in addition to the built-in forbidden paths

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
- accept arbitrary zip structures in `diffship loop`

---

## 2. Core workflow: default operating loop

Typical loop:

1. Build a handoff bundle from local changes.
2. Send the bundle to an AI assistant.
3. Receive one of the supported return modes described below.
4. Apply only an ops-compatible patch bundle with diffship.
5. Run local verification.
6. Promote only after verification passes.

Before running `diffship loop`, do this preflight:

- keep the repository working tree clean (`git status --short` should not show unrelated files)
- store incoming patch bundle zips **outside the repo** or under `.diffship/` so the zip itself does not make the working tree dirty
- if you want an ops-compatible patch bundle, provide the current target repo HEAD SHA (`git rev-parse HEAD`) to the AI up front
- if you want a loop-ready artifact, tell the AI explicitly that missing `base_commit` should result in a request for the SHA or `MODE: ANALYSIS_ONLY`, **not** an auto-generated fallback zip
- remember that `diffship loop` accepts only an **ops-compatible patch bundle**; it does **not** accept review notes, unified-diff-only zips, overlay bundles, or arbitrary non-ops archives

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

## 3. Core workflow: supported AI return modes

The AI must choose **exactly one** return mode.

| Mode | Allowed input to `diffship loop`? | What you should see |
|---|---|---|
| `OPS_PATCH_BUNDLE` | Yes | `manifest.yaml` and `changes/*.patch` under one bundle root |
| `NONOPS_EDIT_PACKAGE` | No | `README_NOT_OPS.md` plus `overlay/`, `edits/`, or `meta/` |
| `ANALYSIS_ONLY` | No | plain analysis / plan / explanation, usually no zip |

Human checklist before calling `diffship loop`:

1. confirm the archive is an `OPS_PATCH_BUNDLE`
2. confirm the extracted bundle root contains `manifest.yaml`
3. confirm the extracted bundle root contains `changes/`
4. stop immediately if you see `README_NOT_OPS.md`
5. stop immediately if the archive name or root path contains `DO_NOT_LOOP`

### Tree cheat sheet

Use the archive tree itself as the first classifier.

Valid loop-ready bundle:

```text
patchship_YYYY-MM-DD_HHMM/
  manifest.yaml
  changes/
    0001.patch
  commit_message.txt        # optional
  summary.md                # optional
  tasks/                    # optional
```

Intentionally non-loop-ready bundle:

```text
DO_NOT_LOOP_nonops_YYYY-MM-DD_HHMM/
  README_NOT_OPS.md
  overlay/
  edits/
  meta/
```

Analysis-only response:

```text
MODE: ANALYSIS_ONLY
(no zip required)
```

Rules of thumb:

- A loop-ready zip always has `manifest.yaml` and `changes/` at the same bundle root.
- A non-ops zip always has `README_NOT_OPS.md` at the bundle root and should look obviously different from a patch bundle.
- Do not accept loose files at zip root; expect exactly one top-level directory.

---

## 4. Customize this section: repository identity

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

## 5. Customize this section: read-first files

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

## 6. Core workflow: patch bundle contract the AI must follow

Patch bundles should contain exactly one top-level directory. The zip root must not contain loose files beside that directory.

Expected tree:

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
- if `apply_mode=git-am`, default the patch mail header author to `Diffship <diffship@example.com>` unless this repository defines a different explicit tool identity
- do not use provider-specific `From:` identities such as `OpenAI <assistant@example.com>` as the default contract
- if the exact `base_commit` is unavailable and you asked for loop-ready output, the AI should request the SHA or return `MODE: ANALYSIS_ONLY` instead of fabricating a fallback zip
- paths must be repo-relative only
- do not touch `.git/` or `.diffship/`
- respect any repository-local `[ops.forbid]` patterns (for example lockfiles such as `pnpm-lock.yaml`) in addition to the built-in forbidden paths
- do not include secrets
- keep file ordering and output deterministic
- patches must not include binary patches, rename/copy metadata, file mode metadata (`old mode`, `new mode`, `new file mode`), or submodule changes
- do not use `tasks/USER_TASKS.md` to ask the user to repair an otherwise invalid patch bundle; tasks are for real user-owned follow-up work only
- reserve the `patchship_...` name for bundles that are already valid ops inputs

Required marker placement:

- `manifest.yaml` must sit directly under `patchship_.../`
- `changes/` must sit directly under `patchship_.../`
- optional files may sit beside them, not inside `overlay/` or another nested parent

Recommended minimal `manifest.yaml`:

```yaml
protocol_version: "1"
task_id: "replace-with-task-id"
base_commit: "<exact 40-hex SHA>"
apply_mode: git-apply
touched_files:
  - path/to/file.ext
```

If manual user work is required, the AI should use `tasks/USER_TASKS.md` and `tasks/ENV_TEMPLATE.env`.

---

## 7. Core workflow: non-ops fallback contract

If the AI cannot produce a valid ops-compatible patch bundle, it should either:

- return `MODE: ANALYSIS_ONLY`, or
- return `MODE: NONOPS_EDIT_PACKAGE` **only when the user explicitly accepts a non-ops fallback**

Non-ops archives should use a clearly unsafe-for-loop shape such as:

```text
DO_NOT_LOOP_nonops_YYYY-MM-DD_HHMM/
  README_NOT_OPS.md
  overlay/
  edits/
  meta/
```

Rules for non-ops archives:

- include `README_NOT_OPS.md` at the bundle root
- start the first sentence with: `This artifact is intentionally not ops-compatible and must not be fed to diffship loop.`
- never include a fake `manifest.yaml`
- never use the `patchship_...` prefix
- prefer the `DO_NOT_LOOP_` prefix so humans do not mistake it for a patch bundle

---

## 8. Customize this section: directory map and ownership boundaries

Explain which directories are safe to edit, generated, sensitive, or user-owned.

Suggested shape:

| Path | Meaning | Normal changes | Avoid / escalate |
|---|---|---|---|
| `src/` | main implementation | feature and bugfix edits | unrelated refactors |
| `docs/` | specs and workflow docs | keep in sync with behavior | undocumented contract changes |
| `tests/` | regression coverage | add/update with behavior changes | removing coverage without cause |

Add rows for migrations, generated code, vendor directories, infra config, or anything with stronger safety requirements.

---

## 9. Customize this section: local commands and gates

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

## 10. Core workflow: commit and promotion behavior

Patch transport and commit behavior are separate:

- the AI may include `commit_message.txt`
- diffship promotion behavior is decided by local config
- promotion may create a commit, leave changes in the working tree, or do nothing depending on config
- with `apply_mode=git-am`, the patch `From:` line becomes the commit author; default AI-generated mail patches to `Diffship <diffship@example.com>` unless this repository says otherwise
- if you want the final commit author to be the local human operator by default, prefer `git-apply` or a documented post-promotion `git commit --amend --reset-author --no-edit` step

If `commit_message.txt` is missing, diffship may use a deterministic fallback.

---

## 11. Customize this section: project-specific operating rules

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
- Update `docs/TRACEABILITY.md` when behavior changes.
- Do not hand-edit generated API clients under `clients/generated/`.
- Stop and ask before writing production migrations.
```

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

---

## 12. Customize this section: ready-to-run workflows

Add a few project-specific command recipes that contributors can copy directly.

Suggested examples:

- build only the relevant handoff bundle
- ask the AI for a loop-ready bundle with the current HEAD SHA
- apply and verify a returned patch bundle
- inspect runs or failed tasks
- prepare a reprompt bundle

Example:

```bash
git rev-parse HEAD
diffship build --include 'src/*.rs' --include 'docs/*.md'
diffship preview ./.diffship/handoffs/diffship_2026-03-07_1118
# ask the AI for MODE: OPS_PATCH_BUNDLE; if base_commit is missing, require ANALYSIS_ONLY instead of a fallback zip
diffship loop ./incoming/patchship_fix.zip
diffship runs --json
diffship pack-fix --run-id <run-id>
```

---

## 13. Core workflow: what to include when sending work to AI

When asking an AI assistant for help, include only the files that matter:

- the handoff bundle or patch bundle
- the relevant spec / contract docs
- the generated local guides under `.diffship/` when they add repository context
- any specific failure logs or run IDs the AI needs
- the exact `git rev-parse HEAD` when you need loop-ready output

Avoid sending:

- unrelated generated artifacts
- secrets
- broad copies of the repository when a focused bundle is enough
- ambiguous requests such as “make a zip I can use later” without saying whether you want `OPS_PATCH_BUNDLE`, `NONOPS_EDIT_PACKAGE`, or `ANALYSIS_ONLY`
