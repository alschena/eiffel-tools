#!/usr/bin/env bash
# Verify that every base (correct) class in buggy-java-jml-eiffel verifies with AutoProof.
# Must be run from the buggy-java-jml-eiffel dataset directory.

set -uo pipefail

AUTOPROOF="${AP}/EIFGENs/batch/F_code/ecb"
PROOF_OUT="EIFGENs/experiment_example/Proofs/output0.txt"
TIMEOUT_SECS=120

n_pass=0
n_fail=0
failures=()

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
    (( elapsed >= TIMEOUT_SECS )) && { echo "TIMEOUT"; return 1; }
    local result
    result=$(grep "Boogie program verifier finished" "$PROOF_OUT" 2>/dev/null | tail -1)
    echo "$result" | grep -q ", 0 errors" && return 0
    echo "$result"
    return 1
}

for dir in */; do
    dir="${dir%/}"
    [[ -d "$dir" ]] && [[ "$dir" != .* ]] || continue

    base_file=""
    for f in "$dir"/*.e; do
        [[ -f "$f" ]] && [[ ! "$f" =~ _[0-9]+\.e$ ]] && { base_file="$f"; break; }
    done
    [[ -z "$base_file" ]] && continue

    class=$(class_name_of "$base_file")
    [[ -z "$class" ]] && continue

    if run_autoproof "$class"; then
        printf "ok   %s\n" "$class"
        (( n_pass++ )) || true
    else
        printf "FAIL %s\n" "$class"
        failures+=("$class")
        (( n_fail++ )) || true
    fi
done

echo ""
echo "Passed: $n_pass   Failed: $n_fail"
if [[ "${#failures[@]}" -gt 0 ]]; then
    echo "Failures:"; printf '  %s\n' "${failures[@]}"; exit 1
fi
