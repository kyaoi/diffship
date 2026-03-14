# Diffship AI Guide (template)

This document is a template used by `diffship init`.

Its audience is an AI coding agent that receives files from a user working with diffship.
The goal is to make the workflow, output contract, and repository-specific expectations explicit.

Use this template in two layers:

- Keep the **core contract** sections aligned with diffship behavior.
- Replace the **customize this section** blocks with repository-specific guidance.

---

## 0. Attachment-ready project rules

Use this file as the default rules attachment when the user wants guidance for an external AI workspace or project UI.
The minimum rules that should survive copy-paste are:

- read the provided spec, workflow docs, and local diffship guides first
- keep changes minimal, deterministic, and traceable back to tests/docs
- emit exactly one response mode and reserve `OPS_PATCH_BUNDLE` for truly loop-ready output
- if `base_commit` is missing or uncertain, ask for `git rev-parse HEAD` or return `MODE: ANALYSIS_ONLY`
- respect local safety rules such as `[ops.forbid]` patterns in addition to the built-in forbidden paths

---

## 1. Core contract: what diffship is

diffship is a local development OS for Git repositories.

It provides two connected workflows:

- **handoff**: package local changes into a deterministic AI-readable bundle
- **ops**: apply an AI-produced patch bundle safely in an isolated sandbox, then verify and promote it

diffship expects the AI to work within explicit contracts rather than inventing its own repo workflow.

Non-negotiable expectations:

- read the provided project/spec files first
- keep changes minimal and deterministic
- produce outputs that diffship can verify or apply safely
- update docs, tests, and traceability together when behavior changes
- never assume diffship will run arbitrary commands that came from AI output
- never emit placeholder manifest values in an ops-compatible patch bundle
- if required metadata is unavailable, either ask for it or fall back according to the mode-selection rules below
- choose **exactly one** return mode and label it clearly

---

## 2. Core contract: response envelope

At the top of your response, state exactly one mode:

- `MODE: OPS_PATCH_BUNDLE`
- `MODE: NONOPS_EDIT_PACKAGE`
- `MODE: ANALYSIS_ONLY`

Do not mix modes.

Examples:

```text
MODE: OPS_PATCH_BUNDLE
```

```text
MODE: NONOPS_EDIT_PACKAGE
```

```text
MODE: ANALYSIS_ONLY
```

If you choose `NONOPS_EDIT_PACKAGE` or `ANALYSIS_ONLY`, also state plainly that the result must **not** be fed to `diffship loop` as-is.

---

## 3. Core contract: mode-selection rules

Choose modes in this order:

1. Use `MODE: OPS_PATCH_BUNDLE` only when all of the following are true:
   - the user wants a loop-ready or apply-ready artifact
   - the exact target `base_commit` is known
   - the change can be represented within diffship patch restrictions
   - you can produce the required bundle structure exactly
2. Use `MODE: ANALYSIS_ONLY` when the user wants a loop-ready artifact but the exact `base_commit` is missing or another hard precondition is unavailable.
3. Use `MODE: NONOPS_EDIT_PACKAGE` only when:
   - the user explicitly accepts a non-ops fallback, or
   - the user asked for file edits / overlay output rather than a loop-ready bundle.

Important behavior:

- If the user clearly wants something they can pass to `diffship loop`, **do not** silently downgrade to a non-ops zip just because `base_commit` is missing.
- In that situation, prefer `MODE: ANALYSIS_ONLY` and say exactly what is missing (usually `git rev-parse HEAD`).
- Reserve non-ops zips for cases where the user actually wants or accepts a non-ops fallback.

### Exact artifact trees to emit

When you emit a zip or directory artifact, make the tree unmistakable.

For `MODE: OPS_PATCH_BUNDLE`, emit exactly one top-level directory such as:

```text
patchship_YYYY-MM-DD_HHMM/
  manifest.yaml
  changes/
    0001.patch
    0002.patch
  summary.md                # optional
  constraints.yaml          # optional
  checks_request.yaml       # optional
  commit_message.txt        # optional
  tasks/                    # optional
    USER_TASKS.md
    TASKS.yaml
    ENV_TEMPLATE.env
```

