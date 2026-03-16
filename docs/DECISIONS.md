# Decisions (diffship OS)

This file records the key decisions for diffship OS.
It keeps concise conclusions so the rationale survives chat switches and work can resume without re-deriving prior choices.

---

## D-001: Optimize for OS mode (worktree / session / sandbox)

- Date: 2026-03-01
- Decision:
  - Run ops in isolated worktrees (session + sandbox) so the user's main working tree stays clean.
- Rationale:
  - The core operating model is repeated apply/verify loops without polluting the main checkout.
- Implications:
  - Worktree management, cleanup, and locking are mandatory.

---

## D-002: Official defaults (V1)

- Date: 2026-03-01
- Defaults:
  - Promotion: `commit`
  - Commit policy: `auto`
  - Verify profile: `standard`
  - Safety: require a clean tree, require base commit match, enable path guards, enable locking

---

## D-003: Separate patch format from commit behavior

- Date: 2026-03-01
- Decision:
  - Treat `apply_mode` (patch transport) and `commit_policy` (commit behavior) as separate axes.
- Notes:
  - With `commit_policy=auto`, `apply_mode=git-apply` can still commit via `git commit -F`.

---

## D-004: Do not proceed automatically on secrets or required user work

- Date: 2026-03-01
- Decision:
  - Stop promotion when secrets-like content is detected unless the user explicitly acknowledges it.
  - Package required user work in `tasks/USER_TASKS.md`.

---

## D-005: Worktree layout and recovery strategy (`status` must recover state)

- Date: 2026-03-01
- Decision:
  - Store worktrees under `.diffship/worktrees/`.
  - Session worktrees live under `.diffship/worktrees/sessions/<session>/`.
  - Sandbox worktrees live under `.diffship/worktrees/sandboxes/<run-id>/`.
  - Session state is tracked via both `refs/diffship/sessions/<session>` and `.diffship/sessions/<session>.json`.
  - Store the run/sandbox linkage in `.diffship/runs/<run-id>/sandbox.json`.
- Recovery:
  - `diffship status` must show sessions and sandboxes so users can see which run a leftover sandbox belongs to.
  - `diffship session repair` is the supported way to reseed a session from the current repo HEAD when manual commits leave the session ref stale.
  - `diffship doctor` should diagnose stale session/worktree state first and only auto-apply clearly safe fixes.
  - Recovery may use `git worktree remove --force <path>` when needed.
  - Sandbox cleanup is best-effort; leftovers are surfaced via `status`.

---

## D-006: Default verify profile behavior (M2)

- Date: 2026-03-01
- Decision:
  - `verify` only runs locally defined commands, never bundle-provided commands.
  - Before full config loading landed, the fallback order was:
    1. `justfile` + available `just` -> run `just ...` according to profile
    2. `Cargo.toml` present -> run `cargo ...` according to profile
    3. otherwise -> run `git diff --check`
- Rationale:
  - diffship itself uses `just` as its quality gate.
  - Generic repositories still need a sane verify fallback.

---

## D-007: `promotion=commit` implementation strategy (M2-04)

- Date: 2026-03-01
- Decision:
  - Reflect sandbox results by cherry-picking into the target branch.
  - For `apply_mode=git-apply`, create one sandbox commit via `git commit -F` before cherry-pick.
  - Refuse promotion unless the target branch HEAD matches the sandbox `base_commit`.
- Defaults:
  - Prefer `develop` as the target branch, then fall back to the current branch.
- Artifacts:
  - Write `.diffship/runs/<run-id>/promotion.json`.

---

## D-008: `loop` implementation strategy (M2-05)

- Date: 2026-03-01
- Decision:
  - Hold a single lock across `apply -> verify -> promote`.
  - Reuse the `apply` run-id as the loop run-id and append `verify.json` / `promotion.json` to the same run directory.
  - On verify failure, create `pack-fix` (reprompt zip) and stop.

---

## D-009: Keep reserved exit codes with `dead_code` allowed

- Date: 2026-03-02
- Decision:
  - When an exit code is reserved ahead of implementation, keep it with `#[allow(dead_code)]` until it is used.
- Notes:
  - Removing reserved codes makes the spec/implementation mapping drift more easily.

---

## D-010: Tasks block promotion and require `--ack-tasks`

- Date: 2026-03-03
- Decision:
  - If a bundle contains `tasks/USER_TASKS.md`, promotion stops by default and requires explicit `--ack-tasks`.
- Rationale:
  - Skipping manual work such as env setup, key rotation, or one-off migration steps can break the workflow.
- Implications:
  - `diffship apply` surfaces the tasks path and keeps it in the run directory.
  - `diffship promote` / `diffship loop` refuse with exit 12 until tasks are acknowledged.

---

## D-011: Config precedence

- Date: 2026-03-03
- Decision:
  - Resolve config in this order (last writer wins):
    1. CLI flags
    2. bundle manifest
    3. project config (`.diffship/config.toml`)
    4. global config (`~/.config/diffship/config.toml`)
    5. built-in defaults
- Notes:
  - Unspecified values should not overwrite lower layers; `None` delegates downward.
  - This preserves stable defaults while allowing safe per-run overrides.

---

## D-012: M4-02 uses CLI flags for promotion / commit-policy switching

- Date: 2026-03-03
- Decision:
  - Support `--promotion` (`none|working-tree|commit`) and `--commit-policy` (`auto|manual`) as CLI overrides.
  - CLI stays at the highest precedence per D-011.
- Rationale:
  - Users need per-run safety overrides without rewriting bundle/project/global defaults.
- Notes:
  - `promotion=none` skips promotion but still writes `promotion.json` and keeps the sandbox by default.
  - With `commit-policy=manual`, git-apply promotion requires a pre-existing commit in the sandbox.

---

## D-013: TUI v0 focuses on visibility + execution support and preserves CLI parity

