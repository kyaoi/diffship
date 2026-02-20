---
name: secrets-warnings
description: Secrets warnings policy: warn + confirm; support --yes and --fail-on-secrets.
---

# Secrets warnings

## Policy
- Only warn; do not print secret values.
- Interactive flow asks for confirmation.
- Non-interactive:
  - `--yes` continues
  - `--fail-on-secrets` exits with code 4

## Config
- `.diffshipignore` should exclude obvious secret paths by default.
