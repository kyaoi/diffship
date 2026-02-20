---
name: untracked-binary
description: Untracked/binary handling modes: auto/patch/raw/meta and how to keep output sane.
---

# Untracked & binary handling

## Defaults
- Untracked: OFF by default
- Binary: excluded by default

## Untracked modes
- auto: text/small → patch, binary/large → raw
- patch: represent as add-diff when possible
- raw: store in attachments.zip
- meta: record only in HANDOFF.md

## Binary modes (when --include-binary)
- raw (default): attachments.zip
- patch: only if explicitly requested; warn about size/readability
- meta: record only

## `.diffshipignore`
Use it to exclude patterns (e.g., `*.png`) to avoid shipping irrelevant binaries.
