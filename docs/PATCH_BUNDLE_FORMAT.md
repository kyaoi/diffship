# Patch Bundle Format (v1)

This document defines the **patch bundle contract** consumed by `diffship apply` / `diffship loop`.

A patch bundle is typically produced by an AI agent and applied locally by a human.

> Design goal: patch bundles should be **strictly machine-validated** before touching the repo.

---

## 1. Directory layout

A patch bundle is either:
- a directory, or
- a zip file containing the directory at its root.

Recommended layout:

```
patchship_YYYY-MM-DD_HHMM/
  manifest.yaml
  changes/
    0001.patch
    0002.patch
  summary.md            # optional
  constraints.yaml      # optional
  checks_request.yaml   # optional
  commit_message.txt    # optional
  tasks/               # optional (user actions)
    USER_TASKS.md
    TASKS.yaml
    ENV_TEMPLATE.env
```

Notes:
- `manifest.yaml` and `changes/*.patch` are required.
- File names under `changes/` should be ordered (e.g. `0001.patch`, `0002.patch`).

---

## 2. `manifest.yaml`

### 2.1 Required fields

- `protocol_version`: string (e.g., `1`)
- `task_id`: string (free-form)
- `base_commit`: string (40-hex full SHA recommended)
- `apply_mode`: `git-apply` or `git-am`
- `touched_files`: list of repo-relative paths

### 2.2 Optional fields

- `created_by`: string (e.g., model/agent name)
- `created_at`: string (ISO 8601)
- `requires_docs_update`: bool
- `requires_plan_update`: bool
- `notes`: string
- `tasks_required`: bool (if true, the bundle should include `tasks/USER_TASKS.md`)
- `secrets_ack_required`: bool (if true, ops should require explicit user acknowledgement)

### 2.3 Example

```yaml
protocol_version: "1"
task_id: "OPS-FOUNDATION"
base_commit: "0123456789abcdef0123456789abcdef01234567"
apply_mode: "git-apply"
touched_files:
  - "docs/SPEC_V1.md"
  - "src/main.rs"
created_by: "ChatGPT"
requires_docs_update: true
```

---

## 3. `changes/*.patch`

- UTF-8, LF
- Must not include binary patches (`GIT binary patch`) unless explicitly allowed
- Must use repo-relative paths (no absolute paths)

---

## 4. Optional files

### 4.1 `summary.md`
Human-facing summary of what the patch intends to do.

### 4.2 `constraints.yaml`
Constraints the AI should follow during iteration.

### 4.3 `checks_request.yaml`
A hint for which verification profile to run (e.g. `fast|standard|full`).

### 4.4 `commit_message.txt`
A proposed commit message. diffship may expose it for copy/paste, and may also use it for auto-commit when commit policy is set to `auto`.

Commit behavior is controlled by diffship configuration (global/project/CLI) and is independent from `apply_mode`.


---

## 5. User tasks

Patch bundles may include a `tasks/` directory to describe actions the user must perform manually (e.g., create `.env`, rotate tokens, run a one-off migration).

Recommended files:

- `tasks/USER_TASKS.md`: human-readable checklist (required if `tasks_required: true`)
- `tasks/TASKS.yaml`: machine-readable tasks (optional)
- `tasks/ENV_TEMPLATE.env`: optional template for environment variables (never include real secrets)

diffship should surface these tasks prominently during `apply/loop`, and blocks promotion by default until the user acknowledges (use `--ack-tasks`).