- Date: 2026-03-04
- Decision:
  - Scope the TUI to status/runs visibility plus loop execution support.
  - Keep it as a thin wrapper over existing ops/CLI code; do not create TUI-only behavior.
  - Add `diffship tui`, and let `diffship` with no args start the TUI only in TTY contexts.
- Rationale:
  - Users need visibility into active runs without breaking automation.
- Implications:
  - Start with the minimal screens: Runs, Status, Run detail/log, and Loop.

---

## D-014: Use CRLF in raw mode to avoid terminal-dependent rendering glitches

- Date: 2026-03-04
- Decision:
  - Emit `\r\n` for TUI single-line output in raw mode to avoid line-return issues on some terminals.
- Rationale:
  - Plain `\n` can leave the cursor mid-line on certain terminals.
- Implications:
  - `writeln_trunc` standardizes on CRLF output.

---

## D-015: Start the TUI automatically only on TTY, with an env var escape hatch

- Date: 2026-03-04
- Decision:
  - `diffship` with no args starts the TUI only when attached to a TTY.
  - `diffship tui` remains the explicit subcommand.
  - `DIFFSHIP_NO_TUI=1` disables auto-TUI.
- Rationale:
  - This preserves CI / pipeline / script behavior while giving interactive users the TUI by default.
- Implications:
  - The TUI must not introduce behavior unavailable from the CLI.

---

## D-016: Lock TUI/CLI parity with non-TTY smoke tests

- Date: 2026-03-04
- Decision:
  - In tests under non-TTY conditions:
    - `diffship` with no args must print help and exit quickly.
    - `diffship tui` must fail with a “requires a TTY” message.
  - Always use timeouts to avoid hangs.
- Notes:
  - Use `assert_cmd::cargo::cargo_bin_cmd!` instead of the deprecated `Command::cargo_bin` path.

---

## D-017: Keep test imports minimal under `-D warnings`

- Date: 2026-03-04
- Decision:
  - Avoid wildcard imports such as `assert_cmd::prelude::*` when only a subset is needed.
- Rationale:
  - Tests must pass the same `-D warnings` quality gate as the implementation.

---

## D-018: Start handoff implementation from `diffship build`, with committed-only as the first MVP

- Date: 2026-03-05
- Decision:
  - Use `diffship build` as the handoff generation command and match `docs/BUNDLE_FORMAT.md` / `docs/HANDOFF_TEMPLATE.md`.
  - Prioritize committed range bundling first; staged/unstaged/untracked follow incrementally.
- Rationale:
  - Starting from the final command/contract reduces backtracking.
  - Range selection + determinism + split rules are the right foundation for later extension.
- Implications:
  - The existing ops TUI remains in place; handoff-specific TUI/preview work comes later.

---

## D-019: Begin `diffship build` with committed-only + one-part output

- Date: 2026-03-05
- Decision:
  - The first `diffship build` implementation supports committed range only.
  - Establish the layout with fixed `parts/part_01.patch` plus `HANDOFF.md`.
  - Support `--range-mode direct|merge-base|last|root`, defaulting to `last`.
  - Default output is `./diffship_YYYY-MM-DD_HHMM_<head7>/` and originally failed on name collisions.
  - `--zip` produces the same layout as a zip.
- Rationale:
  - Fixing the entry document and patch location first makes later split/attachment/preview work safer.

---

## D-020: Group long render-function arguments into context structs

- Date: 2026-03-05
- Decision:
  - Use context structs such as `*Inputs` for functions that would otherwise exceed clippy argument-count limits.
  - In tests, prefer `str::contains` over importing predicate traits when the standard library is enough.
- Rationale:
  - This preserves the quality gate under `-D warnings` without adding many local `#[allow(...)]`s.
- Implications:
  - Structured argument passing is the default; local `allow` remains a last resort.

---

## D-021: Tests must not assume a default branch name

- Date: 2026-03-05
- Decision:
  - Integration tests using temporary repositories must not assume `master` or any other fixed default branch name.
  - Detect the current branch with `git rev-parse --abbrev-ref HEAD` when needed.
- Rationale:
  - Git default branch names vary by environment.
- Implications:
  - `just ci` remains stable across local and CI environments.

---

## D-022: Introduce handoff uncommitted sources as explicit segment toggles

- Date: 2026-03-06
- Decision:
  - `diffship build` keeps committed changes on by default and adds uncommitted sources via `--include-staged`, `--include-unstaged`, and `--include-untracked`.
  - `--no-committed` disables the committed segment.
  - The initial staged rollout handled textual untracked files first; binary/unreadable behavior was added later.
- Rationale:
  - Segment toggles make the source of each diff explicit and support later attachments/excluded/splitting work safely.
- Implications:
  - `HANDOFF.md` must record which segments were included and what base they used.

---

## D-023: Only use `Partial` in traceability when `TBD` remains

- Date: 2026-03-06
- Decision:
  - In `docs/TRACEABILITY.md`, `Status: Partial` is only valid when either the Tests or Code side still contains `TBD`.
  - If both sides point to concrete paths, the status is `Implemented`.
- Rationale:
  - This matches `scripts/check-traceability.sh` and keeps `just trace-check` stable.

---

## D-024: Split / untracked policy for M6-03

- Date: 2026-03-06
- Decision:
  - Add `--split-by auto|file|commit`, with `commit` applying only to committed ranges.
  - `auto` selects `commit` when the committed range spans multiple commits, otherwise `file`.
  - Untracked handling uses `--untracked-mode auto|patch|raw|meta`, where `auto` means text/small -> patch and binary/unreadable/large -> `attachments.zip`.
  - `meta` records the omission plus rerun guidance in `excluded.md`.
- Rationale:
  - This keeps commit views AI-readable without breaking on huge or non-UTF-8 files.
