---
name: secrets-warnings
description: Adjust handoff secret warnings or ops-side secret and task acknowledgements without leaking secret values.
---

# Secrets and warnings

Use this when changing secret detection, warning output, or promotion-blocking acknowledgements.

## Read first
1) `docs/SPEC_V1.md` for `S-SECRETS-*`, `S-OPS-SECRETS-*`, and `S-OPS-TASKS-001`
2) `docs/OPS_WORKFLOW.md`
3) `docs/BUNDLE_FORMAT.md` if `secrets.md` or handoff warnings change

## Handoff-side rules
- Detect likely secrets and emit paths plus reasons only.
- Never print secret values.
- Keep `--yes` and `--fail-on-secrets` behavior aligned with the documented exit flow.
- If bundle output changes, keep `secrets.md` deterministic and documented.

## Ops-side rules
- Scan patch bundles and produced logs/diffs for likely secrets.
- Block promotion by default until the user passes `--ack-secrets`.
- If `tasks/USER_TASKS.md` is present, block promotion until `--ack-tasks`.
- Never print secret values in logs, terminal output, or generated markdown.

## Tests
- `tests/m6_handoff_build.rs` for handoff warnings
- `tests/m2_promotion_loop.rs` for promotion blocking and acknowledgements
- `tests/m3_tasks.rs` for required user tasks
