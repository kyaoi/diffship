# diffship Usage Guide

This guide describes how to use the **current implementation** of diffship end to end.

It is intentionally practical:

- what to run
- when to run it
- which outputs to expect
- where to look when something fails

For the formal product contract, see `docs/SPEC_V1.md`.

---

## 1. What diffship does

diffship supports two connected workflows:

1. **handoff**: collect Git changes into an AI-friendly bundle
2. **ops**: apply an AI-produced patch bundle safely, verify it, and promote it

In short:

1. run `diffship build`
2. inspect with `diffship preview` or `diffship compare`
3. send the handoff bundle to AI
4. receive a patch bundle back
5. run `diffship loop`

For the human workflow of what to send to ChatGPT/Claude/Codex, which response format to request, and how to use the result afterward, see `docs/AI_HANDOFF_FLOW.md`.

---

## 2. Install and setup

### 2.1 Local install

```bash
cargo install --path .
```

Or run without installing:

```bash
cargo run -- <subcommand> ...
```

### 2.2 Developer setup for this repository

```bash
mise install
lefthook install
just ci
```

---

## 3. Command map

### 3.1 Handoff side

- `diffship build`
- `diffship preview`
- `diffship compare`

### 3.2 Ops side

- `diffship init`
- `diffship status`
- `diffship runs`
- `diffship apply`
- `diffship verify`
- `diffship promote`
- `diffship loop`
- `diffship pack-fix`

### 3.3 TUI

- `diffship`
- `diffship tui`

When running in a TTY, `diffship` starts the TUI by default.

---

## 4. Quickstart

### 4.1 Handoff quickstart

Build a bundle from your latest committed change:

```bash
diffship build
```

Inspect it:

```bash
diffship preview ./diffship_YYYY-MM-DD_HHMM_<head7> --list
diffship preview ./diffship_YYYY-MM-DD_HHMM_<head7> --part part_01.patch
```

Compare two bundles when checking reproducibility:

```bash
diffship compare ./bundle_a ./bundle_b.zip
diffship compare ./bundle_a ./bundle_b.zip --json
```

### 4.2 Ops quickstart

Initialize a repository once:

```bash
diffship init
diffship init --zip
diffship init --zip --out ./.diffship/artifacts/rules/review-kit.zip
```

Run the full apply → verify → promote loop:

```bash
diffship loop ./patch-bundle.zip
diffship loop ./patch-bundle.zip --base-commit "$(git rev-parse HEAD)"
```

---

## 5. Handoff workflow in detail

### 5.1 Build from different sources

Only the latest committed change:

```bash
diffship build
```

Only staged / unstaged / untracked work:

```bash
diffship build --no-committed --include-staged --include-unstaged --include-untracked
```

Committed range with commit-oriented splitting:

```bash
diffship build --range-mode direct --from HEAD~3 --to HEAD --split-by commit
```

### 5.2 Filter paths

Apply the same filters across committed, staged, unstaged, and untracked segments:

```bash
diffship build --include 'src/*.rs' --include '*.md' --exclude 'src/generated.rs'
```

Ignore rules from `.diffshipignore` are also applied.

### 5.3 Control untracked and binary handling

Untracked files can be represented as patch, raw attachment, or metadata:

```bash
diffship build --no-committed --include-untracked --untracked-mode meta
```

Binary content is excluded by default. To include it:

```bash
diffship build --include-binary --binary-mode raw
```

Supported binary modes:

- `raw`: store file bytes in `attachments.zip`
- `patch`: keep patch text when possible
- `meta`: record metadata / exclusion information instead of bytes

### 5.4 Packing limits and profiles

Use a built-in profile:

```bash
diffship build --profile 10x100
```

Override the resolved limits directly:

```bash
diffship build --max-parts 10 --max-bytes-per-part 104857600
```

Current built-in profiles:

- `20x512`
- `10x100`

If packing overflows, diffship currently:

1. repacks deterministically
2. reduces diff context from `U3` to `U1` to `U0` when needed
3. records exclusions in `excluded.md` if units still do not fit

### 5.5 Secrets behavior

If secrets-like content is detected, diffship warns before completing the build.

Continue non-interactively:

```bash
diffship build --yes
```

Fail instead of continuing:

```bash
diffship build --fail-on-secrets
```

### 5.6 Inspect the result

List bundle contents:

