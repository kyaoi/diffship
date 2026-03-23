# Patch Bundle Format (v1)

This document defines the **patch bundle contract** consumed by `diffship apply` / `diffship loop`.
A patch bundle is typically produced by an AI agent and applied locally by a human.

> Design goal: patch bundles should be **strictly machine-validated** before touching the repo.

---

## 1. Directory layout

A patch bundle is either:

- a directory, or
- a zip file containing exactly one top-level directory at its root.

Minimal accepted tree:

```text
patchship_YYYY-MM-DD_HHMM/
  manifest.yaml
  changes/
    0001.patch
```

Full recommended tree:

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

Rejected lookalike (not a patch bundle):

```text
DO_NOT_LOOP_nonops_YYYY-MM-DD_HHMM/
  README_NOT_OPS.md
  overlay/
  edits/
  meta/
```

Notes:

- `manifest.yaml` and `changes/*.patch` are required at the same bundle root.
- File names under `changes/` should be ordered (for example `0001.patch`, `0002.patch`).
- The `patchship_...` name is reserved for bundles that are already valid ops inputs.
- Archives such as `DO_NOT_LOOP_nonops_...` are **not** patch bundles and must not be passed to `diffship loop`.
- `manifest.yaml` nested under another directory such as `meta/` or an extra parent folder does not satisfy this contract.

---

## 2. Producer preconditions

Before producing an ops-compatible patch bundle, the producer must know the target repo base exactly.

Required preconditions:

- the producer knows the target repo `base_commit`
- the patch can be expressed without violating the patch restrictions below
- the producer is returning a bundle that is already valid when delivered

If these conditions are not met, do **not** fabricate fields and do **not** ask the user to repair the bundle afterward.
Return `MODE: ANALYSIS_ONLY`, unified diffs, file-by-file edits, or an explicitly accepted non-ops package instead.

If the user asked for something they can pass to `diffship loop`, missing `base_commit` should normally lead to `MODE: ANALYSIS_ONLY`, not a misleading fallback zip.

---

## 3. `manifest.yaml`

### 3.1 Required fields

- `protocol_version`: string (for example `1`)
- `task_id`: string (free-form)
- `base_commit`: string (40-hex full SHA recommended)
- `apply_mode`: `git-apply` or `git-am`
- `touched_files`: list of repo-relative paths

### 3.2 Optional fields

- `created_by`: string (for example model/agent name)
- `created_at`: string (ISO 8601)
- `requires_docs_update`: bool
- `requires_plan_update`: bool
- `notes`: string
- `tasks_required`: bool (if true, the bundle should include `tasks/USER_TASKS.md`)
- `secrets_ack_required`: bool (if true, ops should require explicit user acknowledgement)
- `verify_profile`: string (`fast|standard|full`; bundle-level default)
- `target_branch`: string (promotion target branch name)
- `promotion_mode`: string (`none|working-tree|commit`)
- `commit_policy`: string (`auto|manual`)

### 3.3 Rules

- `base_commit` must be the exact target repo SHA.
- `apply_mode` must be exactly `git-apply` or `git-am`.
- Placeholder values such as `REPLACE_WITH_REPO_HEAD` are invalid.
- If the exact `base_commit` is unknown, do not emit a patch bundle.
- A local human operator MAY correct a stale manifest base with `diffship apply --base-commit <rev>` or `diffship loop --base-commit <rev>`, but that override still has to match the local session HEAD before apply proceeds.

Minimal example:

```yaml
protocol_version: "1"
task_id: "replace-with-task-id"
base_commit: "<exact 40-hex SHA>"
apply_mode: git-apply
touched_files:
  - path/to/file.ext
```

### 3.4 `git-am` author identity

When `apply_mode: git-am`, the patch payload is expected to use a stable tool identity in its mail-style headers.

Recommended default header:

```patch
From: Diffship <diffship@example.com>
```

Rules:

- use `Diffship <diffship@example.com>` as the default author identity for AI-generated `git-am` patches unless a repository-specific override says otherwise
- do not default to provider-specific identities such as `OpenAI <assistant@example.com>`
- if a repository wants the final promoted commit author to be the local human operator, prefer `apply_mode: git-apply` or a documented post-promotion author-reset flow instead of fabricating a human `From:` line

---

## 4. Patch restrictions

The patch payload must be repo-relative and deterministic.

Do not include:

- binary patches
- rename / copy metadata
- file mode metadata for existing files (`old mode`, `new mode`)
- submodule changes
- writes into `.git/` or non-allowlisted `.diffship/` paths
- secrets

Allowed exception:

- new file additions may use `new file mode 100644` or `new file mode 100755` only when the patch is a normal add diff from `/dev/null` to `b/<path>`

If the requested change cannot be represented without violating these restrictions, do not return a malformed ops bundle.
Use `MODE: ANALYSIS_ONLY` or an explicitly accepted non-ops fallback instead.

---

## 5. Human quick check before `diffship loop`

Before passing a zip to `diffship loop`, verify:

1. the archive is intended to be `MODE: OPS_PATCH_BUNDLE`
2. the archive root is a single directory
3. the bundle root contains `manifest.yaml`
4. the bundle root contains `changes/`
5. `changes/` contains at least one ordered patch file such as `0001.patch`
6. the archive does **not** contain `README_NOT_OPS.md`
7. the archive name or root path does **not** contain `DO_NOT_LOOP`

If any check fails, do not run `diffship loop` on that archive.
