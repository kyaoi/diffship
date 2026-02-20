#!/usr/bin/env bash
set -euo pipefail

spec="docs/SPEC_V1.md"
trace="docs/TRACEABILITY.md"

if [[ ! -f "$spec" ]]; then
	echo "error: missing $spec" >&2
	exit 1
fi
if [[ ! -f "$trace" ]]; then
	echo "error: missing $trace" >&2
	exit 1
fi

# IDs in the spec (source of truth)
spec_ids=$(grep -oE '\bS-[A-Z0-9]+(-[A-Z0-9]+)*-[0-9]{3}\b' "$spec" | sort -u)

# IDs in traceability, extracted from requirement mapping lines.
# Expected format (example):
# - **S-FOO-001** — Tests: TBD — Code: TBD — Status: Planned
trace_lines=$(grep -nE '^\s*-\s+\*\*S-[A-Z0-9]+(-[A-Z0-9]+)*-[0-9]{3}\*\*' "$trace" || true)

trace_ids=""
errors=0

# Parse each mapping line and validate Status consistency.
while IFS= read -r entry; do
	[[ -z "$entry" ]] && continue
	lineno="${entry%%:*}"
	line="${entry#*:}"

	id=$(sed -nE 's/.*\*\*(S-[A-Z0-9]+(-[A-Z0-9]+)*-[0-9]{3})\*\*.*/\1/p' <<<"$line")
	if [[ -z "$id" ]]; then
		echo "error: $trace:$lineno: failed to parse requirement ID" >&2
		errors=1
		continue
	fi
	trace_ids+="$id"$'\n'

	# Ensure the required fields exist
	if [[ "$line" != *"— Tests: "* || "$line" != *" — Code: "* || "$line" != *" — Status: "* ]]; then
		echo "error: $trace:$lineno: mapping line must include 'Tests', 'Code', and 'Status' fields" >&2
		echo "       got: $line" >&2
		errors=1
		continue
	fi

	tests_part="${line#*— Tests: }"
	tests="${tests_part%% — Code: *}"
	code_part="${line#*— Code: }"
	code="${code_part%% — Status: *}"
	status="${line##*— Status: }"

	# Trim trailing whitespace
	tests="$(sed -E 's/[[:space:]]+$//' <<<"$tests")"
	code="$(sed -E 's/[[:space:]]+$//' <<<"$code")"
	status="$(sed -E 's/[[:space:]]+$//' <<<"$status")"

	case "$status" in
		Planned|Partial|Implemented|N/A) ;;
		*)
			echo "error: $trace:$lineno: invalid Status '$status' (expected Planned|Partial|Implemented|N/A)" >&2
			errors=1
			continue
			;;
	esac

	tests_is_tbd=false
	code_is_tbd=false
	tests_is_na=false
	code_is_na=false
	[[ "$tests" == "TBD" ]] && tests_is_tbd=true
	[[ "$code" == "TBD" ]] && code_is_tbd=true
	[[ "$tests" == "N/A" ]] && tests_is_na=true
	[[ "$code" == "N/A" ]] && code_is_na=true

	case "$status" in
		N/A)
			if [[ "$tests_is_na" != true || "$code_is_na" != true ]]; then
				echo "error: $trace:$lineno: Status N/A requires Tests: N/A and Code: N/A (got Tests: $tests, Code: $code)" >&2
				errors=1
			fi
			;;
		Planned)
			if [[ "$tests_is_na" == true || "$code_is_na" == true ]]; then
				echo "error: $trace:$lineno: Status Planned should not use N/A (use Status: N/A instead)" >&2
				errors=1
			fi
			if [[ "$tests_is_tbd" != true && "$code_is_tbd" != true ]]; then
				echo "error: $trace:$lineno: Status Planned should keep TBD in Tests or Code (got Tests: $tests, Code: $code)" >&2
				echo "       hint: consider Status: Partial or Implemented" >&2
				errors=1
			fi
			;;
		Partial)
			if [[ "$tests_is_na" == true || "$code_is_na" == true ]]; then
				echo "error: $trace:$lineno: Status Partial should not use N/A (use Status: N/A instead)" >&2
				errors=1
			fi
			if [[ "$tests_is_tbd" == true && "$code_is_tbd" == true ]]; then
				echo "error: $trace:$lineno: Status Partial cannot have both Tests and Code as TBD" >&2
				errors=1
			fi
			if [[ "$tests_is_tbd" != true && "$code_is_tbd" != true ]]; then
				echo "error: $trace:$lineno: Status Partial requires TBD in either Tests or Code" >&2
				echo "       hint: consider Status: Implemented" >&2
				errors=1
			fi
			;;
		Implemented)
			if [[ "$code_is_tbd" == true || "$code_is_na" == true ]]; then
				echo "error: $trace:$lineno: Status Implemented requires a non-TBD, non-N/A Code mapping (got Code: $code)" >&2
				errors=1
			fi
			if [[ "$tests_is_tbd" == true ]]; then
				echo "error: $trace:$lineno: Status Implemented cannot have Tests: TBD" >&2
				echo "       hint: set Tests to a test path or N/A, or change Status" >&2
				errors=1
			fi
			;;
	esac
done <<<"$trace_lines"

trace_ids_sorted=$(printf "%s" "$trace_ids" | sort -u)

# 1) Every spec ID must appear in traceability
missing=0
while IFS= read -r id; do
	[[ -z "$id" ]] && continue
	if ! grep -q "$id" "$trace"; then
		echo "missing traceability mapping for: $id" >&2
		missing=1
	fi
done <<<"$spec_ids"

# 2) No orphan IDs in traceability (keep traceability aligned to spec)
extra_ids=$(comm -13 <(printf "%s\n" "$spec_ids" | sort -u) <(printf "%s\n" "$trace_ids_sorted" | sort -u) || true)
if [[ -n "$extra_ids" ]]; then
	echo "orphan traceability IDs (present in docs/TRACEABILITY.md but not in docs/SPEC_V1.md):" >&2
	echo "$extra_ids" | sed 's/^/  - /' >&2
	errors=1
fi

if [[ "$missing" -ne 0 || "$errors" -ne 0 ]]; then
	echo "" >&2
	echo "Traceability check failed." >&2
	echo "Fix docs/TRACEABILITY.md so it matches docs/SPEC_V1.md and keep Status consistent." >&2
	exit 1
fi

echo "Traceability OK: spec IDs are mapped, no orphans, and Status fields are consistent"
