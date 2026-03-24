# PLAN (diffship OS)

This file is the single source of truth for product direction, current status, and near-term implementation priorities as **diffship** evolves into an AI-assisted development OS for Git repositories.

It should answer three questions quickly:

1. **What is already implemented?**
2. **What are the next important improvements?**
3. **How should new work be designed and validated?**

---

## Related Documents

* Spec: `docs/SPEC_V1.md`
* Patch bundle contract: `docs/PATCH_BUNDLE_FORMAT.md`
* Config: `docs/CONFIG.md`
* AI workflow guide: `docs/AI_WORKFLOW.md`
* Ops workflow: `docs/OPS_WORKFLOW.md`
* Traceability: `docs/TRACEABILITY.md`
* Decision log: `docs/DECISIONS.md`

---

## Product Goal

The target state is that a user can run the following loop without needing to think about internal implementation details:

```bash
# 1) handoff (local diff -> AI bundle)
diffship build [options...]

# 2) ops (AI patch bundle -> apply/verify/promote)
diffship loop <patch-bundle.zip>
```

### Required outcomes on the ops side

* Keep the user's main working tree clean via **session + sandbox worktree isolation**.
* Run verification through repository-specific or built-in profiles.
* Promote successful results automatically when configured.
* Refuse or pause safely when secrets or required user actions are involved.
* Preserve enough run evidence that failures are diagnosable and repromptable.

### Required outcomes on the handoff side

* Package Git diffs into a deterministic AI-readable bundle.
* Respect ignore rules, secret warnings, packing limits, and risky/binary-file handling.
* Include enough structured context that hosted AI can read the right files in the right order.
* Keep the patch payload canonical while allowing richer machine-readable guidance alongside it.

---

## Official Defaults (V1)

* OS mode: isolated worktrees (`session` + `sandbox`)
* Promotion: `commit`
* Commit policy: `auto`
* Verify profile: `standard`
* Safety: require clean tree, require base commit match, keep path guards, keep locking enabled

These defaults remain the baseline until an explicit decision changes them.

---

## Working Rules

* Always update this `PLAN.md` when status or priorities change.
* Record important product decisions in `docs/DECISIONS.md`.
* If behavior changes, update `docs/SPEC_V1.md`, tests, and `docs/TRACEABILITY.md` in the same change.
* Prefer **small, bounded tasks** with clear done criteria.
* Prefer **1 task = ideally 1 commit** when practical.
* Preserve determinism wherever diffship emits bundles, manifests, JSON views, or zip outputs.
* After relevant changes, run at least:

  * `just docs-check`
  * `just trace-check`
* End product-facing work in a state where `just ci` is expected to pass.

---

## Status Definitions

* `todo`: not started
* `doing`: actively in progress
* `blocked`: waiting on a decision or dependency
* `done`: implemented and verified

---

## Current Product Status (Condensed)

The large milestone sets from M0-M8 are now mostly complete. The current state is:

### Ops core: shipped

* `init`, locking, run persistence, session/sandbox isolation, apply/verify/promote/loop, and `pack-fix` all exist and have integration coverage.
* Secrets and required-user-task stops are implemented.
* Config precedence is implemented across built-in, global, project, manifest, and CLI layers.
* Recovery ergonomics such as `doctor`, session repair, human-readable run ids, and command logs are implemented.

### Handoff core: shipped

* `diffship build` supports committed/staged/unstaged/untracked collection, packing limits, deterministic output, ignored paths, secret warnings, and `HANDOFF.md` entrypoint generation.
* `preview`, `compare`, plan export/replay, named packing profiles, and TUI handoff support are implemented.

### Structured context layer: shipped

* Canonical machine-readable bundle facts are emitted through `handoff.manifest.json` and per-part context JSON.
* Optional rendered/context views such as `handoff.context.xml`, focused project context, and bundle-local `AI_REQUESTS.md` are implemented.
* Structured facts now include reading order, semantic labels, relationship hints, task groups, review/verification/widening/execution/task-shape hints, bounded edit scope, and focused project-context file roles.

### TUI and operator visibility: shipped

* TUI can launch loops, browse runs, inspect handoff bundles, and compare bundle manifests.
* Human-readable status/run views surface heads, paths, and command-log coverage.