- Implications:
  - `HANDOFF.md` conditionally emits Commit View / Attachments / Exclusions.
  - Staged / unstaged / untracked remain file-level units.

---

## D-025: Do not write generated output names in README as repo path references

- Date: 2026-03-06
- Decision:
  - In `README.md`, generated outputs such as `HANDOFF.md`, `parts/`, `attachments.zip`, and `excluded.md` should not be written as inline-code path references that doc-check interprets as repo files.
  - Keep `zip::write::FileOptions` typing aligned with the repository's current dependency version.
- Rationale:
  - `scripts/check-doc-links.sh` validates inline-code paths as if they were real repository paths.
- Implications:
  - Distinguish repo files from runtime-generated outputs in docs.

---

## D-026: `HANDOFF.md` is always the bundle entrypoint

- Date: 2026-03-06
- Decision:
  - Treat `HANDOFF.md` as the bundle entry document.
  - Always include at least `Start Here`, `TL;DR`, `Change Map`, and `Parts Index`.
  - Keep `Parts Index` in two layers: quick index and part details.
- Rationale:
  - The AI or human reader must know what to read first without guesswork.
- Implications:
  - Tests must protect both section structure and the first-patch reading path.

---

## D-027: Ignore and secrets warning policy for M6-05

- Date: 2026-03-06
- Decision:
  - Build reads `.diffshipignore` directly and applies the same exclusion rules to all segments.
  - When secrets-like content is detected, record path + reason only in `secrets.md` and `HANDOFF.md`.
  - In non-TTY mode, secrets stop the build with exit 4 unless `--yes` is given; CI should use `--fail-on-secrets`.
- Rationale:
  - Handoff bundles are meant to be shared with external AI, so sharing risk must be surfaced at build time.
- Implications:
  - `diffship build` includes `--yes` / `--fail-on-secrets`.
  - `HANDOFF.md` exposes secrets-warning and ignore-active state at the entrypoint.

---

## D-028: Reserved handoff exit codes keep `#[allow(dead_code)]`

- Date: 2026-03-06
- Decision:
  - Apply the same reserved-exit-code pattern to handoff codes.
  - Keep `EXIT_PACKING_LIMITS=3` reserved with `#[allow(dead_code)]` until used.
- Rationale:
  - This keeps `clippy -D warnings` clean while preserving spec/code alignment.

---

## D-029: Fix handoff output ordering and zip metadata for determinism

- Date: 2026-03-06
- Decision:
  - Fix `HANDOFF.md` listing order to docs -> config -> source -> tests -> other, then path order, then segment order committed -> staged -> unstaged -> untracked.
  - Sort generated zip entries and use fixed zip metadata behavior.
  - Protect determinism with `tests/m6_handoff_determinism.rs` and `tests/golden/` fixtures.
- Rationale:
  - Deterministic trees and zip bytes keep golden tests and bundle comparison stable.
- Implications:
  - Ordering or metadata rule changes require simultaneous updates to `docs/DETERMINISM.md` and fixtures.

---

## D-030: Golden normalization must preserve UTF-8

- Date: 2026-03-06
- Decision:
  - Golden normalization must not break UTF-8 symbols such as `→` while replacing 40-char hex strings.
- Rationale:
  - Byte-wise reconstruction can corrupt non-ASCII text and cause false-positive golden failures.
- Implications:
  - Placeholder replacement operates on character boundaries.

---

## D-031: Plan correction after the 2026-03-06 inventory

- Date: 2026-03-06
- Decision:
  - Move M4-02 (`promotion` / `commit-policy` switching) back to `doing` at that time because `working-tree` still behaved like `commit`.
  - Move M6-03 (profiles + packing limits) back to `doing` because size limits and `EXIT_PACKING_LIMITS` were not yet active.
  - Treat `pack-fix` as implemented but keep dedicated integration coverage as an explicit follow-up.
- Rationale:
  - “done” needed to match the actual implementation and avoid overstating status in README / PLAN / TRACEABILITY.
- Implications:
  - The next priorities became end-to-end operational docs, packing limits / binary policy, promotion `working-tree` separation, and preview flow.

---

## D-032: `promotion=working-tree` updates the target working tree without committing

- Date: 2026-03-06
- Decision:
  - `promotion=working-tree` applies sandbox results to the target branch working tree without creating a commit.
  - `promotion=commit` keeps creating a commit.
  - `promotion=none` performs no promotion.
- Rationale:
  - The three modes need distinct semantics that match the CLI contract.
- Implications:
  - `working-tree` leaves target HEAD unchanged while modifying the working tree.
  - Base-commit matching remains required.

---

## D-033: Enforce packing limits in handoff build

- Date: 2026-03-06
- Decision:
  - Add `--max-parts` / `--max-bytes-per-part` and stop with exit 3 (`EXIT_PACKING_LIMITS`) when generated parts exceed the configured limits.
  - Default limits are `max_parts=20` and `max_bytes_per_part=536870912` (512 MiB).
- Rationale:
  - Upload limit violations should be detected mechanically during build.
- Implications:
  - `EXIT_PACKING_LIMITS` moves from reserved to active use.
  - This stage preferred explicit failure over automatic repartitioning.

---

## D-034: Binary policy is default-exclude with explicit opt-in

- Date: 2026-03-06
- Decision:
  - Exclude binary content by default.
  - When `--include-binary` is used, support `--binary-mode raw|patch|meta` with default `raw`.
  - Unify `auto` untracked behavior as text/small -> patch, large text -> raw, binary -> binary policy.
- Rationale:
  - Default sharing should minimize information exposure while still allowing explicit inclusion.
  - This resolves the previous overlap between `S-UNTRACKED-003` and `S-BINARY-001`.
- Implications:
  - Update `docs/SPEC_V1.md`, `docs/BUNDLE_FORMAT.md`, and `docs/TRACEABILITY.md` around this policy.
  - Show binary policy in `HANDOFF.md`.

