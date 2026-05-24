#!/usr/bin/env python3
"""
Generate classes.txt for llm-correct-features from a dataset directory.

Two modes are detected automatically:
  jml   — nested subdirectories each containing a base file + numbered variants
           (e.g. buggy-java-jml-eiffel/alphabet/alphabet.e + alphabet_1.e …)
           Compares variants to base, strips hint comments, outputs CLASS.feature lines.
  maple — flat directory of numbered .e files with no base counterpart
           (e.g. maple-recursive-eiffel/maple_recursive_absolute_1.e …)
           Outputs class names only; the tool runs AutoProof to discover failures.
"""

import os
import re
import sys
import difflib
import argparse
from pathlib import Path
from typing import List, Tuple, Optional


# ---------------------------------------------------------------------------
# Shared helpers
# ---------------------------------------------------------------------------

def extract_class_name(lines: List[str]) -> Tuple[str, int]:
    for i, line in enumerate(lines):
        stripped = line.strip().lstrip('﻿').strip()
        if stripped == "class":
            for j in range(i + 1, len(lines)):
                candidate = lines[j].strip().lstrip('﻿').strip()
                if candidate:
                    return (candidate, j + 1)
    return ("", 0)


def parse_features(lines: List[str]) -> List[Tuple[str, int, int]]:
    features = []
    in_feature_block = False
    current_feature = None
    feature_start = None

    for i, line in enumerate(lines, start=1):
        stripped = line.strip().lstrip('﻿').strip()

        if stripped == "feature" or re.match(r'^feature\s*\{', stripped):
            in_feature_block = True
            continue

        if in_feature_block:
            if line.startswith("\t") and not line.startswith("\t\t"):
                if stripped and stripped != "end":
                    match = re.match(r'^\t([a-zA-Z_][a-zA-Z0-9_]*)', line)
                    if match:
                        if current_feature and feature_start:
                            features.append((current_feature, feature_start, i - 1))
                        current_feature = match.group(1)
                        feature_start = i
            elif line.startswith("\t") and stripped == "end" and not line.startswith("\t\t"):
                if current_feature and feature_start:
                    features.append((current_feature, feature_start, i))
                    current_feature = None
                    feature_start = None

    if current_feature and feature_start:
        features.append((current_feature, feature_start, len(lines)))

    return features


def find_feature_for_line(line_num: int, features: List[Tuple[str, int, int]]) -> Optional[str]:
    for name, start, end in features:
        if start <= line_num <= end:
            return name
    return None


# ---------------------------------------------------------------------------
# JML mode — source comparison + comment stripping
# ---------------------------------------------------------------------------

def _is_only_type_name_change(base_str: str, numbered_str: str,
                               base_class: str, numbered_class: str) -> bool:
    base_norm = base_str.replace(base_class, "CLASS_NAME_PLACEHOLDER")
    num_norm  = numbered_str.replace(numbered_class, "CLASS_NAME_PLACEHOLDER")

    if base_norm.strip() == num_norm.strip():
        return True

    type_pat = r'\b([A-Z][A-Z0-9_]*)\b'
    base_types = set(re.findall(type_pat, base_norm)) - {"CLASS_NAME_PLACEHOLDER"}
    num_types  = set(re.findall(type_pat, num_norm))  - {"CLASS_NAME_PLACEHOLDER"}

    if len(base_types) != len(num_types):
        return False

    for bt in base_types:
        found = any(
            nt == bt or (nt.startswith(bt + "_") and nt[len(bt)+1:].isdigit())
            for nt in num_types
        )
        if not found:
            found = any(
                bt.startswith(nt + "_") and bt[len(nt)+1:].isdigit()
                for nt in num_types
            )
        if not found:
            return False

    base_ph = base_norm
    num_ph  = num_norm
    for bt in base_types:
        base_ph = re.sub(r'\b' + re.escape(bt) + r'\b', 'T', base_ph)
        for nt in num_types:
            if nt == bt or (nt.startswith(bt + "_") and nt[len(bt)+1:].isdigit()):
                num_ph = re.sub(r'\b' + re.escape(nt) + r'\b', 'T', num_ph)
                break

    return base_ph.strip() == num_ph.strip()


def _remove_comment(line: str) -> str:
    pos = line.find("--")
    return line[:pos].rstrip() if pos >= 0 else line


def _strip_comments(lines: List[str], line_nums: List[int]) -> List[str]:
    result = list(lines)
    for n in line_nums:
        if 0 < n <= len(result):
            orig = result[n - 1]
            nl   = orig.endswith('\n')
            stripped = _remove_comment(orig.rstrip('\n\r'))
            result[n - 1] = stripped + ('\n' if nl else '')
    return result


