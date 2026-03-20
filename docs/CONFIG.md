# diffship Configuration

This document describes **how configuration is resolved** and which keys are **actually supported** by the current diffship binary.

> diffship is developed with spec-driven development.
> Ops verify profile commands under `[verify.profiles.*]` and handoff settings under `[handoff]` / `[handoff.profiles.*]` are now consumed by the current implementation.
> The generated `.diffship/config.toml` intentionally marks repository-owned edit points with "Customize this section" comments so the initial stub is easier to adopt.

---

## 0. Resolution / precedence

diffship resolves configuration by **merging multiple sources** (later overrides earlier):

1. built-in defaults
2. `~/.config/diffship/config.toml` (global)
3. `./.diffship.toml` (project, legacy)
4. `./.diffship/config.toml` (project; written by `diffship init`)
5. patch bundle `manifest.yaml` (when available)
6. CLI flags

Precedence summary: **CLI > manifest > project > global > default**.

Notes:

- Ops commands that need the manifest (verify/promote/loop) resolve manifest-level settings by reading the run copy at `.diffship/runs/<run-id>/bundle/manifest.yaml`.
- Unknown keys are ignored.
- Current config parsing is intentionally minimal (scalar values only).

---

## 1. Ops settings (implemented)

These are the keys consumed by the current implementation.

### 1.1 Verify profile

Choose which verification profile runs by default.

```toml
[verify]
default_profile = "standard" # fast|standard|full
```

CLI override:

- `diffship verify --profile <fast|standard|full>`
- `diffship loop ... --profile <fast|standard|full>`

Compatibility aliases (accepted, but not recommended for new configs):

```toml
[ops]
verify_profile = "standard"
```

#### 1.1.1 Custom verify profile commands (implemented)

You can define profile-specific command sequences under `[verify.profiles.<name>]`.
Each key value is executed as a local shell command (`sh -lc ...`) in the sandbox worktree.

```toml
[verify]
default_profile = "custom"

[verify.profiles.custom]
cmd1 = "cargo fmt --all -- --check"
cmd2 = "cargo clippy --all-targets --all-features -- -D warnings"
cmd3 = "cargo test"
```

Notes:

- If `[verify].default_profile` names a custom profile, it is accepted even when it is not `fast|standard|full`.
- CLI `--profile` still has top precedence and can override to `fast|standard|full` or another configured custom profile.
- Commands are loaded only from local config sources (global/project), not from the patch bundle.

#### 1.1.2 Post-apply commands (implemented)

You can define commands that run automatically after a successful `diffship apply` patch step and before `diffship verify` in `diffship loop`.

```toml
[ops.post_apply]
cmd1 = "just fmt-fix"
cmd2 = "just docs-check"
cmd3 = "just trace-check"
cmd4 = "just ci"
```

Notes:

- commands run in the sandbox worktree
- command order follows `cmd1`, `cmd2`, `cmd3`, ...
- failures are recorded under the run directory and make `apply` / `loop` fail
- commands are loaded only from local config sources (global/project), not from the patch bundle
- treat these as local normalizers (formatters, docs/spec sync, deterministic fixups), not as a way to repair invalid AI output

Suggested starting points:

- Rust-oriented repo: `cargo fmt --all`, `just docs-check`, `just trace-check`
- Node/TS-oriented repo: `pnpm exec prettier -w .`, `pnpm exec eslint . --fix`
- Docs/spec-oriented repo: `just docs-check`, `just trace-check`

`diffship init` now comments these presets into the generated project config stub so repositories can keep one or two narrow local normalization commands without turning `post_apply` into a hidden second verify phase.

#### 1.1.3 Extra forbidden patch targets (implemented)

You can define additional repo-relative path or glob patterns that local ops runs must refuse.

```toml
[ops.forbid]
path1 = "pnpm-lock.yaml"
path2 = "package-lock.json"
path3 = "apps/*/pnpm-lock.yaml"
```

Notes:

- these rules are local-only config values; patch bundles cannot loosen them
- apply/loop enforce them against both `manifest.touched_files` and patch diff headers
- built-in forbidden prefixes such as `.git/` and `.diffship/` still apply regardless of config

You can keep these entries either in `.diffship/config.toml` or in a dedicated `.diffship/forbid.toml` file.
`diffship init` now generates the dedicated file as a starter template, and diffship merges it as project-local config automatically.
If the repo gains new lockfiles or similar fragile targets later, `diffship init --refresh-forbid` rewrites only `.diffship/forbid.toml` from current detections without forcing unrelated generated files.