### Init / rules export: shipped

* `diffship init` generates repo-local guidance and config stubs.
* `PROJECT_RULES.md` and optional rules zip export support external AI rule UIs.
* Project-local forbid patterns and AI-editable `.diffship/*` policy are supported.
* Generated config comments already frame `post_apply` as a **local normalizer**, not an AI-output repair step.

### Net result

The product is already beyond “core tool exists” stage. The next improvements should focus on:

1. **failure interpretation**,
2. **workflow standardization**,
3. **bundle-local guidance quality**, and
4. **keeping all of that in one AI-facing artifact per phase**.

---

## Key Design Principles Going Forward

### 1. One phase = one AI-facing zip

Do **not** introduce extra sidecar artifacts that users must remember to attach separately.

Preferred model:

* `init` creates **repo-local source-of-truth guidance**.
* `build` exports the relevant guidance into the normal handoff zip.
* `pack-fix` exports the relevant guidance plus failure-aware suggestions into the normal reprompt zip.

This keeps user workflow simple:

* normal iteration -> attach the `build` zip
* failed loop / failed verify -> attach the `pack-fix` zip

No second “rules zip” or “strategy zip” should be required during ordinary use.

### 2. Separate bootstrap from runtime judgment

`init` should remain a **bootstrap / baseline policy** command.
It should not become the main way to decide what to do during day-to-day failures.

That means:

* `init` defines the repo's standard workflow defaults.
* `build` and `pack-fix` export those defaults into AI-facing artifacts.
* failure-specific suggestions are resolved at runtime from run evidence.

### 3. Separate “strategy” from “verify”

These are related but distinct concepts.

#### Strategy

How AI should approach the requested repair or implementation.

Examples:

* `balanced`
* `cautious-tdd`
* `prototype-speed`
* `bugfix-minimal`
* `regression-test-first`
* `docs-sync-minimal`
* `patch-repair-only`
* `no-test-fast`

#### Verify

What local checks diffship should run after apply.

Examples:

* `fast`
* `standard`
* `full`
* repo-defined `[verify.profiles.*]`

A user may want:

* no new tests written by AI, **but** existing local tests still run, or
* fast local verify, **but** AI still writes a regression test first.

The model must support these independently.

### 4. Prefer normalized failure categories over raw error text

Failure handling should not be configured against unstable raw messages.
Instead, diffship should resolve failures into normalized categories such as:

* `patch_apply_failed`
* `base_commit_mismatch`
* `post_apply_failed`
* `verify_test_failed`
* `verify_lint_failed`
* `verify_docs_failed`
* `promotion_blocked_secrets`
* `promotion_blocked_tasks`

All strategy suggestions and per-error overrides should target these categories.

### 5. Suggestions should be advisory by default

For most repositories, strategy output should be **guidance**, not a hard lock.
Default behavior should therefore prefer:

* a selected recommendation,
* a small number of alternatives,
* a short reason,
* explicit next actions.

This keeps pack-fix output helpful without becoming overly prescriptive.

---

## New Product Direction: Workflow Profiles

### Purpose

Different repositories and users want different development cadence:

* some want careful test-first work,
* some want balanced small-task iteration,
* some want very fast prototype-style edits,
* some want bugfix-only minimal changes.

Diffship should make those expectations explicit instead of leaving them implicit inside prompts or tribal knowledge.

### Scope

Workflow profiles are the repo's **standard development posture**.
They are chosen during bootstrap and then exported into AI-facing bundles.

### Initial profile set

Recommended first built-ins:

* `balanced`

  * practical default
  * small bounded tasks
  * add tests when needed
  * standard verify bias
* `cautious-tdd`

  * prefer related failing test first
  * narrow change scope
  * documentation/traceability kept in sync quickly
* `prototype-speed`

  * optimize for speed and quick iteration
  * minimal verification bias
  * broader cleanup deferred when possible
* `bugfix-minimal`

  * fix only the reproduced problem
  * avoid broad refactors
  * good default for narrow regressions
* `no-test-fast`

  * intentionally avoid new regression tests unless unavoidable
  * fix the immediate issue with minimal scope
  * prefer faster verify bias

Note: `bugfix-minimal` and `no-test-fast` can exist both as user-selectable defaults and as runtime-selected recommendations when appropriate.

