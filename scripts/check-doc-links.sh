#!/usr/bin/env bash
set -euo pipefail

# Check that inline code references to repo paths in key docs actually exist.
# This prevents broken navigation in spec-driven workflows.

root="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd "$root"

DOCS=("README.md" "CONTRIBUTING.md" "AGENTS.md")

# Extract inline code blocks: `...`
extract_tokens() {
	local file="$1"
	# Grep all inline-code segments, strip surrounding backticks.
	# shellcheck disable=SC2016
	grep -oE '`[^`]+`' "$file" 2>/dev/null | sed -e 's/^`//' -e 's/`$//' || true
}

is_candidate_path() {
	local s="$1"

	# Skip empty / whitespace / obvious non-path tokens
	[[ -z "$s" ]] && return 1
	[[ "$s" =~ [[:space:]] ]] && return 1

	# Skip URLs / git refs / option-like tokens
	case "$s" in
	http://* | https://* | ssh://* | git@*) return 1 ;;
	HANDOFF.md) return 1 ;;
	--*) return 1 ;;
	esac

	# Heuristic: consider it a repo path if it contains a slash OR starts with dot OR ends with a file extension
	if [[ "$s" == */* || "$s" == .* || "$s" == *.* || "$s" == */ ]]; then
		return 0
	fi

	return 1
}

check_path_exists() {
	local raw="$1"

	# If token includes glob characters, ensure it matches at least one path.
	if [[ "$raw" == *"*"* || "$raw" == *"?"* || "$raw" == *"["* ]]; then
		local matches=()
		shopt -s nullglob
		# word-splitting is desired for glob expansion here
		# shellcheck disable=SC2206
		matches=($raw)
		shopt -u nullglob

		((${#matches[@]} > 0))
		return
	fi

	# Directory token (ends with /)
	if [[ "$raw" == */ ]]; then
		[[ -d "${raw%/}" ]]
		return
	fi

	[[ -e "$raw" ]]
}

missing=0
echo "docs-check: verifying inline code path references exist..."
for f in "${DOCS[@]}"; do
	[[ -f "$f" ]] || continue
	while IFS= read -r token; do
		is_candidate_path "$token" || continue
		if ! check_path_exists "$token"; then
			echo "Missing path reference: $f -> \`$token\`" >&2
			missing=1
		fi
	done < <(extract_tokens "$f")
done

if [[ "$missing" -ne 0 ]]; then
	echo "docs-check: FAILED (missing references above)" >&2
	exit 1
fi

echo "docs-check: OK"