### 1.2 Promotion mode + target branch

```toml
[ops.promote]
mode = "commit"           # none|working-tree|commit
target_branch = "develop" # falls back to current branch if missing
```

CLI overrides:

- `diffship promote --promotion <...> --target-branch <...>`
- `diffship loop ... --promotion <...> --target-branch <...>`

Compatibility aliases:

```toml
[ops]
promotion_mode = "commit"
target_branch = "develop"
```

### 1.3 Commit policy

```toml
[ops.commit]
policy = "auto" # auto|manual
```

CLI override:

- `diffship promote --commit-policy <auto|manual>`
- `diffship loop ... --commit-policy <auto|manual>`

Compatibility aliases:

```toml
[ops]
commit_policy = "auto"
```

### 1.4 Bundle manifest fields (overrides)

If present, these fields in `manifest.yaml` override config defaults (unless CLI overrides them):

- `verify_profile`
- `target_branch`
- `promotion_mode`
- `commit_policy`

---

## 2. Ops acknowledgements (implemented, not configurable yet)

Promotion may be refused until the user explicitly acknowledges:

- secrets-like strings → `--ack-secrets`
- required user tasks in `tasks/USER_TASKS.md` → `--ack-tasks`

Today these policies are **built-in** and not configurable via TOML.

---

## 3. Handoff / TUI configuration

Only handoff packing profiles are consumed today. The other handoff/TUI sections below remain planned/spec-defined for future work.

All examples below are TOML.

### 3.1 Profiles (upload limits, implemented)

Built-in profiles:

- `20x512` (default) → `max_parts = 20`, `max_bytes_per_part = 536870912`
- `10x100` → `max_parts = 10`, `max_bytes_per_part = 104857600`

CLI:

- `diffship build --profile <name>`
- `diffship build --max-parts <n> --max-bytes-per-part <bytes>`
- `diffship build --out-dir <dir>`

Resolution:

- precedence is `CLI > project > global > built-in default`
- explicit `--max-parts` / `--max-bytes-per-part` override the selected profile values
- explicit `--out-dir` overrides `[handoff].output_dir`
- explicit `--out` bypasses `[handoff].output_dir` because it sets the exact output path
- TUI handoff screen uses the same resolved profile set and can cycle profiles with `h`

Config:

```toml
[handoff]
default_profile = "20x512"
output_dir = "./.diffship/artifacts/handoffs" # optional parent dir for auto-generated bundle names

[handoff.profiles."team-ci"]
max_parts = 8
max_bytes_per_part = 104857600 # 100 MiB
```

Compatibility alias (also accepted):

```toml
[profiles."team-ci"]
max_parts = 8
max_bytes_per_part = 104857600
```

The generated `plan.toml` records `profile` plus resolved numeric limits so replay remains stable even if config later changes.
To share named profiles across repositories, copy the relevant `[handoff.profiles.*]` stanzas into the target repo config (or into `~/.config/diffship/config.toml` for global reuse). `plan.toml` is intentionally narrower: it exports the selected profile name plus resolved limits, not the full profile catalog.
The optional `[handoff].output_dir` changes the parent directory used for auto-generated bundle names when `--out` is omitted.
If `output_dir` starts with a tilde-slash form such as ~/handoffs, diffship expands it against the current user's `HOME`. Tilde-user shorthand is not supported.
Compatibility alias `[handoff].out_dir` is also accepted.

### 3.2 Diff options

```toml
[diff]
unified = 3
renames = "auto"      # auto|on|off
include_binary = false
binary_mode = "raw"   # raw|patch|meta
```

### 3.3 Sources (segments)

```toml
[sources]
include_committed = true
include_staged = false
include_unstaged = false
include_untracked = false
```

### 3.4 Untracked options

```toml
[untracked]
mode = "auto" # auto|patch|raw|meta
threshold_bytes = 200000
binary_globs = ["*.png", "*.jpg", "*.pdf", "*.zip"]
```

### 3.5 Split mode

```toml
[split]
by = "auto" # auto|file|commit
```

### 3.6 Secrets warnings (handoff side)

```toml
[secrets]
enabled = true
fail_on_secrets = false
```

### 3.7 Ignore file: `.diffshipignore`

diffship supports a gitignore-like file `.diffshipignore` to exclude paths.

---

## 4. Related docs

- Ops workflow guide: `docs/OPS_WORKFLOW.md`
- Patch bundle contract: `docs/PATCH_BUNDLE_FORMAT.md`
- Spec (source of truth): `docs/SPEC_V1.md`