def _compare_files(base_file: Path, numbered_file: Path) -> Optional[Tuple[List[int], List[str], int, str]]:
    try:
        base_lines     = base_file.read_text(encoding='utf-8').splitlines(keepends=True)
        numbered_lines = numbered_file.read_text(encoding='utf-8').splitlines(keepends=True)
    except Exception as e:
        print(f"Error reading {base_file} or {numbered_file}: {e}", file=sys.stderr)
        return None

    _, base_cls_ln = extract_class_name(base_lines)
    _, num_cls_ln  = extract_class_name(numbered_lines)
    base_feats  = parse_features(base_lines)
    num_feats   = parse_features(numbered_lines)
    base_cls, _ = extract_class_name(base_lines)
    num_cls,  _ = extract_class_name(numbered_lines)

    def _skip_cls_line(lines, cls_ln):
        mapping = {}
        result  = []
        idx = 0
        for i, line in enumerate(lines):
            if i + 1 != cls_ln:
                result.append(line.rstrip('\n\r'))
                mapping[idx] = i + 1
                idx += 1
        return result, mapping

    base_diff, base_map = _skip_cls_line(base_lines, base_cls_ln)
    num_diff,  num_map  = _skip_cls_line(numbered_lines, num_cls_ln)

    matcher = difflib.SequenceMatcher(None, base_diff, num_diff)
    differences = []
    for tag, i1, i2, j1, j2 in matcher.get_opcodes():
        if tag == 'equal':
            continue
        if tag in ('delete', 'replace'):
            for k in range(i1, i2):
                differences.append((base_map[k], base_diff[k], None))
        if tag in ('insert', 'replace'):
            for k in range(j1, j2):
                differences.append((num_map[k], None, num_diff[k]))

    if not differences:
        return None

    first_feat_line = None
    all_starts = [f[1] for f in base_feats] + [f[1] for f in num_feats]
    if all_starts:
        first_feat_line = min(all_starts)

    diffs_by_line: dict = {}
    for ln, bl, nl in differences:
        diffs_by_line.setdefault(ln, []).append((bl, nl))

    filtered = []
    for ln, pairs in diffs_by_line.items():
        if ln in (1, base_cls_ln, num_cls_ln):
            continue
        if first_feat_line and ln < first_feat_line:
            continue
        dels = [bl for bl, nl in pairs if bl is not None and nl is None]
        ins  = [nl for bl, nl in pairs if bl is None and nl is not None]
        mods = [(bl, nl) for bl, nl in pairs if bl is not None and nl is not None]

        only_type = True
        for bl, nl in mods:
            if not _is_only_type_name_change(bl, nl, base_cls, num_cls):
                only_type = False; break
        if only_type and len(dels) == len(ins):
            for bl, nl in zip(dels, ins):
                if not _is_only_type_name_change(bl, nl, base_cls, num_cls):
                    only_type = False; break
        elif dels or ins:
            only_type = False

        if not only_type:
            filtered.extend((ln, bl, nl) for bl, nl in pairs)

    if not filtered:
        return None

    first_ln, first_bl, first_nl = filtered[0]
    if first_nl is None:
        feature_name = find_feature_for_line(first_ln, base_feats)
    else:
        feature_name = find_feature_for_line(first_ln, num_feats)

    if not feature_name:
        raise ValueError(
            f"Cannot identify feature at line {first_ln} in {numbered_file.name}.\n"
            f"Features found: {num_feats}"
        )

    diff_line_nums = [ln for ln, _, nl in filtered if nl is not None]
    return diff_line_nums, numbered_lines, num_cls_ln, feature_name


def prepare_jml(dataset_dir: Path) -> None:
    """JML mode: compare numbered files to base, strip comments, output CLASS.feature."""
    for folder in sorted(dataset_dir.iterdir()):
        if not folder.is_dir() or folder.name.startswith('.'):
            continue

        numbered_files = []
        base_files: dict = {}

        for f in folder.glob("*.e"):
            if "_prepared" in f.name:
                continue
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
                result = _compare_files(base_file, nf)
                if not result:
                    continue
                diff_lns, num_lines, _, feature_name = result
                class_name, _ = extract_class_name(num_lines)
                while class_name.endswith("_PREPARED"):
                    class_name = class_name[:-9]
                modified = _strip_comments(num_lines, diff_lns)
                nf.write_text("".join(modified), encoding='utf-8')
                if class_name:
                    print(f"{class_name}.{feature_name}" if feature_name else class_name)
            except ValueError as e:
                print(f"Error: {e}", file=sys.stderr)
                raise


# ---------------------------------------------------------------------------
# Maple mode — flat directory, no base files
# ---------------------------------------------------------------------------

def prepare_maple(dataset_dir: Path) -> None:
    """Maple mode: output class names from all top-level .e files."""
    for e_file in sorted(dataset_dir.glob("*.e")):
        try:
            lines = e_file.read_text(encoding='utf-8', errors='replace').splitlines(keepends=True)
            class_name, _ = extract_class_name(lines)
            if class_name:
                print(class_name)
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