---

## D-035: Add `preview` / `compare` as handoff inspection commands

- Date: 2026-03-06
- Decision:
  - Add `diffship preview <bundle>` for reading `HANDOFF.md` and parts from directory or zip bundles.
  - Add `diffship compare <a> <b>` for normalized comparison by default plus strict byte-oriented comparison via `--strict`.
- Rationale:
  - Users need both pre-share inspection and reproducibility checks from the CLI alone.
- Implications:
  - README and ops workflow docs should show the handoff -> AI -> ops flow.

---

## D-036: Extend verify profiles via local config commands

- Date: 2026-03-06
- Decision:
  - Support custom verify profiles under `[verify.profiles.<name>]` in addition to `fast|standard|full`.
  - Run custom profile commands via `sh -lc` inside the sandbox.
- Rationale:
  - Repositories need profile names that map to local quality gates.
- Implications:
  - `docs/CONFIG.md` must document the implemented custom profile behavior.

---

## D-037: Packing overflow falls back to First-Fit Decreasing plus exclusion

- Date: 2026-03-06
- Decision:
  - On packing overflow, sort diff units by descending size and repack with FFD.
  - Move units that still do not fit to `excluded.md` with reason/guidance.
  - Only fail with `EXIT_PACKING_LIMITS` when everything gets excluded.
- Rationale:
  - Producing a still-readable bundle is better than failing immediately when partial output is possible.
- Implications:
  - Concentrate the implementation/tests in `src/handoff.rs` and `tests/m6_handoff_build.rs`.

---

## D-038: Add a handoff screen to the TUI and always show the equivalent CLI

- Date: 2026-03-07
- Decision:
  - Add a TUI handoff screen that can switch range/sources/split/binary/output and run preview/build.
  - Reuse `src/plan.rs` to reconstruct CLI arguments rather than creating TUI-only logic.
  - Implement internal preview by generating a temporary bundle and showing the first patch part.
- Rationale:
  - Users should be able to follow the handoff flow in the TUI without breaking CLI parity.
- Implications:
  - Plan export/replay remained a follow-up task at that stage; equivalent CLI display shipped first.

---

## D-039: Explicit path filters combine with `.diffshipignore` and apply to all segments

- Date: 2026-03-07
- Decision:
  - Add repeatable `--include <glob>` / `--exclude <glob>` to `diffship build`.
  - Apply `.diffshipignore` and `--exclude` first; if `--include` is empty allow the path, otherwise require a match.
  - Apply the same filter rules to committed/staged/unstaged/untracked and record them in `HANDOFF.md`.
- Rationale:
  - Bundle contents must remain explainable and consistent across source categories.
- Implications:
  - The TUI handoff screen should also edit include/exclude patterns.

---

## D-040: Try context reduction before exclusion on packing overflow

- Date: 2026-03-07
- Decision:
  - When a unit does not fit, first reduce unified diff context to `U1` and then `U0` before excluding it.
  - Only send units to `excluded.md` if they still do not fit after reduction.
  - Mark reduced-context file rows in `HANDOFF.md`.
- Rationale:
  - Keeping the changed lines is better than excluding the file outright.
- Implications:
  - The packing fallback contract in `docs/SPEC_V1.md` moves from future work to current behavior.

---

## D-041: `preview --json` / `compare --json` always write to stdout, and compare keeps non-zero exit on diff

- Date: 2026-03-07
- Decision:
  - `diffship preview --json` prints pretty JSON to stdout.
  - `diffship compare --json` prints a compare report to stdout and still returns non-zero on differences.
- Rationale:
  - CI should parse stdout directly while still using exit codes for pass/fail.
- Implications:
  - README and ops workflow docs should show CI-oriented `--json` usage.

---

## D-042: `plan.toml` stores handoff selection, while runtime/output flags are replay-time overrides

- Date: 2026-03-07
- Decision:
  - `plan.toml` stores range/sources/filters/split/binary/packing selection.
  - `out`, `zip`, `yes`, and `fail-on-secrets` are not fixed inside the plan and are applied at replay time.
  - The TUI replay command should include runtime flag examples.
- Rationale:
  - Baking output paths into the plan makes replay unnecessarily rigid.
- Implications:
  - `docs/BUNDLE_FORMAT.md` must state that output paths are supplied at replay time.

---

## D-043: Resolve handoff packing profiles through named presets plus config defaults

- Date: 2026-03-07
- Decision:
  - Add `--profile <name>` with built-ins `20x512` (default) and `10x100`.
  - Load `[handoff].default_profile` and `[handoff.profiles.<name>]` from global/project config, with compatibility support for `[profiles.<name>]`.
  - Reuse the same profile set in the TUI and allow cycling with `h`.
  - Store both the selected `profile` name and resolved numeric limits in `plan.toml`.
- Rationale:
  - Named profiles make upload constraints reusable across repos and preserve CLI/TUI/replay parity.
- Implications:
  - `HANDOFF.md` should show the actual profile name rather than an internal label.
  - The init config stub should include an example handoff profile definition.

---

## D-044: `compare` reports differences by area and kind

- Date: 2026-03-07
- Decision:
  - Classify compare diffs by `area` (`handoff|patch|attachments|excluded|secrets|plan|other`) and `kind` (`only_in_a|only_in_b|content_differs`).
  - Human-readable output uses `[area/kind] path` plus counts.
  - JSON output also includes `areas` / `kinds` aggregates and per-diff classification.
- Rationale:
  - This makes it easier to tell whether a difference is in patch content or surrounding metadata.
- Implications:
  - Exit-code behavior does not change: differences remain non-zero.

---

## D-045: TUI handoff input uses a live buffer plus field navigation

