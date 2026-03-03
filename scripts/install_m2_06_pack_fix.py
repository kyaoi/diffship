#!/usr/bin/env python3
from __future__ import annotations
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[1]

PACK_FIX_RS = (ROOT / "src" / "ops" / "pack_fix.rs").read_text(encoding="utf-8")


def ensure_file(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")


def patch_cargo_toml() -> None:
    path = ROOT / "Cargo.toml"
    s = path.read_text(encoding="utf-8")
    if re.search(r'^\s*zip\s*=\s*".+"\s*$', s, flags=re.M):
        return

    m = re.search(r'^\[dependencies\]\s*$', s, flags=re.M)
    if not m:
        raise SystemExit("Cargo.toml: missing [dependencies] section")

    # Insert zip dependency right after [dependencies]
    insert_at = m.end()
    s2 = s[:insert_at] + '\nzip = "0.6.6"\n' + s[insert_at:]
    path.write_text(s2, encoding="utf-8")


def patch_cli_rs() -> None:
    path = ROOT / "src" / "cli.rs"
    s = path.read_text(encoding="utf-8")

    if "PackFix(" in s or "PackFixArgs" in s:
        return

    # Insert Command variant after Verify(...)
    # We rely on doc comment line that contains "Verify(".
    s = re.sub(
        r'(\s*/// Run verification[^\n]*\n\s*Verify\(VerifyArgs\),\n)',
        r'\1\n    /// Create a reprompt zip for a run (M2-06)\n    #[command(name = "pack-fix")]\n    PackFix(PackFixArgs),\n',
        s,
        count=1,
        flags=re.M,
    )

    if "PackFix(PackFixArgs)" not in s:
        # fallback: insert before internal test helpers or before enum close
        s = re.sub(
            r'(\n\s*/// Internal test helper:)',
            r'\n    /// Create a reprompt zip for a run (M2-06)\n    #[command(name = "pack-fix")]\n    PackFix(PackFixArgs),\n\n\1',
            s,
            count=1,
            flags=re.M,
        )

    # Add args struct after VerifyArgs
    s = re.sub(
        r'(\n\#\[derive\(Debug, Args\)\]\npub struct VerifyArgs\s*\{[\s\S]*?\n\}\n)',
        r'\1\n\n#[derive(Debug, Args)]\npub struct PackFixArgs {\n    /// Run id to pack (defaults to the latest run)\n    #[arg(long)]\n    pub run_id: Option<String>,\n\n    /// Output zip path (default: .diffship/runs/<run-id>/pack-fix.zip)\n    #[arg(long)]\n    pub out: Option<String>,\n}\n',
        s,
        count=1,
        flags=re.M,
    )

    if "pub struct PackFixArgs" not in s:
        raise SystemExit("cli.rs: failed to inject PackFixArgs")

    path.write_text(s, encoding="utf-8")


def patch_ops_mod_rs() -> None:
    path = ROOT / "src" / "ops" / "mod.rs"
    s = path.read_text(encoding="utf-8")

    if re.search(r'^\s*mod\s+pack_fix;\s*$', s, flags=re.M) is None:
        # insert after verify module if present, else at end of module list
        if "mod verify;" in s:
            s = s.replace("mod verify;\n", "mod verify;\nmod pack_fix;\n")
        else:
            # insert before first blank line after module list
            s = re.sub(r'^(mod [^;]+;\n)+', lambda m: m.group(0) + "mod pack_fix;\n", s, count=1)

    # add match arm
    if "Command::PackFix" not in s:
        s = re.sub(
            r'(Command::Verify\(args\)\s*=>\s*verify::cmd\(&git_root,\s*args\),\n)',
            r'\1        Command::PackFix(args) => pack_fix::cmd(&git_root, args),\n',
            s,
            count=1,
            flags=re.M,
        )

    if "pack_fix::cmd" not in s:
        # fallback: insert near other arms before test helpers
        s = re.sub(
            r'(\n\s*Command::__Test)',
            r'\n        Command::PackFix(args) => pack_fix::cmd(&git_root, args),\1',
            s,
            count=1,
            flags=re.M,
        )

    path.write_text(s, encoding="utf-8")


def patch_verify_rs() -> None:
    path = ROOT / "src" / "ops" / "verify.rs"
    s = path.read_text(encoding="utf-8")

    if "pack_fix::" not in s and "ops::pack_fix" not in s:
        # Insert `use crate::ops::pack_fix;` after an existing ops import if possible.
        if "use crate::ops::worktree;\n" in s:
            s = s.replace(
                "use crate::ops::worktree;\n",
                "use crate::ops::worktree;\nuse crate::ops::pack_fix;\n",
            )
        else:
            # Insert after the last `use crate::ops::...;`
            ops_uses = list(re.finditer(r"^use crate::ops::.*;$", s, flags=re.M))
            if ops_uses:
                last = ops_uses[-1]
                insert_at = last.end()
                s = s[:insert_at] + "\nuse crate::ops::pack_fix;\n" + s[insert_at:]
            else:
                # Fallback: insert after the last `use crate::...;`
                crate_uses = list(re.finditer(r"^use crate::.*;$", s, flags=re.M))
                if crate_uses:
                    last = crate_uses[-1]
                    insert_at = last.end()
                    s = s[:insert_at] + "\nuse crate::ops::pack_fix;\n" + s[insert_at:]
                else:
                    s = "use crate::ops::pack_fix;\n" + s

    if "try_write_default_pack_fix_zip" in s:
        path.write_text(s, encoding="utf-8")
        return

    # Insert before the final Err(ExitError::new(EXIT_VERIFY_FAILED,...))
    pat = r"\n\s*Err\(ExitError::new\(\n\s*EXIT_VERIFY_FAILED,"
    m = re.search(pat, s)
    if not m:
        raise SystemExit("verify.rs: could not find EXIT_VERIFY_FAILED return site")

    insert_pos = m.start()
    snippet = """
    match pack_fix::try_write_default_pack_fix_zip(git_root, &run_id, &run_dir, &sandbox_path, &created_at) {
        Ok(p) => eprintln!("diffship verify: pack-fix saved to {}", p.display()),
        Err(e) => eprintln!("diffship verify: pack-fix failed: {}", e),
    }
"""
    s = s[:insert_pos] + "\n" + snippet + s[insert_pos:]
    path.write_text(s, encoding="utf-8")


def main() -> None:
    # Ensure module file exists (we ship it in the zip, but keep it robust)
    ensure_file(ROOT / "src" / "ops" / "pack_fix.rs", PACK_FIX_RS)

    patch_cargo_toml()
    patch_cli_rs()
    patch_ops_mod_rs()
    patch_verify_rs()

    print("OK: installed M2-06 pack-fix")


if __name__ == "__main__":
    try:
        main()
    except Exception as e:
        print(f"ERROR: {e}", file=sys.stderr)
        sys.exit(1)
