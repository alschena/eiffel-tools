#!/usr/bin/env python3
"""
Verify that every base (correct) class in buggy-java-jml-eiffel verifies with AutoProof.

Must be run from the buggy-java-jml-eiffel dataset directory.

Usage:
  cd datasets/buggy-java-jml-eiffel
  python3 ../../scripts/check_bases.py
"""

import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from prepare import extract_class_name
import autoproof_runner as ap


def main() -> None:
    dataset_dir = Path(".")
    n_pass = 0
    n_fail = 0
    failures: list[str] = []

    for folder in sorted(dataset_dir.iterdir()):
        if not folder.is_dir() or folder.name.startswith("."):
            continue

        base_files = [
            f for f in folder.glob("*.e")
            if not re.search(r"_\d+\.e$", f.name)
        ]
        if not base_files:
            continue

        for base_file in sorted(base_files):
            lines = base_file.read_text(encoding="utf-8").splitlines(keepends=True)
            class_name, _ = extract_class_name(lines)
            if not class_name:
                continue

            print(f"  running {class_name} ...", end=" ", flush=True)
            passed, result = ap.run(class_name)
            if passed:
                print(f"ok   ({result})", flush=True)
                n_pass += 1
            else:
                print(f"FAIL ({result})", flush=True)
                failures.append(class_name)
                n_fail += 1

    print(f"\nPassed: {n_pass}   Failed: {n_fail}")
    if failures:
        print("Failures:")
        for f in failures:
            print(f"  {f}")
        sys.exit(1)


if __name__ == "__main__":
    main()
