# diffship Configuration

diffship resolves configuration by **merging multiple sources** (later overrides earlier):

1. built-in defaults
2. `~/.config/diffship/config.toml` (global)
3. `./.diffship.toml` (project, legacy)
4. `./.diffship/config.toml` (project; written by `diffship init`)
5. patch bundle `manifest.yaml` (when available)
6. CLI flags

Precedence summary: **CLI > manifest > project > global > default**.

Notes:
- `diffship verify/promote` resolve manifest-level settings by reading the run copy at `.diffship/runs/<run-id>/bundle/manifest.yaml`.
- This file documents more keys than are currently consumed by the implementation; unknown keys are ignored.

All examples below are TOML.

---

## 1. Profiles (upload limits)

Built-in default profile:

- Name: `20x512`
- `max_parts = 20`
- `max_bytes_per_part = 536870912` (512 MiB)

Custom profiles:

```toml
default_profile = "20x512"

[profiles."20x512"]
max_parts = 20
max_bytes_per_part = 536870912

[profiles."10x100"]
max_parts = 10
max_bytes_per_part = 104857600 # 100 MiB
```

Optional heuristic token limit:

```toml
[profiles."20x512"]
max_parts = 20
max_bytes_per_part = 536870912
max_approx_tokens_per_part = 2000000
```

---

## 2. Diff options

```toml
[diff]
unified = 3
renames = "auto"      # auto|on|off
include_binary = false
binary_mode = "raw"   # raw|patch|meta
```

---

## 3. Sources (segments)

```toml
[sources]
include_committed = true
include_staged = false
include_unstaged = false
include_untracked = false
```

---

## 4. Untracked options

```toml
[untracked]
mode = "auto" # auto|patch|raw|meta
threshold_bytes = 200000
binary_globs = ["*.png", "*.jpg", "*.pdf", "*.zip"]
```

---

## 5. Split mode

```toml
[split]
by = "auto" # auto|file|commit
```

---

## 6. Secrets warnings

```toml
[secrets]
enabled = true
fail_on_secrets = false
```

---

## 7. Ignore file: `.diffshipignore`

diffship supports a gitignore-like file `.diffshipignore` to exclude paths.

---

## 8. Ops settings (apply/verify/loop)

These settings control safety defaults for operations.

```toml
[ops]
run_dir = ".diffship/runs"
lock_path = ".diffship/lock"

# Safe defaults (recommended)
require_clean_tree = true
require_base_commit_match = true
rollback_on_apply_failure = true

# Refuse tricky patch features in MVP
refuse_binary_patch = true
refuse_submodule = true
refuse_mode_change = true
refuse_rename_copy = true

[ops.paths]
# Glob-like patterns. If `allowed` is non-empty, anything not matching is refused.
allowed = []
forbidden = [
  ".git/**",
  ".diffship/**",
  ".env",
  ".env.*",
  "**/secrets/**",
]
```

---

## 9. Verify profiles

`diffship verify` runs a named profile consisting of one or more shell commands.

```toml
[verify]
default_profile = "standard"

[verify.profiles.fast]
commands = [
  "cargo fmt --all -- --check",
  "cargo clippy --all-targets --all-features -- -D warnings",
]

[verify.profiles.standard]
commands = [
  "cargo fmt --all -- --check",
  "cargo clippy --all-targets --all-features -- -D warnings",
  "cargo test",
]

[verify.profiles.full]
commands = [
  "just ci",
]
```

Notes:
- diffship should log stdout/stderr per command under the run directory.
- Patch bundles may include `checks_request.yaml` as a *hint*, but diffship should only run locally configured profiles.


---

## 10. OS mode (sessions, sandboxes, promotion)

OS mode enables repeated `apply/verify/loop` runs without requiring the user’s main working tree to stay clean.
diffship achieves this by using Git worktrees: a persistent session worktree plus per-run sandbox worktrees.

```toml
[ops.os]
enabled = true
# Where diffship creates worktrees for sessions and sandboxes
worktrees_dir = ".diffship/worktrees"
# Default session name
session = "default"
# Internal refs used for sessions (kept out of refs/heads by default)
session_ref_prefix = "refs/diffship/sessions"
cleanup_sandboxes = true

[ops.promote]
# Promotion decides how results are reflected back onto the user’s branch.
# - none: keep only the session state
# - working-tree: apply changes onto the target branch working tree (no commit)
# - commit: apply + commit onto the target branch
mode = "commit"
target_branch = "develop"

[ops.commit]
# - manual: never commit automatically (show commit_message.txt for copy/paste)
# - auto: commit automatically after successful apply+verify
policy = "auto"
# If commit_message.txt is missing, use a deterministic fallback template
fallback_template = "{task_id}: apply patch bundle"

[ops.secrets]
# - warn: print warnings but allow promotion
# - block: require explicit acknowledgement to promote
policy = "block"
ack_flag = "--ack-secrets"

[ops.tasks]
# - warn: show tasks but allow promotion
# - block: require explicit acknowledgement to promote
policy = "warn"
ack_flag = "--ack-tasks"
```

Notes:
- `ops.require_clean_tree` and `ops.require_base_commit_match` apply to the **session/sandbox** worktree in OS mode.
- The user’s current working directory is not mutated during apply/verify; only promotion may affect the target branch.
