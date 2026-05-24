#!/usr/bin/env python3
"""
Generate buggy_features.txt for llm-correct-features from a dataset directory.

Two modes are detected automatically:
  jml   — nested subdirectories each containing a base file + numbered variants
           (e.g. buggy-java-jml-eiffel/alphabet/alphabet.e + alphabet_1.e …)
           Compares variants to base to identify the buggy feature, outputs CLASS.feature lines.
  maple — flat directory of numbered .e files with no base counterpart
           (e.g. maple-recursive-eiffel/maple_recursive_absolute_1.e …)
           Each file contains one feature; outputs CLASS.feature from filename + parse.
"""

import re
import sys
import argparse
from pathlib import Path
from typing import List, Optional


def _feature_at_line(lines: List[str], line_num: int) -> Optional[str]:
    """Scan backwards from line_num to find the enclosing feature name."""
    for i in range(line_num - 1, -1, -1):
        m = re.match(r'^\t([a-zA-Z_][a-zA-Z0-9_]*)', lines[i])
        if m and not lines[i].startswith('\t\t') and m.group(1) != 'end':
            return m.group(1)
    return None


# ---------------------------------------------------------------------------
# JML mode — source comparison
# ---------------------------------------------------------------------------

def _strip_variant_numbers(line: str) -> str:
    """Remove _N suffixes from class names so variant numbering is ignored when diffing."""
    return re.sub(r'_\d+\b', '', line)


def _compare_files(base_file: Path, numbered_file: Path) -> Optional[str]:
    try:
        base_lines     = base_file.read_text(encoding='utf-8').splitlines(keepends=True)
        numbered_lines = numbered_file.read_text(encoding='utf-8').splitlines(keepends=True)
    except Exception as e:
        print(f"Error reading {base_file} or {numbered_file}: {e}", file=sys.stderr)
        return None

    for i, (bl, nl) in enumerate(zip(
        [_strip_variant_numbers(l.rstrip('\n\r')) for l in base_lines],
        [_strip_variant_numbers(l.rstrip('\n\r')) for l in numbered_lines],
    ), start=1):
        if bl != nl and (bl.strip() or nl.strip()):
            feature_name = _feature_at_line(numbered_lines, i)
            if not feature_name:
                raise ValueError(f"Cannot identify feature at line {i} in {numbered_file.name}.")
            return feature_name

    return None


def prepare_jml(dataset_dir: Path) -> None:
    """JML mode: compare numbered files to base, output CLASS.feature pairs."""
    for folder in sorted(dataset_dir.iterdir()):
        if not folder.is_dir() or folder.name.startswith('.'):
            continue

        numbered_files = []
        base_files: dict = {}

        for f in folder.glob("*.e"):
            if re.search(r'_\d+\.e$', f.name):
                numbered_files.append(f)
            elif not re.search(r'_\d+', f.name):
                base_files[f.stem] = f

        if not base_files:
            continue

        for nf in sorted(numbered_files):
            m = re.match(r'^(.+?)_\d+\.e$', nf.name)
            if not m:
                continue
            base_file = base_files.get(m.group(1))
            if not base_file:
                continue

            try:
                feature_name = _compare_files(base_file, nf)
                if feature_name:
                    print(f"{nf.stem.upper()}.{feature_name}")
            except ValueError as e:
                print(f"Error: {e}", file=sys.stderr)
                raise


# ---------------------------------------------------------------------------
# Maple mode — flat directory, no base files
# ---------------------------------------------------------------------------

def prepare_maple(dataset_dir: Path) -> None:
    """Maple mode: output CLASS.feature for each top-level .e file (one feature per file)."""
    for e_file in sorted(dataset_dir.glob("*.e")):
        try:
            lines = e_file.read_text(encoding='utf-8', errors='replace').splitlines(keepends=True)
            feature_name = _feature_at_line(lines, len(lines))
            if feature_name:
                print(f"{e_file.stem.upper()}.{feature_name}")
        except Exception as e:
            print(f"Warning: cannot read {e_file}: {e}", file=sys.stderr)


# ---------------------------------------------------------------------------
# Detection + entry point
# ---------------------------------------------------------------------------

def detect_mode(dataset_dir: Path) -> str:
    subdirs_with_e = [
        d for d in dataset_dir.iterdir()
        if d.is_dir() and not d.name.startswith('.') and any(d.glob('*.e'))
    ]
    if subdirs_with_e:
        return 'jml'
    if any(dataset_dir.glob('*.e')):
        return 'maple'
    return 'unknown'


def main(dataset_dir_path: str) -> None:
    dataset_dir = Path(dataset_dir_path)
    if not dataset_dir.is_dir():
        print(f"Not a directory: {dataset_dir}", file=sys.stderr)
        sys.exit(1)

    mode = detect_mode(dataset_dir)
    if mode == 'jml':
        prepare_jml(dataset_dir)
    elif mode == 'maple':
        prepare_maple(dataset_dir)
    else:
        print(f"No .e files found in {dataset_dir}", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("dataset_dir", help="Path to dataset directory")
    args = parser.parse_args()
    main(args.dataset_dir)
