#!/usr/bin/env bash
set -euo pipefail
repo_root="$(git rev-parse --show-toplevel)"
python3 "$repo_root/scripts/fix_m3_02_tasks_compile.py"