- Date: 2026-03-07
- Decision:
  - Add edit buffer/help display to the handoff screen.
  - Make `plan path`, `max parts`, and `max bytes per part` editable from the TUI.
  - Use `Tab` / `Shift+Tab` to move between editable handoff fields.
- Rationale:
  - The previous hotkey-only model made the current target/value too hard to see and did not cover key CLI parity knobs.
- Implications:
  - The TUI handoff screen reaches near-v1-core parity; finer UX polish remains future work.

---

## D-046: Additional compare/TUI polish is v1.1 backlog, not a v1 blocker

- Date: 2026-03-07
- Decision:
  - Treat additional display polish in `compare` and small TUI input improvements as v1.1 backlog items.
  - Define v1 completion as CLI/TUI parity, plan export/replay, preview/compare, and deterministic handoff build.
- Rationale:
  - The current handoff/ops flow already satisfies the documented v1 contract; remaining gaps are mostly usability polish.
- Implications:
  - README / IMPLEMENTATION_STATUS / PLAN use “handoff v1 core is implemented; remaining work is future extension.”

---

## D-047: `compare --strict` compares extracted entry bytes, not raw zip container bytes

- Date: 2026-03-07
- Decision:
  - `diffship compare --strict` compares raw bundle entry bytes without normalization, not the zip container as a single byte blob.
  - Differences limited to zip entry ordering, archive metadata, or container layout do not count as strict differences.
  - If raw zip-container byte equality is ever needed, treat it as a separate future contract.
- Rationale:
  - The real goal is bundle-content reproducibility, not sensitivity to container implementation noise.
- Implications:
  - Align `docs/SPEC_V1.md`, `README.md`, and `docs/IMPLEMENTATION_STATUS.md` with this contract.
  - Keep coverage in `tests/m6_compare.rs` for equivalent-content / different-container cases.

---

## D-048: Named handoff profiles are owned by config; `plan.toml` exports only the selected result

- Date: 2026-03-07
- Decision:
  - Treat project/global config (`[handoff.profiles.*]` and compatibility `[profiles.*]`) as the source of truth for named handoff profiles.
  - `plan.toml` exports the selected profile name plus resolved numeric limits, not the full profile catalog.
  - Clarify this via the generated config stub, README, BUNDLE_FORMAT docs, and TUI/export messaging.
- Rationale:
  - Duplicating the profile catalog into `plan.toml` makes the source of truth ambiguous.
- Implications:
  - Do not add a dedicated import/export command yet.
  - The init config stub and related docs should explain how to share profile definitions.

---

## D-049: Default handoff output names use local time and auto-number collisions

- Date: 2026-03-07
- Decision:
  - When `--out` is omitted, use `diffship_YYYY-MM-DD_HHMM_<head7>` based on the local system timezone.
  - If the path already exists, choose `_2`, `_3`, ... automatically.
  - Explicit `--out` keeps the previous “existing path is an error” behavior.
- Rationale:
  - Users expect bundle names to match their local time and do not want same-minute collisions to fail.
- Implications:
  - Clarify `S-OUT-001` accordingly and fix tests around naming behavior.

---

## D-050: `--out-dir` changes only the parent directory of the auto-generated handoff bundle name

- Date: 2026-03-07
- Decision:
  - Add `diffship build --out-dir <dir>` so users can change the parent directory while keeping the auto-generated bundle name.
  - Keep `--out <path>` as an exact output path.
  - Reject using both `--out` and `--out-dir` together.
- Rationale:
  - Users often want to keep the generated name but place it elsewhere.
- Implications:
  - `HandoffPlan` replay and generated shell commands must include `--out-dir`.
  - Docs must explain the role split between `--out` and `--out-dir`.

---

## D-051: `[handoff].output_dir` is the config default for auto-generated handoff output parents

- Date: 2026-03-07
- Decision:
  - Accept `[handoff].output_dir` in project/global config as the default parent directory when neither `--out` nor `--out-dir` is given.
  - The precedence is `--out` > `--out-dir` > `[handoff].output_dir` > current working directory.
  - Also accept `[handoff].out_dir` as a compatibility alias.
- Rationale:
  - Users often want a stable default handoff output parent without specifying it on every build.
- Implications:
  - `src/handoff_config.rs` resolves `[handoff].output_dir` into `BuildArgs.out_dir`.
  - Path resolution is centralized and docs/tests are updated accordingly.

---

## D-052: `diffship init` generates separate human and AI guides

- Date: 2026-03-07
- Decision:
  - Generate `.diffship/PROJECT_KIT.md` and `.diffship/AI_GUIDE.md` separately.
  - `PROJECT_KIT.md` is the human workflow guide; `AI_GUIDE.md` captures the workflow, artifact contracts, file semantics, and non-file deliverables the AI must respect.
- Rationale:
  - Combining human guidance and AI output contracts in one file adds noise for both audiences.
- Implications:
  - Use `docs/AI_PROJECT_TEMPLATE.md` as the template source for `.diffship/AI_GUIDE.md`.
  - Keep init integration coverage and docs in sync.

---

## D-053: post-apply commands are local-config-only sandbox hooks

- Date: 2026-03-07
- Decision:
  - Run commands listed under `[ops.post_apply]` immediately after a successful patch apply inside the sandbox.
  - Only resolve them from local config, never from the patch bundle manifest.
  - If any command fails, mark `apply` / `loop` as failed and keep logs in the run directory.
- Rationale:
  - Users want automatic formatter / docs/spec consistency commands after apply.
  - Bundle-provided arbitrary commands would violate the safety model.
- Implications:
  - `src/ops/config.rs` resolves `[ops.post_apply]`.
  - `src/ops/post_apply.rs` owns sandbox execution and `post_apply.json` / log output.
  - Hook failures are not treated as success.

---

## D-054: Apply `~/...` shorthand across the CLI with a shared path resolver

