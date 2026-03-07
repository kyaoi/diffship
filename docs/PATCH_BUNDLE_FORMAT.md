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

```text
patchship_YYYY-MM-DD_HHMM/
  manifest.yaml
  changes/
    0001.patch
    0002.patch
  summary.md              # optional
  constraints.yaml        # optional
  checks_request.yaml     # optional
  commit_message.txt      # optional
  tasks/                  # optional (user actions)
    USER_TASKS.md
    TASKS.yaml
    ENV_TEMPLATE.env
```

Notes:

- `manifest.yaml` and `changes/*.patch` are required.
- File names under `changes/` should be ordered (e.g. `0001.patch`, `0002.patch`).

---

## 2. Producer preconditions

Before producing an ops-compatible patch bundle, the producer must know the target repo base exactly.

Required preconditions:

- the producer knows the target repo `base_commit`
- the patch can be expressed without violating the patch restrictions below
- the producer is returning a bundle that is already valid when delivered

If these conditions are not met, do **not** fabricate fields and do **not** ask the user to repair the bundle afterward.
Return unified diffs, file-by-file edits, or review notes instead.

---

## 3. `manifest.yaml`

### 3.1 Required fields

- `protocol_version`: string (e.g., `1`)
- `task_id`: string (free-form)
- `base_commit`: string (40-hex full SHA recommended)
- `apply_mode`: `git-apply` or `git-am`
- `touched_files`: list of repo-relative paths

### 3.2 Optional fields

- `created_by`: string (e.g., model/agent name)
- `created_at`: string (ISO 8601)
- `requires_docs_update`: bool
- `requires_plan_update`: bool
- `notes`: string
- `tasks_required`: bool (if true, the bundle should include `tasks/USER_TASKS.md`)
- `secrets_ack_required`: bool (if true, ops should require explicit user acknowledgement)
- `verify_profile`: string (fast|standard|full; bundle-level default)
- `target_branch`: string (promotion target branch name)
- `promotion_mode`: string (none|working-tree|commit)
- `commit_policy`: string (auto|manual)

### 3.3 Rules

- `base_commit` must be the exact target repo SHA. Never use placeholders such as `REPLACE_WITH_REPO_HEAD`.
- `apply_mode` must be exactly `git-apply` or `git-am`. Values such as `patch` are invalid.
- `touched_files` must contain repo-relative paths only.
- Bundles must not target `.git/` or `.diffship/`.

### 3.4 Example

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

## 4. `changes/*.patch`

General requirements:

- UTF-8
- LF line endings
- deterministic ordering under `changes/`
- repo-relative paths only (no absolute paths, no traversal)

Current v1 safety restrictions:

- binary patches (`GIT binary patch`) are refused
- rename/copy metadata (`rename from`, `rename to`, `copy from`, `copy to`) is refused
- file mode metadata (`old mode`, `new mode`, `new file mode`) is refused
- submodule changes (mode `160000`) are refused

If the requested change would require one of the refused constructs in the current environment, do not ship an ops-compatible patch bundle.
Return a non-ops format instead.

---

## 5. Optional files

### 5.1 `summary.md`

Human-facing summary of what the patch intends to do.

### 5.2 `constraints.yaml`

Constraints the AI should follow during iteration.

### 5.3 `checks_request.yaml`

A hint for which verification profile to run (e.g. `fast|standard|full`).

### 5.4 `commit_message.txt`

A proposed commit message.

diffship may expose it for copy/paste, and may also use it for auto-commit when commit policy is set to `auto`.
Commit behavior is controlled by diffship configuration (global/project/CLI) and is independent from `apply_mode`.

---

## 6. User tasks

Patch bundles may include a `tasks/` directory to describe actions the user must perform manually (e.g., create `.env`, rotate tokens, run a one-off migration).

Recommended files:

- `tasks/USER_TASKS.md`: human-readable checklist (required if `tasks_required: true`)
- `tasks/TASKS.yaml`: machine-readable tasks (optional)
- `tasks/ENV_TEMPLATE.env`: optional template for environment variables (never include real secrets)

Use user tasks only for real follow-up work owned by the user.
Do not use them to ask the user to fill in missing manifest fields or otherwise repair an invalid bundle.

diffship should surface these tasks prominently during `apply/loop`, and blocks promotion by default until the user acknowledges (use `--ack-tasks`).