### Init integration

`diffship init` should gain a workflow-profile selection mechanism, for example:

```bash
diffship init --workflow-profile balanced
```

That selection should drive:

* generated repo-local workflow docs,
* selected default policy comments in generated config,
* later bundle-local exports in `build` and `pack-fix`.

### Repo-local source of truth

Init should generate a dedicated repo-local workflow document, for example:

* `.diffship/WORKFLOW_PROFILE.md`

This file should describe:

* the active default workflow profile,
* what “tests first” means in this repo,
* when docs/traceability updates are expected,
* how broad a change may be,
* preferred verify cadence,
* what should be avoided.

This file is the editable, persistent source for human-maintained workflow expectations.

---

## New Product Direction: Failure-Aware Strategy Resolution

### Purpose

When `loop` or `verify` fails, the user should not need to manually interpret logs before asking AI for help.
Diffship should convert run evidence into a concise “how to proceed” recommendation.

### Runtime behavior

When a run fails and a `pack-fix` zip is produced, diffship should also resolve a **strategy result** using:

1. repo default workflow profile,
2. optional per-error overrides,
3. actual failure category,
4. possibly user-provided CLI override.

### Strategy modes

Recommended config-level modes:

* `suggest`

  * default
  * emit one recommendation plus alternatives
* `prefer`

  * bias toward user-configured defaults more strongly
* `force`

  * strongly prefer configured strategy profile when possible
* `off`

  * disable strategy resolution/export entirely

Default should be `suggest`.

### Per-error override support

Users should be able to say “for this type of failure, prefer this strategy”.
That should be done against normalized categories.

Example direction:

```toml
[workflow.strategy]
mode = "suggest"
default_profile = "balanced"

[workflow.strategy.error_overrides]
patch_apply_failed = "patch-repair-only"
verify_test_failed = "regression-test-first"
verify_docs_failed = "docs-sync-minimal"
post_apply_failed = "bugfix-minimal"
```

### Important constraint

Structural and policy failures may need category-specific handling even if a user prefers `no-test-fast`.
For example:

* corrupt patch / patch does not apply -> patch repair first
* secrets/task acknowledgements -> policy/action handling first

So user override should not erase the distinction between **behavioral fixes** and **structural/policy failures**.

---

## New Product Direction: Bundle-Local Workflow / Strategy Exports

### Build zip exports

The normal handoff zip should be able to include workflow guidance derived from repo-local sources.

Recommended artifacts:

* `WORKFLOW_CONTEXT.md`
* `workflow.context.json`

These should summarize the repo's standard workflow posture in AI-facing terms.

They should be referenced from `AI_REQUESTS.md` so hosted AI can consume them without additional attachments.

### Pack-fix zip exports

The reprompt zip should include both standard workflow context and failure-specific strategy resolution.

Recommended artifacts:

* `WORKFLOW_CONTEXT.md`
* `strategy.resolved.json`

These should be referenced from `PROMPT.md` before or alongside verify/post-apply evidence.

### Preferred pack-fix zip shape

A representative target shape is:

```text
pack-fix_<...>.zip
├─ PROMPT.md
├─ WORKFLOW_CONTEXT.md
├─ strategy.resolved.json
├─ run/
│  ├─ verify.json
│  ├─ commands.json
│  ├─ verify/...
│  ├─ post_apply.json
│  └─ post-apply/...
├─ bundle/
│  └─ manifest.yaml
└─ metadata.json
```

### Strategy resolution artifact shape

The machine-readable strategy result should make it clear:

* which failure category was detected,
* which default profile the repo uses,
* which strategy was selected,
* which alternatives remain valid,
* whether tests are expected or avoided,
* which verify profile is preferred next.

Representative fields:

* `failure_category`
* `strategy_mode`
* `default_profile`
* `selected_profile`
* `alternatives`
* `reason`
* `tests_expected`
* `preferred_verify_profile`
* `next_actions`

---

## New Product Direction: Future Local Judgment Command

Not required for the first implementation, but highly desirable later:

* `diffship strategy --latest`
* `diffship strategy --run-id <id>`
* or `diffship explain --run-id <id>`

