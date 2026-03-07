# Diffship AI Guide (template)

This document is a template used by `diffship init`.

Its audience is an AI coding agent that receives files from a user working with diffship.
The goal is to make the expected workflow, output contract, and supporting artifacts explicit.

---

## 1. What diffship is

diffship is a local development OS for Git repositories.

It provides two connected workflows:

- **handoff**: package local changes into a deterministic AI-readable bundle
- **ops**: apply an AI-produced patch bundle safely in an isolated sandbox, then verify and promote it

diffship expects the AI to work within explicit contracts rather than inventing its own repo workflow.

Key expectations:

- read the provided project/spec files first
- keep changes minimal and deterministic
- produce outputs that diffship can verify or apply safely
- treat docs, tests, and traceability as part of the same change when behavior changes

---

## 2. What the AI is expected to produce

### 2.1 For review or planning tasks

When the user asks for analysis only, produce:

- findings grouped by priority or severity
- references to the affected files
- missing tests or docs if behavior changed
- next-step tasks with completion conditions when asked

### 2.2 For implementation tasks

When the user asks for code changes, produce the smallest complete change that matches the task:

- code edits
- relevant tests
- relevant docs/spec/traceability updates
- a clear commit message proposal

If the environment supports diffship patch bundles, prefer returning an ops-compatible patch bundle.
Otherwise return unified diffs or file-by-file edits.

### 2.3 Required output shape for ops-compatible patch bundles

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
- patch files must be repo-relative and deterministic
- do not touch `.git/` or `.diffship/`
- do not include secrets

---

## 3. Meaning of files the user may provide

### 3.1 Handoff-side files

- `HANDOFF.md`: the map; read this first
- `parts/part_XX.patch`: the primary code diff payload
- `attachments.zip`: binary or raw attachments when explicitly included
- `excluded.md`: files intentionally omitted from the bundle
- `secrets.md`: secret warnings; never print secret values back
- `plan.toml`: replayable build settings for the handoff bundle

### 3.2 Ops-side files

- `manifest.yaml`: patch bundle metadata and apply contract
- `changes/*.patch`: the patch payload to apply
- `commit_message.txt`: commit text diffship may use during promotion
- `tasks/USER_TASKS.md`: manual work required from the user
- `tasks/ENV_TEMPLATE.env`: environment variable template with placeholders only

### 3.3 Project guidance files

- `docs/SPEC_V1.md`: source of truth for product behavior
- `docs/BUNDLE_FORMAT.md`: handoff bundle contract
- `docs/PATCH_BUNDLE_FORMAT.md`: ops patch bundle contract
- `docs/TRACEABILITY.md`: mapping from spec IDs to tests/code
- `.diffship/PROJECT_KIT.md`: human-oriented local workflow guide

---

## 4. Additional deliverables beyond file edits

Depending on the task, the AI may also need to provide:

- a proposed commit message
- a short verification checklist or commands to run
- explicit warnings about follow-up user tasks
- notes on whether docs/spec/traceability must be updated

When commit text is needed for ops flow, put it in `commit_message.txt`.
When manual user action is required, document it in `tasks/USER_TASKS.md`.

---

## 5. Operating rules

1. Read the spec and relevant contracts first.
2. Keep scope tight; avoid unrelated refactors.
3. Update tests and docs together when behavior changes.
4. Prefer deterministic output ordering and stable file contents.
5. Never embed secrets or ask diffship to run arbitrary AI-provided commands.

