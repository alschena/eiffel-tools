#!/usr/bin/env bash
# Sanity-check the buggy-java-jml-eiffel dataset:
#   base classes          → must verify
#   buggy variant classes → must NOT verify  (identified via prepare.py)
#   clean helper classes  → must verify      (e.g. SWAP_IN_ARRAY_N)
#
# Must be run from the buggy-java-jml-eiffel dataset directory.
# Optional first argument: restrict to one problem folder (e.g. "bubble_sort").
#
# Usage:
#   cd datasets/buggy-java-jml-eiffel
#   ../../scripts/check_dataset.sh
#   ../../scripts/check_dataset.sh bubble_sort

set -uo pipefail

AUTOPROOF="${AP}/EIFGENs/batch/F_code/ecb"
PROOF_OUT="EIFGENs/experiment_example/Proofs/output0.txt"
PREPARE_PY="$(dirname "$0")/prepare.py"
TIMEOUT_SECS=120

n_expected=0
n_unexpected=0
unexpected=()

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

class_name_of() {
    awk '
        { gsub(/\r/, ""); sub(/^\xef\xbb\xbf/, "") }
        /^class$/ { found = 1; next }
        found && /[A-Z]/ { gsub(/[\t ]/, ""); print; exit }
    ' "$1"
}

run_autoproof() {
    local class="$1"
    rm -f "$PROOF_OUT"
    "$AUTOPROOF" -autoproof "$class" > /dev/null 2>&1 &
    local pid=$!
    local elapsed=0
    while kill -0 "$pid" 2>/dev/null && (( elapsed < TIMEOUT_SECS )); do
        grep -q "Boogie program verifier finished" "$PROOF_OUT" 2>/dev/null && break
        sleep 2; (( elapsed += 2 )) || true
    done
    kill "$pid" 2>/dev/null || true
    pkill -9 -f boogie 2>/dev/null || true
    wait "$pid" 2>/dev/null || true
    (( elapsed >= TIMEOUT_SECS )) && { echo "  TIMEOUT after ${TIMEOUT_SECS}s"; return 1; }
    local result
    result=$(grep "Boogie program verifier finished" "$PROOF_OUT" 2>/dev/null | tail -1)
    echo "  $result"
    echo "$result" | grep -q ", 0 errors"
}

record() {
    local label="$1" class="$2" ok="$3"
    if [[ "$ok" == yes ]]; then
        printf "  ok   %-50s %s\n" "$class" "$label"
        (( n_expected++ )) || true
    else
        printf "  FAIL %-50s %s\n" "$class" "$label"
        unexpected+=("$class  ($label)")
        (( n_unexpected++ )) || true
    fi
}

# ---------------------------------------------------------------------------
# Build set of buggy class names from prepare.py output
# ---------------------------------------------------------------------------

problem_filter="${1:-}"

# prepare.py outputs "CLASS_N.feature" or "CLASS_N" lines for buggy variants
buggy_classes=()
while IFS= read -r line; do
    cls="${line%%.*}"   # strip .feature_name if present
    buggy_classes+=("$cls")
done < <(python3 "$PREPARE_PY" . 2>/dev/null)

is_buggy() {
    local cls="$1"
    for b in "${buggy_classes[@]}"; do
        [[ "$b" == "$cls" ]] && return 0
    done
    return 1
}

# ---------------------------------------------------------------------------
# Main loop
# ---------------------------------------------------------------------------

for dir in */; do
    dir="${dir%/}"
    [[ -d "$dir" ]] && [[ "$dir" != .* ]] || continue
    [[ -n "$problem_filter" && "$dir" != "$problem_filter" ]] && continue

    # Collect base files and variant files
    base_files=()
    variant_files=()
    for f in "$dir"/*.e; do
        [[ -f "$f" ]] || continue
        if [[ "$f" =~ _[0-9]+\.e$ ]]; then
            variant_files+=("$f")
        else
            base_files+=("$f")
        fi
    done
    [[ ${#base_files[@]} -eq 0 ]] && continue

    echo "=== $dir ==="

    # Base classes must verify
    for bf in "${base_files[@]}"; do
        cls=$(class_name_of "$bf")
        [[ -z "$cls" ]] && continue
        if run_autoproof "$cls"; then
            record "base — correctly passes" "$cls" yes
        else
            record "base — should verify"   "$cls" no
        fi
    done

    # Variants: buggy ones must fail, clean helpers must pass
    for vf in "${variant_files[@]}"; do
        cls=$(class_name_of "$vf")
        [[ -z "$cls" ]] && continue
        if is_buggy "$cls"; then
            if run_autoproof "$cls"; then
                record "variant — should fail"     "$cls" no
            else
                record "variant — correctly fails" "$cls" yes
            fi
        else
            if run_autoproof "$cls"; then
                record "helper — correctly passes" "$cls" yes
            else
                record "helper — should verify"   "$cls" no
            fi
        fi
    done
done

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------

echo ""
echo "Expected: $n_expected   Unexpected: $n_unexpected"

if [[ "${#unexpected[@]}" -gt 0 ]]; then
    echo ""
    echo "Unexpected results:"
    printf '  %s\n' "${unexpected[@]}"
    exit 1
fi
