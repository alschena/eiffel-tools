#!/usr/bin/env python3
"""
Verify that every unstaged git change in the dataset is a valid hint-comment removal.

Two valid change forms:
  1. Stripped trailing comment:  "A -- B"  →  "A"
  2. Deleted pure-comment line:  "-- ..."  →  (line deleted, no replacement)

Anything else is reported as a violation.

Usage:
  cd path/to/buggy-java-jml-eiffel
  python3 verify_hints_removed.py
"""

import subprocess
import sys


def strip_comment(line: str) -> str:
    pos = line.find("--")
    return line[:pos].rstrip() if pos >= 0 else line.rstrip()


def is_pure_comment(line: str) -> bool:
    return line.strip().startswith("--")


def check_change_block(removed: list[str], added: list[str], file: str) -> list[str]:
    """
    Check one contiguous block of +/- lines within a hunk.
    Returns a list of violation messages.
    """
    violations = []

    # Pair removed and added lines by position.
    # Extra removed lines (beyond len(added)) are treated as pure deletions.
    for i, r in enumerate(removed):
        if i < len(added):
            a = added[i]
            expected = strip_comment(r)
            if expected == a and r != a:
                pass  # valid: "A -- B" → "A"
            else:
                violations.append(
                    f"  {file}\n"
                    f"    - {r!r}\n"
                    f"    + {a!r}\n"
                    f"    expected + to be {expected!r}"
                )
        else:
            # Deletion with no replacement — must be a pure comment line
            if not is_pure_comment(r):
                violations.append(
                    f"  {file}\n"
                    f"    - {r!r}\n"
                    f"    (deleted with no replacement, but not a pure comment)"
                )

    # Any additions without a corresponding removal are unexpected
    for a in added[len(removed):]:
        violations.append(
            f"  {file}\n"
            f"    + {a!r}\n"
            f"    (addition with no corresponding removal)"
        )

    return violations


def main() -> None:
    result = subprocess.run(
        ["git", "diff"],
        capture_output=True, text=True
    )
    if result.returncode != 0:
        print(f"git diff failed: {result.stderr}", file=sys.stderr)
        sys.exit(1)

    violations = []
    current_file = ""
    removed: list[str] = []
    added:   list[str] = []

    def flush():
        if removed or added:
            violations.extend(check_change_block(removed, added, current_file))
            removed.clear()
            added.clear()

    for line in result.stdout.splitlines():
        if line.startswith("diff --git "):
            flush()
            # "diff --git a/foo/bar.e b/foo/bar.e" → take the b/ path
            current_file = line.split(" b/", 1)[-1]
        elif line.startswith(("--- ", "+++ ", "index ", "@@")):
            flush()
        elif line.startswith("-") and not line.startswith("---"):
            removed.append(line[1:])
        elif line.startswith("+") and not line.startswith("+++"):
            added.append(line[1:])
        else:
            flush()  # context line — end of current change block

    flush()

    if violations:
        print(f"FAIL — {len(violations)} violation(s):\n")
        for v in violations:
            print(v)
        sys.exit(1)
    else:
        print("PASS — all changes are valid hint-comment removals.")


if __name__ == "__main__":
    main()
