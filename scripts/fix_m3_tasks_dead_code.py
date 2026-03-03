#!/usr/bin/env python3
from __future__ import annotations

import re
from pathlib import Path

TARGETS = [
    r"^\s*pub\s+fn\s+tasks_present_in_run\s*\(",
    r"^\s*pub\s+fn\s+tasks_dir_in_run\s*\(",
]

ALLOW_LINE = "#[allow(dead_code)]\n"

def ensure_allow(lines: list[str]) -> tuple[list[str], bool]:
    changed = False
    out: list[str] = []
    i = 0
    while i < len(lines):
        line = lines[i]
        matched = any(re.match(pat, line) for pat in TARGETS)
        if matched:
            # If the previous non-empty line is already an allow(dead_code), keep as-is.
            j = len(out) - 1
            while j >= 0 and out[j].strip() == "":
                j -= 1
            if j >= 0 and out[j].strip() == "#[allow(dead_code)]":
                out.append(line)
            else:
                out.append(ALLOW_LINE)
                out.append(line)
                changed = True
            i += 1
            continue

        out.append(line)
        i += 1
    return out, changed

def main() -> None:
    repo_root = Path(".")
    target = repo_root / "src" / "ops" / "tasks.rs"
    if not target.exists():
        raise SystemExit(f"ERROR: not found: {target}")

    original = target.read_text(encoding="utf-8").splitlines(keepends=True)
    updated, changed = ensure_allow(original)

    if changed:
        target.write_text("".join(updated), encoding="utf-8")
        print("OK: inserted #[allow(dead_code)] for tasks helpers")
    else:
        print("OK: already patched (no changes)")

if __name__ == "__main__":
    main()
