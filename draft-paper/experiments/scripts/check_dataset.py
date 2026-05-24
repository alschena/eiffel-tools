#!/usr/bin/env python3
"""
Verify AutoProof results for the entire buggy-java-jml-eiffel dataset.

For each problem directory:
  base classes    — must verify      (0 errors)
  buggy variants  — must NOT verify  (detected via _compare_files)
  clean helpers   — must verify      (pure renames of a base, no real diffs)

Must be run from the buggy-java-jml-eiffel dataset directory.

Usage:
  cd datasets/buggy-java-jml-eiffel
  python3 ../../scripts/check_dataset.py [problem_folder]
"""

import re
import sys
from pathlib import Path
from typing import Literal

sys.path.insert(0, str(Path(__file__).parent))
from prepare import extract_class_name, _compare_files
import autoproof_runner as ap

Kind = Literal["base", "buggy", "helper"]


def classify_folder(folder: Path) -> list[tuple[Path, Kind]]:
    """
    Returns [(file, kind), ...] for every .e file in folder.
    kind is 'base', 'buggy', or 'helper'.
    """
    base_files: dict[str, Path] = {}
    numbered_files: list[Path] = []

    for f in folder.glob("*.e"):
        if re.search(r"_\d+\.e$", f.name):
            numbered_files.append(f)
        else:
            base_files[f.stem] = f

    result: list[tuple[Path, Kind]] = [(f, "base") for f in sorted(base_files.values())]

    for nf in sorted(numbered_files):
        m = re.match(r"^(.+?)_\d+\.e$", nf.name)
        if not m:
            continue
        base_file = base_files.get(m.group(1))
        if not base_file:
            continue
        # _compare_files returns None when only class-name differences exist
        kind: Kind = "buggy" if _compare_files(base_file, nf) is not None else "helper"
        result.append((nf, kind))

    return result


def main() -> None:
    dataset_dir = Path(".")
    problem_filter = sys.argv[1] if len(sys.argv) > 1 else ""

    n_expected = 0
    n_unexpected = 0
    unexpected: list[str] = []

    for folder in sorted(dataset_dir.iterdir()):
        if not folder.is_dir() or folder.name.startswith("."):
            continue
        if problem_filter and folder.name != problem_filter:
            continue

        entries = classify_folder(folder)
        if not entries:
            continue

        print(f"=== {folder.name} ===", flush=True)

        for f, kind in entries:
            lines = f.read_text(encoding="utf-8").splitlines(keepends=True)
            class_name, _ = extract_class_name(lines)
            if not class_name:
                continue

            print(f"  running {class_name} ...", end=" ", flush=True)
            passed, result = ap.run(class_name)

            if kind == "buggy":
                expected_pass = False
                label_ok   = "variant — correctly fails"
                label_fail = "variant — should fail"
            else:
                expected_pass = True
                label_ok   = f"{kind} — correctly passes"
                label_fail = f"{kind} — should verify"

            ok = (passed == expected_pass)
            label = label_ok if ok else label_fail
            marker = "ok  " if ok else "FAIL"
            print(f"{marker} ({result})", flush=True)
            print(f"       {class_name:<50} {label}", flush=True)

            if ok:
                n_expected += 1
            else:
                n_unexpected += 1
                unexpected.append(f"{class_name}  ({label})")

    print(f"\nExpected: {n_expected}   Unexpected: {n_unexpected}")
    if unexpected:
        print("\nUnexpected results:")
        for u in unexpected:
            print(f"  {u}")
        sys.exit(1)


if __name__ == "__main__":
    main()