This should reuse the same internal resolver as `pack-fix` so the user can inspect the same strategy result locally without opening the zip.

This is a follow-up convenience layer, not the first blocker.

---

## Development Policy for Implementing the New Direction

### Scope policy

Implement the workflow/strategy direction in small vertical slices.
Do not try to redesign `init`, `build`, `pack-fix`, config, and TUI all at once.

### Recommended implementation order

1. add workflow profile concept to generated docs/config shape
2. add bundle-local workflow export in `build`
3. add strategy resolution model for normalized failure categories
4. add strategy export in `pack-fix`
5. only later consider local `strategy` / `explain` command or TUI surfacing

### Testing policy

Use a balanced, bounded approach:

* write focused tests for config parsing / profile resolution / normalized failure classification / emitted bundle files
* prefer regression tests for resolver behavior and export contents
* avoid introducing broad brittle end-to-end coverage when narrow targeted tests are enough
* keep determinism expectations explicit for new JSON/Markdown/zip artifacts

### Docs policy

If new workflow/strategy behavior changes product-facing behavior, update together:

* `docs/SPEC_V1.md`
* `docs/CONFIG.md`
* `docs/OPS_WORKFLOW.md`
* `docs/AI_WORKFLOW.md`
* `docs/TRACEABILITY.md`
* this `PLAN.md`

---

## Next Milestone Proposal

The next milestone should focus on workflow standardization and failure-aware bundle guidance.

### M9: Workflow profiles and failure-aware strategy guidance

| ID    | Status | Description                      | Done Criteria                                                                                                                                                                             |
| ----- | ------ | -------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| M9-01 | done   | Workflow profile concept in init | `diffship init` accepts a workflow-profile selection and generates repo-local workflow guidance (for example `.diffship/WORKFLOW_PROFILE.md`) without regressing existing generated files |
| M9-02 | done   | Workflow config shape            | Project config supports a stable `[workflow]` / `[workflow.strategy]` schema with default profile and strategy mode, documented in `docs/CONFIG.md`                                       |
| M9-03 | done   | Build-side workflow export       | `diffship build` can emit bundle-local workflow guidance (`WORKFLOW_CONTEXT.md` and/or `workflow.context.json`) and `AI_REQUESTS.md` references it deterministically                      |
| M9-04 | done   | Normalized failure categories    | Ops failures resolve into stable normalized categories suitable for strategy selection rather than raw stderr matching alone                                                              |
| M9-05 | done   | Strategy resolver                | `pack-fix` resolves a selected strategy plus alternatives from repo defaults, strategy mode, per-error overrides, and detected failure category                                           |
| M9-06 | done   | Pack-fix strategy export         | Generated reprompt zips include failure-aware strategy export (for example `strategy.resolved.json`) and `PROMPT.md` points AI at it before or alongside verify/post-apply evidence       |
| M9-07 | done   | Explosive-speed profile support  | Built-in strategy set includes a documented fast path such as `no-test-fast`, while still preserving category-specific handling for structural/policy failures                            |
| M9-08 | done   | Determinism + tests              | Workflow and strategy exports are deterministic and covered by focused tests for config resolution, failure classification, and exported artifact shape                                   |

---

## What Is Explicitly Not a Near-Term Goal

To prevent scope creep, the following are not immediate blockers:

* adding many new rendered views beyond current canonical JSON + selected Markdown entrypoints
* broad TUI redesign around workflow strategy before the core config/export path exists
* over-automating runtime judgment before failure categories are stable
* requiring a second external attachment beyond the ordinary build zip or pack-fix zip

---

## Current Priority Order

1. Keep the existing build/loop/pack-fix path stable.
2. Add repo-standard workflow profiles through `init` and config.
3. Export workflow guidance into normal build zips.
4. Resolve failure-aware strategies into pack-fix zips.
5. Only then consider richer local/TUI judgment surfaces.

---

## Notes

* When in doubt, preserve the rule: **one phase = one AI-facing zip**.
* `init` remains bootstrap-focused; runtime failure judgment belongs in build/pack-fix/runtime layers.
* Strategy output should remain advisory by default.
* “Tests first” and “fast/no new tests” should both be supported as valid repo/user preferences.
* Structural failures and policy stops must remain distinguishable from normal bugfix strategy selection.
