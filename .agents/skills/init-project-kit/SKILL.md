---
name: init-project-kit
description: Change diffship init templates or generated .diffship project-kit artifacts without breaking the documented AI workflow contract.
---

# Init project kit

Use this when changing `diffship init`, template files, or generated `.diffship/*` guides.

## Read first
1) `docs/SPEC_V1.md` section 10 plus `S-GOAL-010` and `S-INIT-001..011`
2) `docs/PROJECT_KIT_TEMPLATE.md`
3) `docs/AI_PROJECT_TEMPLATE.md`
4) `docs/AI_WORKFLOW.md`
5) `docs/TRACEABILITY.md`

## Generated artifacts
- `.diffship/PROJECT_KIT.md`
- `.diffship/PROJECT_RULES.md`
- `.diffship/AI_GUIDE.md`
- `.diffship/forbid.toml`
- `.diffship/config.toml`
- rules-kit zip under `.diffship/artifacts/rules/` unless `--out` overrides it

## Rules
- Keep core workflow language aligned with the implemented diffship contracts.
- `--template-dir` must override template sources before falling back to repo defaults.
- `--lang en|ja` affects the generated `PROJECT_RULES.md` snippet as specified.
- `--refresh-forbid` must rewrite only the forbid file.
- Generated rules kits stay deterministic and clearly separate loop-ready ops bundles from analysis-only/non-ops outputs.
- Keep "Customize this section" blocks easy for humans to maintain across upgrades.

## Tests
- `tests/m0_integration.rs`

## Related skills
- Use `ops-safety` if the change affects forbid rules, config semantics, or loop safety rather than template wording.
