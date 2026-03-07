# Diffship AI Guide (template)

This document is a template used by `diffship init`.

Its audience is an AI coding agent that receives files from a user working with diffship.
The goal is to make the workflow, output contract, and repository-specific expectations explicit.

Use this template in two layers:

- Keep the **core contract** sections aligned with diffship behavior.
- Replace the **customize this section** blocks with repository-specific guidance.

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
- if required metadata is unavailable, fall back to unified diffs / file edits / review notes instead of inventing fields

---

## 2. Customize this section: repository identity

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

## 3. Customize this section: read order for this repository

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

## 4. Customize this section: directory map and ownership boundaries

Explain which directories matter and what kinds of changes are expected or forbidden.

Suggested shape:

| Path | Meaning | Safe AI changes | Avoid / escalate |
|---|---|---|---|
| `src/` | main CLI implementation | feature work, tests, doc-linked edits | broad refactors without spec changes |
| `docs/` | contracts and operational docs | keep in sync with behavior | inventing undocumented behavior |
| `tests/` | integration coverage | add/update alongside behavior changes | removing coverage without cause |

Add repo-specific rows for generated code, migrations, vendor code, deployment files, or anything that needs stronger guardrails.

---

## 5. Customize this section: commands and quality gates

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

## 6. Core contract: what the AI is expected to produce

### 6.1 For review or planning tasks

When the user asks for analysis only, produce:

- findings grouped by priority or severity
- references to the affected files
- missing tests or docs if behavior changed
- next-step tasks with completion conditions when asked

### 6.2 For implementation tasks

When the user asks for code changes, produce the smallest complete change that matches the task:

- code edits
- relevant tests
- relevant docs/spec/traceability updates
- a clear commit message proposal

If the environment supports diffship patch bundles **and** you know the exact target `base_commit` **and** the requested change can be represented within the patch restrictions below, prefer returning an ops-compatible patch bundle.
Otherwise return unified diffs or file-by-file edits.

### 6.3 Required output shape for ops-compatible patch bundles

The expected structure is:

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
- `base_commit` must be the real target repo SHA supplied by the user or otherwise known from the environment; never use placeholders such as `REPLACE_WITH_REPO_HEAD`
- if the exact `base_commit` is not known, do **not** fabricate it and do **not** emit an ops-compatible patch bundle
- patch files must be repo-relative and deterministic
- do not touch `.git/` or `.diffship/`
- do not include secrets
- do not include binary patches, rename/copy metadata, file mode metadata (`old mode`, `new mode`, `new file mode`), or submodule changes
- if the requested change would require refused metadata in this environment, return unified diffs / file edits / review notes instead of an invalid patch bundle

---

## 7. Core contract: meaning of files the user may provide

### 7.1 Handoff-side files

- `HANDOFF.md`: the map; read this first
- `parts/part_XX.patch`: the primary code diff payload
- `attachments.zip`: binary or raw attachments when explicitly included
- `excluded.md`: files intentionally omitted from the bundle
- `secrets.md`: secret warnings; never print secret values back
- `plan.toml`: replayable build settings for the handoff bundle

### 7.2 Ops-side files

- `manifest.yaml`: patch bundle metadata and apply contract; this must already be valid when delivered
- `changes/*.patch`: the patch payload to apply
- `commit_message.txt`: commit text diffship may use during promotion
- `tasks/USER_TASKS.md`: manual work required from the user
- `tasks/ENV_TEMPLATE.env`: environment variable template with placeholders only

Do not use `tasks/USER_TASKS.md` to ask the user to repair an invalid manifest or change `apply_mode`. If the bundle cannot be made valid, return a non-ops format instead.

### 7.3 Project guidance files

- `docs/SPEC_V1.md`: source of truth for product behavior
- `docs/BUNDLE_FORMAT.md`: handoff bundle contract
- `docs/PATCH_BUNDLE_FORMAT.md`: ops patch bundle contract
- `docs/TRACEABILITY.md`: mapping from spec IDs to tests/code
- `.diffship/PROJECT_KIT.md`: human-oriented local workflow guide

---

## 8. Customize this section: project-specific change rules

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

## 9. Core contract: additional deliverables beyond file edits

Depending on the task, the AI may also need to provide:

- a proposed commit message
- a short verification checklist or commands to run
- explicit warnings about follow-up user tasks
- notes on whether docs/spec/traceability must be updated
- an explicit statement when the response is **not** ops-compatible and must not be fed to `diffship loop` as-is

When commit text is needed for ops flow, put it in `commit_message.txt`.
When manual user action is required, document it in `tasks/USER_TASKS.md`.

---

## 10. Customize this section: ready-to-send prompts for users

Add a few repository-specific prompts the user can paste into an AI tool.

Suggested shapes:

- review-only prompt
- implementation prompt
- docs-sync prompt
- regression triage prompt

Example:

```text
Read `docs/SPEC_V1.md`, `docs/DECISIONS.md`, and `tests/m6_handoff_build.rs` first.
The current target repo HEAD is `<paste git rev-parse HEAD here>`.
Implement the requested change with the smallest possible diff.
Only return an ops-compatible patch bundle if you can use that exact SHA in `manifest.yaml`
and can satisfy the diffship patch-bundle restrictions.
Otherwise return unified diffs or file-by-file edits.
Update docs and traceability in the same change.
Finish only when `just ci` would be expected to pass.
```

---

## 11. Core contract: operating rules

1. Read the spec and relevant contracts first.
2. Keep scope tight; avoid unrelated refactors.
3. Update tests and docs together when behavior changes.
4. Prefer deterministic output ordering and stable file contents.
5. Never embed secrets or ask diffship to run arbitrary AI-provided commands.
6. Never ask the user to fix placeholder values inside a supposedly ops-compatible bundle.
7. If the user will run `diffship loop`, remind them to keep the repo clean and store incoming zips outside the repo or under `.diffship/`.