- Date: 2026-03-07
- Decision:
  - Use the same `~/...` -> `HOME` rule across CLI commands that accept filesystem paths, not only handoff build.
  - Continue to reject tilde-user shorthand.
- Rationale:
  - Users expect shorthand path behavior to stay consistent across commands.
  - A shared helper is simpler to test and document.
- Implications:
  - `src/pathing.rs` is the shared helper used by handoff / preview / compare / apply / pack-fix.
  - Spec/docs describe this as a general CLI rule rather than a handoff-only rule.

---

## D-055: `diffship init --template-dir` can override generated guide templates

- Date: 2026-03-07
- Decision:
  - `diffship init` accepts `--template-dir <dir>`.
  - When provided, diffship looks for `PROJECT_KIT_TEMPLATE.md` and `AI_PROJECT_TEMPLATE.md` in that directory before falling back to repository templates or built-in defaults.
- Rationale:
  - Repositories may want project-specific onboarding guidance without editing the committed default templates.
  - A directory-level override is simpler than separate override flags per generated guide.
- Implications:
  - `src/cli.rs` exposes the option and `src/ops/init.rs` resolves the template directory with the shared path rules.
  - Init integration tests and user-facing docs must show the override behavior.


## D-056: When loop-ready output is requested, missing `base_commit` should not silently produce a non-ops zip

- Date: 2026-03-07
- Decision:
  - Reserve `patchship_...` for bundles that are already valid ops inputs.
  - Reserve `DO_NOT_LOOP_nonops_...` for non-ops archives.
  - If the user explicitly wants something they can pass to `diffship loop` and the exact `base_commit` is missing, generated guidance should prefer `MODE: ANALYSIS_ONLY` (or an explicit request for the SHA) over an automatic non-ops fallback zip.
- Rationale:
  - A non-ops archive can be perfectly valid as documentation, but it is easy for humans to mistake it for a loop-ready bundle.
  - The most common hard precondition for ops bundles is the exact target `git rev-parse HEAD` value.
- Implications:
  - `diffship init` templates must clearly separate return modes.
  - Human-facing guidance should teach a quick classifier (`manifest.yaml` + `changes/` vs `README_NOT_OPS.md`).
  - AI-facing guidance should include a self-check and a minimal valid manifest example.


---

## D-057: `diffship init` templates should show explicit artifact trees for each AI return mode

- Date: 2026-03-07
- Decision:
  - Show concrete tree examples for `OPS_PATCH_BUNDLE`, `NONOPS_EDIT_PACKAGE`, and `ANALYSIS_ONLY` in the generated human/AI guides.
  - Reserve loop-ready-looking trees for real patch bundles only.
  - Teach humans to classify returned archives from their marker files and top-level tree before running `diffship loop`.
- Rationale:
  - Many failures come from structurally ambiguous zips rather than bad intent.
  - Tree-level examples are faster to verify than prose-only contracts.
- Implications:
  - `docs/PROJECT_KIT_TEMPLATE.md` and `docs/AI_PROJECT_TEMPLATE.md` should include artifact-tree cheat sheets.
  - `docs/PATCH_BUNDLE_FORMAT.md` should document both the minimal accepted patch bundle tree and a common rejected non-ops lookalike.


## D-058: Standardize AI-generated `git-am` author identity as Diffship

- Date: 2026-03-08
- Decision:
  - For AI-generated mail-style patches that use `apply_mode=git-am`, standardize the default `From:` identity as `Diffship <diffship@example.com>`.
  - Do not use provider-specific identities such as `OpenAI <assistant@example.com>` as the default contract.
  - If a repository wants the final promoted commit author to be the local human operator by default, prefer `apply_mode=git-apply` or an explicit post-promotion author-reset flow.
- Rationale:
  - `git am` preserves the patch `From:` line as the commit author, so a stable tool identity is clearer than leaking a provider or model name into repository history.
  - Human authorship and tool authorship are separate concerns and should not be conflated in the patch contract.
- Implications:
  - `diffship init` templates should document the default `git-am` author identity and the escape hatch for repositories that prefer local human authorship.

---

## D-059: `diffship init` should generate a dedicated paste-ready rules file with a small language surface

- Date: 2026-03-16
- Decision:
  - Generate `.diffship/PROJECT_RULES.md` as a short, copy/paste-oriented rules snippet for external AI project-rule UIs.
  - Keep the existing `.diffship/PROJECT_KIT.md` and `.diffship/AI_GUIDE.md` as the longer human/AI guides.
  - Scope `diffship init --lang <en|ja>` to the generated `PROJECT_RULES.md` snippet and record the resolved language in the rules-kit metadata.
- Rationale:
  - Users need something shorter than the full guides when a UI only offers a compact project-rules field.
  - Localizing the concise snippet first solves the immediate English/Japanese need without duplicating the larger guide templates.
- Implications:
  - `diffship init --zip` now includes `PROJECT_RULES.md` in the exported rules kit.
  - Init integration tests, docs, and progress tracking must stay aligned with the new generated artifact.

---

## D-060: Dedicated `.diffship/forbid.toml` keeps fragile-path policy separate from the main config stub

- Date: 2026-03-16
- Decision:
  - Let project-local forbid patterns live in a dedicated `.diffship/forbid.toml` file in addition to `.diffship/config.toml`.
  - Have `diffship init` generate that file as a starter template and prefill detected lockfiles when applicable.
- Rationale:
  - Forbid policy is often edited by humans who do not want to touch the larger config stub.
  - Lockfiles and generated manifests are common fragile targets, so a dedicated file reduces setup friction.
- Implications:
  - `src/ops/config.rs` loads `.diffship/forbid.toml` as another project-local config source.
  - `src/ops/init.rs` writes the starter file and tests/docs must cover the new artifact.

---

## D-061: Surface run command-log coverage in human-facing status views