For `MODE: NONOPS_EDIT_PACKAGE`, emit exactly one top-level directory such as:

```text
DO_NOT_LOOP_nonops_YYYY-MM-DD_HHMM/
  README_NOT_OPS.md
  overlay/
    ... full file contents ...
  edits/
    0001-unified.diff
  meta/
    commit_message.txt
    verification_checklist.md
```

For `MODE: ANALYSIS_ONLY`, do not emit a zip unless the user explicitly asks for one:

```text
MODE: ANALYSIS_ONLY
(no bundle tree)
```

Never mix these trees. Do not put `README_NOT_OPS.md` inside a patch bundle, and do not put `manifest.yaml` inside a non-ops package.

---

## 4. Customize this section: repository identity

Replace this section with project-specific facts.

Suggested shape:

- Repository name:
- Product / service summary:
- Primary languages / frameworks:
- Main runtime targets:
- Main package or app entrypoints:
- Current development branch policy:

Example:

```md
- Repository name: example-service
- Product / service summary: internal web API for order workflows
- Primary languages / frameworks: Rust CLI + TypeScript frontend
- Main runtime targets: Linux CI, macOS developer machines
- Main package or app entrypoints: `src/main.rs`, `web/package.json`
- Current development branch policy: develop-first, squash merge to main
```

---

## 5. Customize this section: read order for this repository

List the files the AI must read before making changes.
Put the repository-specific read order here, not just the generic diffship defaults.

Suggested shape:

1. Core spec or architecture doc
2. Feature-specific spec / decision log
3. Relevant package or module README
4. Test file that defines expected behavior
5. Any generated local guides under `.diffship/`

Example:

```md
1. `docs/SPEC_V1.md`
2. `docs/DECISIONS.md`
3. `apps/api/README.md`
4. `tests/m4_verify_profiles.rs`
5. `.diffship/PROJECT_KIT.md`
```

---

## 6. Customize this section: directory map and ownership boundaries

Explain which directories matter and what kinds of changes are expected or forbidden.

Suggested shape:

| Path | Meaning | Safe AI changes | Avoid / escalate |
|---|---|---|---|
| `src/` | main CLI implementation | feature work, tests, doc-linked edits | broad refactors without spec changes |
| `docs/` | contracts and operational docs | keep in sync with behavior | inventing undocumented behavior |
| `tests/` | integration coverage | add/update alongside behavior changes | removing coverage without cause |

Add repo-specific rows for generated code, migrations, vendor code, deployment files, or anything that needs stronger guardrails.

---

## 7. Customize this section: commands and quality gates

Record the real commands the AI should optimize for.

Suggested shape:

- Format:
- Lint:
- Unit tests:
- Integration tests:
- Full gate:
- Optional local-only helpers:

Example:

```md
- Format: `cargo fmt --all`
- Lint: `cargo clippy --all-targets --all-features -- -D warnings`
- Unit tests: `cargo test --lib`
- Integration tests: `cargo test --tests`
- Full gate: `just ci`
- Optional local-only helpers: `just docs-check`, `just trace-check`
```

If a command is expensive, flaky, or platform-specific, say so explicitly.

---

## 8. Core contract: what the AI is expected to produce

### 8.1 For review or planning tasks

When the user asks for analysis only, produce:

- findings grouped by priority or severity
- references to the affected files
- missing tests or docs if behavior changed
- next-step tasks with completion conditions when asked

### 8.2 For implementation tasks

When the user asks for code changes, produce the smallest complete change that matches the task:

- code edits
- relevant tests
- relevant docs/spec/traceability updates
- a clear commit message proposal

If the environment supports diffship patch bundles **and** you know the exact target `base_commit` **and** the requested change can be represented within the patch restrictions below, prefer returning an ops-compatible patch bundle.
Otherwise follow the mode-selection rules above.

### 8.3 Required output shape for ops-compatible patch bundles

The expected structure is exactly:

```text
patchship_YYYY-MM-DD_HHMM/
  manifest.yaml
  changes/
    0001.patch
  summary.md              # optional
  constraints.yaml        # optional
  checks_request.yaml     # optional
  commit_message.txt      # optional but recommended
  tasks/                  # optional
    USER_TASKS.md
    TASKS.yaml
    ENV_TEMPLATE.env
```

Required contract details:

- `manifest.yaml` must include `protocol_version`, `task_id`, `base_commit`, `apply_mode`, and `touched_files`
- `apply_mode` must be exactly `git-apply` or `git-am`; never use `patch`
- if `apply_mode=git-am`, use `From: Diffship <diffship@example.com>` as the default mail-style patch author unless the repository guidance explicitly defines another tool identity
- do not use provider- or model-specific identities such as `OpenAI <assistant@example.com>` as the default `From:` line
- `base_commit` must be the real target repo SHA supplied by the user or otherwise known from the environment; never use placeholders such as `REPLACE_WITH_REPO_HEAD`
- if the exact `base_commit` is not known, do **not** fabricate it and do **not** emit an ops-compatible patch bundle
- patch files must be repo-relative and deterministic
- do not touch `.git/` or `.diffship/`
- respect any repository-local `[ops.forbid]` patterns the user provides (for example lockfiles such as `pnpm-lock.yaml`) in addition to the built-in forbidden paths
- do not include secrets
- do not include binary patches, rename/copy metadata, file mode metadata for existing files (`old mode`, `new mode`), or submodule changes
- new file additions may use `new file mode 100644|100755` only when represented as a normal `/dev/null -> b/<path>` add patch in standard `MODE: OPS_PATCH_BUNDLE`
- if the requested change would require refused metadata in this environment, return `MODE: ANALYSIS_ONLY` or an explicitly accepted non-ops fallback instead of an invalid patch bundle
- reserve the `patchship_...` name for bundles that are already valid ops inputs

Required placement rules:

- the archive must contain exactly one top-level directory
- `manifest.yaml` must be at the bundle root, not nested under `meta/`, `overlay/`, or an extra parent directory
- `changes/` must be at the bundle root and contain ordered patch files

Recommended minimal `manifest.yaml`:

```yaml
protocol_version: "1"
task_id: "replace-with-task-id"
base_commit: "<exact 40-hex SHA>"
apply_mode: git-apply
touched_files:
  - path/to/file.ext
```

Recommended `git-am` header when mail-style patches are required:

```patch
From 0000000000000000000000000000000000000000 Mon Sep 17 00:00:00 2001
From: Diffship <diffship@example.com>
Date: Thu, 1 Jan 1970 00:00:00 +0000
Subject: [PATCH] replace-with-task-id
```

If the repository wants the final promoted commit author to be the local human operator by default, prefer `apply_mode: git-apply` rather than inventing a human `From:` identity.

### 8.4 Required output shape for non-ops edit packages

Use this only when the mode-selection rules allow `MODE: NONOPS_EDIT_PACKAGE`. The tree should be visibly different from a loop-ready patch bundle.

```text
DO_NOT_LOOP_nonops_YYYY-MM-DD_HHMM/
  README_NOT_OPS.md
  overlay/
  edits/
  meta/
```

Rules:

- include `README_NOT_OPS.md` at the bundle root
- the first sentence must be: `This artifact is intentionally not ops-compatible and must not be fed to diffship loop.`
- never include a fake or placeholder `manifest.yaml`
- never use the `patchship_...` prefix
- prefer the `DO_NOT_LOOP_` prefix so humans do not mistake the archive for an ops bundle

### 8.5 Self-check before finalizing an ops bundle

Before returning `MODE: OPS_PATCH_BUNDLE`, verify all of the following:

1. the archive has exactly one top-level directory
2. the bundle root contains `manifest.yaml`
3. the bundle root contains `changes/`
4. `changes/` contains at least one ordered patch file such as `0001.patch`
5. `manifest.yaml` uses a real `base_commit`
6. `apply_mode` is `git-apply` or `git-am`
7. if `apply_mode=git-am`, the patch mail header uses the repository-approved tool identity (default: `Diffship <diffship@example.com>`)
8. no forbidden patch metadata is present
9. the archive does not contain `README_NOT_OPS.md`

