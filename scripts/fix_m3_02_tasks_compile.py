#!/usr/bin/env python3
# Fix compile errors introduced by M3-02 tasks wiring:
# - src/ops/apply.rs: user_tasks -> user_tasks_path, and missing fields in ApplySummary initializer(s)
# - src/ops/promote.rs: missing fields in PromoteSummary initializer(s)
#
# This script performs conservative text edits.
from __future__ import annotations
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[1]

def patch_apply_rs(path: Path) -> None:
    s = path.read_text(encoding="utf-8")

    # 1) Fix wrong variable name in ApplyRunMeta (or similar) where user_tasks was referenced.
    s2 = s.replace("user_tasks_path: user_tasks.clone()", "user_tasks_path: user_tasks_path.clone()")
    s2 = s2.replace("user_tasks_path: user_tasks", "user_tasks_path: user_tasks_path")

    # 2) Ensure ApplySummary initializer(s) include tasks_required + user_tasks_path.
    # We patch any block like: let summary = ApplySummary { ... };
    def fix_block(m: re.Match) -> str:
        block = m.group(0)
        # If already has both fields, keep.
        if re.search(r"(?m)^\s*tasks_required\s*:", block) or re.search(r"(?m)^\s*tasks_required\s*,\s*$", block):
            has_tasks = True
        else:
            has_tasks = False
        if re.search(r"(?m)^\s*user_tasks_path\s*:", block) or re.search(r"(?m)^\s*user_tasks_path\s*,\s*$", block):
            has_path = True
        else:
            has_path = False

        if has_tasks and has_path:
            return block

        # Insert right after opening brace line.
        lines = block.splitlines(True)
        out = []
        inserted = False
        for i, ln in enumerate(lines):
            out.append(ln)
            if not inserted and re.search(r"ApplySummary\s*\{\s*$", ln):
                # Insert missing fields using in-scope variables.
                if not has_tasks:
                    out.append("        tasks_required,\n")
                if not has_path:
                    out.append("        user_tasks_path: user_tasks_path.clone(),\n")
                inserted = True
        return "".join(out)

    s3 = re.sub(r"(?ms)let\s+summary\s*=\s*ApplySummary\s*\{.*?\n\s*\};", fix_block, s2)

    if s3 != s:
        path.write_text(s3, encoding="utf-8")

def patch_promote_rs(path: Path) -> None:
    s = path.read_text(encoding="utf-8")

    def fix_block(m: re.Match) -> str:
        block = m.group(0)
        has_present = bool(re.search(r"(?m)^\s*tasks_present\s*:", block) or re.search(r"(?m)^\s*tasks_present\s*,\s*$", block))
        has_path = bool(re.search(r"(?m)^\s*user_tasks_path\s*:", block) or re.search(r"(?m)^\s*user_tasks_path\s*,\s*$", block))
        if has_present and has_path:
            return block
        lines = block.splitlines(True)
        out = []
        inserted = False
        for ln in lines:
            out.append(ln)
            if not inserted and re.search(r"PromoteSummary\s*\{\s*$", ln):
                if not has_present:
                    out.append("        tasks_present,\n")
                if not has_path:
                    out.append("        user_tasks_path: user_tasks_path.clone(),\n")
                inserted = True
        return "".join(out)

    s2 = re.sub(r"(?ms)let\s+summary\s*=\s*PromoteSummary\s*\{.*?\n\s*\};", fix_block, s)

    if s2 != s:
        path.write_text(s2, encoding="utf-8")

def main() -> int:
    apply_rs = ROOT / "src" / "ops" / "apply.rs"
    promote_rs = ROOT / "src" / "ops" / "promote.rs"

    missing = [p for p in [apply_rs, promote_rs] if not p.exists()]
    if missing:
        print("missing files:", ", ".join(str(p) for p in missing), file=sys.stderr)
        return 2

    patch_apply_rs(apply_rs)
    patch_promote_rs(promote_rs)
    print("OK: patched apply.rs and promote.rs")
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