- Date: 2026-03-16
- Decision:
  - Keep `commands.json` as the machine-readable command-log index under each run.
  - Show command-count and phase summary in `diffship runs`, `diffship status`, and the TUI run detail when logs exist.
- Rationale:
  - The logs already existed on disk, but users still had to inspect the run directory manually to know whether `apply`, `post-apply`, `verify`, or `promote` emitted command logs.
- Implications:
  - `src/ops/run.rs` now derives command-count and phase summaries from `commands.json`.
  - Human-facing docs and integration tests should mention the new summaries.

---

## D-062: Treat missing or invalid `run.json` as orphaned run metadata during cleanup

- Date: 2026-03-16
- Decision:
  - `cleanup` should classify a run as orphaned when `run.json` is missing or invalid, even if the sandbox directory still exists.
- Rationale:
  - A leftover sandbox alone is not enough to preserve a broken run record; users expect `cleanup --include-runs` to remove run logs once the run metadata is no longer recoverable.
- Implications:
  - `src/ops/cleanup.rs` now checks `run.json` validity while collecting orphan sandbox/run candidates.
  - `tests/m7_cleanup.rs` covers both `cleanup` and `cleanup --include-runs` for this metadata-loss edge case.

---

## D-063: Structured handoff context Phase 1 starts with deterministic `handoff.manifest.json`

- Date: 2026-03-16
- Decision:
  - Keep patch parts canonical for executable changes.
  - Keep `HANDOFF.md` as the primary human/LLM entrypoint.
  - Add a root-level deterministic `handoff.manifest.json` as the canonical machine-readable structured-context layer for handoff bundles.
  - Do not use AI/API calls during build; generate the manifest only from deterministic repository facts in Rust.
- Rationale:
  - We want structured navigation and warning metadata without weakening the existing patch/apply contract.
  - JSON is easier to version, compare, and test deterministically than a rendered view.
- Implications:
  - `docs/BUNDLE_FORMAT.md`, `docs/SPEC_V1.md`, and `docs/DETERMINISM.md` must treat `handoff.manifest.json` as part of the bundle contract.
  - Later XML/Markdown rendered views or per-part context files remain optional extensions on top of this JSON layer.

---

## D-064: Per-part JSON context stays supplemental to the patch payload

- Date: 2026-03-16
- Decision:
  - Emit `parts/part_XX.context.json` next to each patch part as deterministic per-part context.
  - Keep these files strictly supplemental; they summarize scope/stats/constraints but do not redefine the patch.
  - Generate the summary text from repository facts and fixed templates only.
- Rationale:
  - The root manifest is useful for bundle navigation, but AI workflows also need a stable map at the patch-part level.
  - Per-part JSON gives that map without introducing an AI/API dependency or weakening the canonical patch contract.
- Implications:
  - `handoff.manifest.json` should link each part to its context file.
  - Compare / determinism tests must treat these files as first-class generated bundle artifacts.

---

## D-065: XML is a rendered view only, generated from the same deterministic facts as JSON

- Date: 2026-03-16
- Decision:
  - Emit `handoff.context.xml` as a deterministic rendered view of the structured handoff context.
  - Keep JSON canonical; XML is strictly a view layer and never replaces `handoff.manifest.json` or patch parts.
  - Generate XML from the same local deterministic facts and fixed templates used for JSON generation.
- Rationale:
  - Some AI workflows navigate hierarchy more easily in XML, but JSON remains easier to version, compare, and treat as the canonical machine-readable contract.
- Implications:
  - `handoff.manifest.json` should reference the XML rendered view.
  - Bundle docs, determinism docs, and compare tests must treat the XML file as a generated artifact without elevating it above JSON.

---

## D-066: Run summaries should expose direct artifact paths, not just log counts

- Date: 2026-03-16
- Decision:
  - Keep command-count and phase summaries, but also expose `run_dir`, `commands.json`, and phase-directory paths in `diffship runs` / `diffship status` and their JSON summaries when available.
- Rationale:
  - Counts answer whether logs exist, but not where to open them.
  - CLI users should get the same direct path affordance that the TUI run detail already provides.
- Implications:
  - `src/ops/run.rs` expands `RunSummary` with path fields.
  - Human-facing docs and integration tests should mention the direct path output.

---

## D-067: Forbid setup UX should support refreshing only the dedicated forbid file

- Date: 2026-03-16
- Decision:
  - Add `diffship init --refresh-forbid` so users can rewrite only `.diffship/forbid.toml` from current repo detections.
  - Do not require `--force` for unrelated generated files when using this option.
- Rationale:
  - Repositories often gain new lockfiles or generated manifests after the initial `init`.
  - Users should be able to refresh the dedicated forbid policy without rewriting the larger project-kit files.
- Implications:
  - `src/ops/init.rs` treats `forbid.toml` as independently refreshable.
  - Init docs and integration tests should describe the targeted refresh behavior.

---

## D-068: Structured context should expose aggregate JSON counts before adding more rendered views

- Date: 2026-03-16
- Decision:
  - Enrich the canonical JSON structured-context outputs with aggregate category / segment / status counts.
  - Keep these counts derived only from the already selected file rows; do not introduce heuristic or AI-generated metadata.
- Rationale:
  - Downstream tooling often needs scope summaries without reparsing `HANDOFF.md` or walking every diff hunk.
  - This follows the project rule that new structured-context investment should prefer richer machine-readable facts before more rendered layers.
- Implications:
  - `handoff.manifest.json` gains bundle-level aggregate counts.
  - `parts/part_XX.context.json` gains per-part segment/status counts alongside existing category and diff totals.

---

## D-069: Preview list output should consume canonical structured-context JSON directly

- Date: 2026-03-17
- Decision:
  - Teach `diffship preview --list` and `--list --json` to read `handoff.manifest.json` when present.
  - Surface structured-context artifact presence and aggregate category / segment / status counts from that canonical JSON instead of reparsing `HANDOFF.md`.