If any item fails, do not return `MODE: OPS_PATCH_BUNDLE`.

---

## 9. Core contract: meaning of files the user may provide

### 9.1 Handoff-side files

- `HANDOFF.md`: the map; read this first
- `parts/part_XX.patch`: the primary code diff payload
- `attachments.zip`: binary or raw attachments when explicitly included
- `excluded.md`: files intentionally omitted from the bundle
- `secrets.md`: secret warnings; never print secret values back
- `plan.toml`: replayable build settings for the handoff bundle

### 9.2 Ops-side files

- `manifest.yaml`: patch bundle metadata and apply contract; this must already be valid when delivered
- `changes/*.patch`: the patch payload to apply
- `commit_message.txt`: commit text diffship may use during promotion
- `tasks/USER_TASKS.md`: manual work required from the user
- `tasks/ENV_TEMPLATE.env`: environment variable template with placeholders only

Do not use `tasks/USER_TASKS.md` to ask the user to repair an invalid manifest or change `apply_mode`. If the bundle cannot be made valid, use the mode-selection rules instead.

### 9.3 Project guidance files

- `docs/SPEC_V1.md`: source of truth for product behavior
- `docs/BUNDLE_FORMAT.md`: handoff bundle contract
- `docs/PATCH_BUNDLE_FORMAT.md`: ops patch bundle contract
- `docs/TRACEABILITY.md`: mapping from spec IDs to tests/code
- `.diffship/PROJECT_KIT.md`: human-oriented local workflow guide

---

## 10. Customize this section: project-specific change rules

Use this section for local rules that are too specific for the generic contract.

Suggested topics:

- naming conventions
- module boundaries
- migration policy
- generated files policy
- release-note or changelog requirements
- branch / commit message expectations
- when to stop and ask the user instead of proceeding

Example:

```md
- Do not edit `api/generated/` by hand; change the schema and regenerate.
- Keep commit messages in Conventional Commit style.
- If a schema change affects production migrations, stop and ask for approval before writing migration files.
- Frontend changes must preserve the existing design system tokens in `web/src/theme/`.
```

---

## 11. Core contract: additional deliverables beyond file edits

Depending on the task, the AI may also need to provide:

- a proposed commit message
- a short verification checklist or commands to run
- explicit warnings about follow-up user tasks
- notes on whether docs/spec/traceability must be updated
- an explicit statement when the response is **not** ops-compatible and must not be fed to `diffship loop` as-is

When commit text is needed for ops flow, put it in `commit_message.txt`.
When manual user action is required, document it in `tasks/USER_TASKS.md`.

---

## 12. Customize this section: ready-to-send prompts for users

Add repository-specific prompts that users can copy.

Suggested examples:

### Ask for a loop-ready artifact

```text
Read `.diffship/AI_GUIDE.md`, `.diffship/PROJECT_KIT.md`, and the listed repository docs first.
Return `MODE: OPS_PATCH_BUNDLE` only if you can produce a valid diffship patch bundle.
Here is the exact target base commit: <paste git rev-parse HEAD here>.
If that information is insufficient or the patch would violate diffship restrictions, do not create a fallback zip.
Instead return `MODE: ANALYSIS_ONLY` and tell me exactly what is missing.
```

### Ask for a non-ops fallback explicitly

```text
A loop-ready bundle is not required for this task.
If needed, you may return `MODE: NONOPS_EDIT_PACKAGE` with a `DO_NOT_LOOP_nonops_.../` root and `README_NOT_OPS.md` at the top.
```

### Ask for analysis only

```text
Do not produce any zip for this task.
Return `MODE: ANALYSIS_ONLY` with findings, file references, and the smallest next-step plan.
```

---

## 13. Core contract: operating rules

1. Read the spec and relevant contracts first.
2. Keep scope tight; avoid unrelated refactors.
3. Update tests and docs together when behavior changes.
4. Prefer deterministic output ordering and stable file contents.
5. Never embed secrets or ask diffship to run arbitrary AI-provided commands.