```bash
diffship preview ./diffship_YYYY-MM-DD_HHMM_<head7> --list
```

Show a patch part:

```bash
diffship preview ./diffship_YYYY-MM-DD_HHMM_<head7> --part part_01.patch
```

Machine-readable preview:

```bash
diffship preview ./diffship_YYYY-MM-DD_HHMM_<head7> --list --json
```

### 5.7 Export and replay a plan

Export:

```bash
diffship build --include-untracked --plan-out ./diffship_plan.toml
```

Replay:

```bash
diffship build --plan ./diffship_plan.toml --out ./replayed_bundle
```

Current `plan.toml` behavior:

- stores the selected profile name
- stores resolved numeric limits
- stores handoff selection such as range, sources, filters, split mode, and binary mode
- does not store runtime output routing such as `--out` or `--out-dir`
- does not store the entire named profile catalog

Named profile definitions stay in config, not in the plan export.

### 5.8 Handoff output layout

Typical output:

- `HANDOFF.md`
- `parts/part_XX.patch`
- `attachments.zip` when raw attachments are included
- `excluded.md` when diff units are intentionally omitted
- `secrets.md` when secrets-like content is detected
- `plan.toml` when exported

Default output naming:

- `--out <path>` sets the exact output directory path
- `--out-dir <dir>` places the auto-generated bundle name under a custom parent directory
- filesystem path arguments such as bundle paths, `--out`, `--out-dir`, `--plan`, and `pack-fix --out` accept leading tilde-slash
- if `--out` is omitted, diffship uses a `diffship_YYYY-MM-DD_HHMM_<head7>` directory name
- the timestamp is rendered in the local system timezone
- if the base path already exists, diffship creates a suffixed name such as `diffship_YYYY-MM-DD_HHMM_<head7>_2`, then `_3`, and so on

Example:

```bash
diffship build --out-dir ./.diffship/artifacts/handoffs
```

This produces a bundle under `./.diffship/artifacts/handoffs/` while keeping the generated bundle name.

Project or global config can also set this default:

```toml
[handoff]
output_dir = "./.diffship/artifacts/handoffs"
```

Tilde-slash paths are also accepted here:

```toml
[handoff]
output_dir = "~/ghq/github.com/kyaoi/diffship/.diffship/handoffs"
```

diffship expands that path against the current user's `HOME`. Tilde-user shorthand is intentionally unsupported.

See `docs/BUNDLE_FORMAT.md` for the bundle contract.

---

## 6. Ops workflow in detail

### 6.1 Initialize a repository

```bash
diffship init
```

This writes:

- `.diffship/.gitignore`
- `.diffship/PROJECT_KIT.md`
- `.diffship/AI_GUIDE.md`
- `.diffship/config.toml`

To use project-specific init templates:

```bash
diffship init --template-dir ./templates/diffship
```

The directory may contain either or both of:

- `PROJECT_KIT_TEMPLATE.md`
- `AI_PROJECT_TEMPLATE.md`

Missing files fall back to the repository templates and then to built-in defaults.

`AI_PROJECT_TEMPLATE.md` is intentionally split into:

- core contract sections that should stay aligned with diffship behavior
- "Customize this section" blocks for repository-specific rules, commands, directory ownership, and ready-to-send prompts

That makes it practical to keep one stable diffship contract while still generating a repo-specific `.diffship/AI_GUIDE.md`.

`PROJECT_KIT_TEMPLATE.md` follows the same pattern for the human-facing guide:

- core workflow sections describe the default diffship loop
- "Customize this section" blocks hold repo-specific commands, ownership boundaries, and operating rules

That keeps `.diffship/PROJECT_KIT.md` useful as a local onboarding document instead of a copy of generic product docs.

The generated `.diffship/config.toml` now follows the same idea:

- core defaults stay close to the repository's actual diffship workflow
- "Customize this section" comments show where to set repo-specific defaults such as verify profile, handoff profile, output directory, promotion mode, and post-apply commands
- the generated handoff `output_dir` defaults to `./.diffship/artifacts/handoffs` so diffship-owned outputs stay under `.diffship/` after `diffship init`

### 6.2 Full loop

```bash
diffship loop ./patch-bundle.zip
```

What happens:

1. acquire the repo lock
2. create or reuse a session
3. create a sandbox worktree for the run
4. apply the patch bundle
5. run configured ops.post_apply commands, if any
6. run verification
7. promote if verification succeeds
8. persist run logs under `.diffship/runs/<run-id>/`

### 6.3 Use individual ops commands

Apply only:

```bash
diffship apply ./patch-bundle.zip
diffship apply ./patch-bundle.zip --base-commit "$(git rev-parse HEAD)"
```

Verify a specific run:

```bash
diffship verify --run-id <run-id> --profile standard
```

Promote a specific run:

```bash
diffship promote --run-id <run-id>
```

List runs:

```bash
diffship runs
diffship runs --heads-only
diffship runs --json
```

Show overall status:

```bash
diffship status
diffship status --heads-only
diffship status --json
```

Repair a stale session after manual commits:

```bash
diffship session repair --session default
diffship doctor --session default
diffship doctor --session default --fix
```

### 6.4 Promotion modes

Available promotion modes:

- `commit`
- `working-tree`
- `none`

Examples:

```bash
diffship loop ./patch-bundle.zip --promotion none
diffship loop ./patch-bundle.zip --promotion working-tree
diffship loop ./patch-bundle.zip --promotion commit --commit-policy manual
```

### 6.5 Verification profiles

Built-in names:

- `fast`
- `standard`
- `full`

You can also define custom local profiles in config and run them by name.

### 6.6 When verify fails

diffship writes a reprompt bundle under the run directory:

- `.diffship/runs/<run-id>/pack-fix_YYYY-MM-DD_HHMMSS_<head7>[_N].zip`

You can recreate it manually:

```bash
diffship pack-fix --run-id <run-id>
```

### 6.7 Acknowledgement gates

Promotion may require explicit acknowledgement:

- `--ack-secrets`
- `--ack-tasks`

Examples:

```bash
diffship loop ./patch-bundle.zip --ack-secrets
diffship promote --run-id <run-id> --ack-tasks
```

See `docs/OPS_WORKFLOW.md` for the ops-focused walkthrough.

---

## 7. TUI usage

Start the TUI:

```bash
diffship
```

or:

```bash
diffship tui
```

Current screens:

- Runs
- Status
- Loop
- Handoff

Current handoff screen capabilities:

- range selection
- source toggles
- include / exclude filters
- split mode selection
- named profile cycling
- packing limit overrides
- plan path editing
- preview
- build
- equivalent CLI command display
- plan export

The TUI and CLI are intended to stay equivalent. If a handoff option matters, there should be a CLI representation for it.

---

## 8. Configuration

Resolution order:

1. built-in defaults
2. global config
3. project config
4. patch bundle manifest, when applicable
5. CLI flags

In practical terms:

- ops settings may be influenced by patch bundle `manifest.yaml`
- handoff packing profiles are resolved from config and CLI
- CLI flags always win

Current config files:

- global: HOME config under the standard diffship config path
- project: `.diffship.toml`
- project: `.diffship/config.toml`

Use config for:

- default verify profile
- custom verify profile commands
- default promotion mode / target branch / commit policy
- named handoff packing profiles

See `docs/CONFIG.md` for concrete TOML examples.

---

## 9. CI and automation patterns

Bundle preview for CI:

```bash
diffship preview ./bundle --list --json
```

Bundle comparison for CI:

```bash
diffship compare ./bundle_a ./bundle_b --json
```

Repository validation before finishing local work:

```bash
just docs-check
just trace-check
just ci
```

---

## 10. Common files and directories

Important repository paths:

- `docs/SPEC_V1.md`
- `docs/BUNDLE_FORMAT.md`
- `docs/PATCH_BUNDLE_FORMAT.md`
- `docs/CONFIG.md`
- `docs/OPS_WORKFLOW.md`
- `.diffship/config.toml`
- `.diffship/PROJECT_KIT.md`
- `.diffship/AI_GUIDE.md`
- `.diffship/runs/<run-id>/`

---

## 11. Current scope

The current v1 core includes:

- end-to-end handoff bundle generation
- preview / compare
- plan export / replay
- named handoff packing profiles
- TUI handoff flow
- end-to-end ops loop with safety defaults

Still treated as future-extension territory:

- extra compare/TUI polish
- raw zip container byte equality as a separate compare contract
- dedicated profile import/export commands

For exact status tracking, see `docs/IMPLEMENTATION_STATUS.md` and `PLAN.md`.