- Rationale:
  - We already emit the canonical machine-readable facts; preview should expose them directly so CI and humans can inspect bundle scope cheaply.
  - This creates a first consumer for the richer JSON facts without adding another rendered layer.
- Implications:
  - `src/preview.rs` parses the manifest summary opportunistically and keeps working for older bundles that lack the file.
  - Preview docs and tests must describe the extra list/json fields while preserving existing entry-text behavior.

---

## D-070: Compare should report canonical manifest-summary deltas, not just file-level artifact diffs

- Date: 2026-03-17
- Decision:
  - When both bundles provide `handoff.manifest.json`, `diffship compare` should parse the canonical manifest summary and report aggregate deltas.
  - Keep the existing file-level diff list; summary deltas are supplemental and never replace artifact diffs.
- Rationale:
  - File-by-file differences show which artifacts changed, but not whether the bundle scope widened or narrowed.
  - The canonical manifest already contains stable aggregate counts, so compare can surface scope changes cheaply and deterministically.
- Implications:
  - `src/bundle_compare.rs` opportunistically parses both manifests and adds structured-context summary deltas to human-readable and JSON output.
  - Compare tests and docs must cover the new summary-delta reporting while preserving existing non-zero exit behavior.

---

## D-071: TUI handoff preview should reuse canonical manifest summary instead of inventing a separate preview summary

- Date: 2026-03-17
- Decision:
  - When the temporary TUI preview bundle contains `handoff.manifest.json`, prepend the canonical structured-context summary to the preview pane before the first patch part.
  - Keep the patch preview itself unchanged after that header block.
- Rationale:
  - The TUI should expose the same machine-readable scope summary that CLI preview/compare now use.
  - Reusing the existing manifest avoids a second summary implementation just for the TUI.
- Implications:
  - `src/tui/mod.rs` reads the temporary preview bundle manifest opportunistically and stays compatible with older/malformed bundles by falling back to patch-only preview.
  - TUI docs and unit tests should cover the prepended summary lines.

---

## D-072: Reading-order guidance should live in canonical manifest JSON, not only in HANDOFF.md

- Date: 2026-03-17
- Decision:
  - Add a deterministic `reading_order` list to `handoff.manifest.json`.
  - Reuse the same derived guidance that `HANDOFF.md` already renders instead of inventing a second ordering heuristic.
- Rationale:
  - Downstream tools should be able to consume the recommended reading order without scraping markdown.
  - This keeps the manifest as the canonical machine-readable bundle summary while leaving `HANDOFF.md` as the primary human-facing entrypoint.
- Implications:
  - `src/handoff.rs` passes the existing reading-order computation into manifest rendering.
  - Bundle-format/spec docs and build tests must treat the new manifest field as part of the output contract.

---

## D-073: Preview list output should surface canonical reading-order guidance once the manifest provides it

- Date: 2026-03-17
- Decision:
  - Extend `diffship preview --list` and `--list --json` to show `reading_order` from `handoff.manifest.json` when available.
  - Keep the guidance sourced strictly from the manifest rather than rebuilding it in preview.
- Rationale:
  - Once the manifest carries reading-order guidance, preview should expose it directly so humans and CI consumers see the same canonical navigation hints.
  - This keeps preview as a thin consumer of the bundle contract rather than a second author of bundle guidance.
- Implications:
  - `src/preview.rs` parses and forwards `reading_order` opportunistically while remaining backward-compatible with older bundles.
  - Preview docs and tests must cover both the human-readable list output and the JSON field.

---

## D-074: Compare should treat manifest reading-order guidance as a first-class canonical delta

- Date: 2026-03-17
- Decision:
  - Extend `diffship compare` to report manifest `reading_order` deltas when both bundles provide that field.
  - Keep these deltas supplemental to the file-level diff list and summary-count deltas.
- Rationale:
  - Reading-order guidance is now part of the canonical manifest contract, so compare should expose when that navigation guidance changes.
  - This helps downstream tooling understand not just scope drift but also workflow/navigation drift between bundles.
- Implications:
  - `src/bundle_compare.rs` compares manifest `reading_order` lists by position and emits human-readable plus JSON deltas.
  - Compare docs and integration tests must cover the new field while preserving existing exit semantics.

---

## D-075: TUI handoff preview should surface canonical reading-order guidance from the manifest

- Date: 2026-03-17
- Decision:
  - Extend the prepended TUI handoff preview header to include `reading_order` when `handoff.manifest.json` provides it.
  - Keep the TUI as a thin consumer of the existing manifest JSON rather than recomputing guidance from patch content.
- Rationale:
  - CLI preview already surfaces canonical reading-order guidance, so the TUI preview should expose the same navigation hints to preserve parity.
  - Reusing manifest JSON avoids a separate TUI-only interpretation layer.
- Implications:
  - `src/tui/mod.rs` reads `reading_order` opportunistically and remains backward-compatible with older or malformed preview bundles.
  - TUI docs and unit tests should cover the prepended reading-order lines alongside the existing summary counts.

---

## D-076: TUI compare should be a thin consumer of `diffship compare --json`

- Date: 2026-03-17
- Decision:
  - Add a TUI compare screen that invokes `diffship compare --json` and renders a concise report from that JSON.
  - Keep comparison semantics canonical in the CLI compare contract rather than reimplementing bundle comparison inside the TUI.
- Rationale:
  - This preserves CLI/TUI parity while still giving interactive users access to canonical structured-context deltas.
  - Reusing JSON output avoids a second compare implementation and keeps future compare enhancements automatically consumable by the TUI.
- Implications:
  - `src/tui/mod.rs` owns only input/editing/report rendering and delegates comparison work to the existing CLI entrypoint.
  - TUI tests should cover the rendered compare report lines for manifest summary and reading-order deltas.
