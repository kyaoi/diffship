---
name: ops-safety
description: Change apply, verify, promote, loop, or cleanup behavior without weakening safety defaults, isolation, or run logs.
---

# Ops safety

Use this when touching `src/ops/*`, patch-bundle handling, or promotion flow.

## Read first
1) `docs/SPEC_V1.md` sections for `S-APPLY-*`, `S-VERIFY-*`, `S-COMMIT-*`, `S-OPS-*`, `S-CLEANUP-*`
2) `docs/PATCH_BUNDLE_FORMAT.md`
3) `docs/OPS_WORKFLOW.md`
4) `docs/CONFIG.md`

## Non-negotiables
- Acquire the lock before mutating (`.diffship/lock`).
- Apply in isolated sandbox worktrees; do not mutate the user's main working tree during apply/verify.
- Require base-commit match by default; any CLI override must still resolve to the session head.
- Enforce strict repo-relative path guards plus project/local forbid rules.
- Run preflight before mutation and rollback automatically on failure.
- Run only locally configured post-apply or verify commands.
- Keep machine-readable run artifacts under `.diffship/runs/<run-id>/`.
- Secrets and required user tasks block promotion by default.
- Promotion mode and commit policy must remain coherent.

## Files commonly involved
- `src/ops/apply.rs`, `verify.rs`, `promote.rs`, `loop_cmd.rs`
- `src/ops/patch_bundle.rs`, `post_apply.rs`, `config.rs`
- `src/ops/run.rs`, `lock.rs`, `cleanup.rs`, `session.rs`, `worktree.rs`

## Tests to keep green
- `tests/m2_apply_verify.rs`
- `tests/m2_promotion_loop.rs`
- `tests/m2_pack_fix.rs`
- `tests/m3_tasks.rs`
- `tests/m4_config_precedence.rs`
- `tests/m4_verify_profiles.rs`
- `tests/m7_cleanup.rs`

## Related skills
- Use `secrets-warnings` for secret/task acknowledgement flow.
- Use `init-project-kit` when the change is about generated `.diffship/forbid.toml` or init-time config scaffolding.
