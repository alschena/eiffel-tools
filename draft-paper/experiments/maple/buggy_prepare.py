#!/usr/bin/env python3
"""
Script to compare numbered Eiffel files with their base files and identify
differences in features (excluding class name differences).
"""

import os
import re
import difflib
from pathlib import Path
from typing import List, Tuple, Optional


def extract_class_name(lines: List[str]) -> Tuple[str, int]:
    """
    Extract class name and its line number from the file.
    Returns (class_name, line_number) where line_number is 1-indexed.
    The class name is the line that comes after the 'class' keyword.
    Handles BOM characters and empty lines between 'class' and class name.
    """
    for i, line in enumerate(lines):
        # Strip BOM and whitespace
        stripped = line.strip().lstrip('\ufeff').strip()
        if stripped == "class":
            # Class name is on the next non-empty line
            for j in range(i + 1, len(lines)):
                class_name_line = lines[j].strip().lstrip('\ufeff').strip()
                if class_name_line:  # Skip empty lines
                    return (class_name_line, j + 1)  # j+1 because line numbers are 1-indexed
    return ("", 0)


def parse_features(lines: List[str]) -> List[Tuple[str, int, int]]:
    """
    Parse features from Eiffel file.
    Returns list of (feature_name, start_line, end_line) tuples.
    Line numbers are 1-indexed.
    """
    features = []
    in_feature_block = False
    current_feature = None
    feature_start = None
    
    for i, line in enumerate(lines, start=1):
        stripped = line.strip().lstrip('\ufeff').strip()
        
        # Check if we're entering the feature section
        if stripped == "feature":
            in_feature_block = True
            continue
        
        # If we're in the feature block, look for feature declarations
        if in_feature_block:
            # Feature declarations start with a single tab and contain a feature name
            # Pattern: \tfeature_name (params): return_type or \tfeature_name: return_type
            if line.startswith("\t") and not line.startswith("\t\t"):
                # Check if this looks like a feature declaration (not "end" or empty)
                if stripped and stripped != "end":
                    # Extract feature name (first word before parentheses or colon)
                    match = re.match(r'^\t([a-zA-Z_][a-zA-Z0-9_]*)', line)
                    if match:
                        # If we were in a previous feature, close it
                        if current_feature and feature_start:
                            features.append((current_feature, feature_start, i - 1))
                        
                        # Start new feature
                        current_feature = match.group(1)
                        feature_start = i
            
            # Check if we're ending a feature (end at single tab level)
            elif line.startswith("\t") and stripped == "end" and not line.startswith("\t\t"):
                if current_feature and feature_start:
                    features.append((current_feature, feature_start, i))
                    current_feature = None
                    feature_start = None
    
    # If we ended while still in a feature, close it at the end
    if current_feature and feature_start:
        features.append((current_feature, feature_start, len(lines)))
    
    return features


def find_feature_for_line(line_num: int, features: List[Tuple[str, int, int]]) -> Optional[str]:
    """Find which feature contains the given line number."""
    for feature_name, start, end in features:
        if start <= line_num <= end:
            return feature_name
    return None


def remove_comment(line: str) -> str:
    """Remove Eiffel comment (-- and everything after) from a line."""
    if "--" in line:
        # Find the position of -- that's not part of a string
        # Simple approach: find first -- and remove everything after it
        comment_pos = line.find("--")
        return line[:comment_pos].rstrip()
    return line


def remove_comments_from_lines(numbered_lines: List[str], diff_line_nums: List[int]) -> List[str]:
    """
    Remove comments from differing lines in the file.
    Returns modified lines with comments removed.
    """
    modified_lines = numbered_lines.copy()
    
    for diff_line_num in diff_line_nums:
        if diff_line_num > 0 and diff_line_num <= len(modified_lines):
            line_idx = diff_line_num - 1
            original_line = modified_lines[line_idx]
            # Preserve the newline character
            has_newline = original_line.endswith('\n')
            line_without_newline = original_line.rstrip('\n\r')
            line_without_comment = remove_comment(line_without_newline)
            if has_newline:
                modified_lines[line_idx] = line_without_comment + '\n'
            else:
                modified_lines[line_idx] = line_without_comment
    
    return modified_lines


def compare_files(base_file: Path, numbered_file: Path) -> Optional[Tuple[List[int], List[str], int, str]]:
    """
    Compare two files and return (diff_line_nums, numbered_lines, class_name_line_num, feature_name) if there's a difference
    other than the class name. Returns None if only class name differs or files are identical.
    """
    try:
        with open(base_file, 'r', encoding='utf-8') as f:
            base_lines = f.readlines()
        with open(numbered_file, 'r', encoding='utf-8') as f:
            numbered_lines = f.readlines()
    except Exception as e:
        print(f"Error reading files {base_file} or {numbered_file}: {e}", file=os.sys.stderr)
        return None
    
    # Find class name line numbers in both files
    _, base_class_line_num = extract_class_name(base_lines)
    _, numbered_class_line_num = extract_class_name(numbered_lines)
    
    # Parse features from both files to find which feature contains the diff
    base_features = parse_features(base_lines)
    numbered_features = parse_features(numbered_lines)
    
    # Use SequenceMatcher to find differences, but skip class name lines
    # Create versions without class name line for comparison
    base_lines_for_diff = []
    numbered_lines_for_diff = []
    
    # Build line number maps (accounting for skipped class name line)
    base_line_map = {}  # Map from diff index to original line number
    numbered_line_map = {}  # Map from diff index to original line number
    
    base_orig_idx = 0
    for i, line in enumerate(base_lines):
        if i + 1 != base_class_line_num:  # Skip class name line (1-indexed)
            base_lines_for_diff.append(line.rstrip('\n\r'))
            base_line_map[base_orig_idx] = i + 1  # 1-indexed
            base_orig_idx += 1
    
    numbered_orig_idx = 0
    for i, line in enumerate(numbered_lines):
        if i + 1 != numbered_class_line_num:  # Skip class name line (1-indexed)
            numbered_lines_for_diff.append(line.rstrip('\n\r'))
            numbered_line_map[numbered_orig_idx] = i + 1  # 1-indexed
            numbered_orig_idx += 1
    
    # Use SequenceMatcher to find opcodes (operations)
    matcher = difflib.SequenceMatcher(None, base_lines_for_diff, numbered_lines_for_diff)
    differences = []
    
    for tag, i1, i2, j1, j2 in matcher.get_opcodes():
        if tag == 'equal':
            continue
        elif tag == 'delete':
            # Lines deleted from base (not in numbered)
            for idx in range(i1, i2):
                base_line = base_lines_for_diff[idx]
                base_line_num = base_line_map[idx]
                differences.append((base_line_num, base_line, None))
        elif tag == 'insert':
            # Lines inserted in numbered (not in base)
            for idx in range(j1, j2):
                numbered_line = numbered_lines_for_diff[idx]
                numbered_line_num = numbered_line_map[idx]
                differences.append((numbered_line_num, None, numbered_line))
        elif tag == 'replace':
            # Lines replaced (different in both)
            # Add all base lines as deletions
            for idx in range(i1, i2):
                base_line = base_lines_for_diff[idx]
                base_line_num = base_line_map[idx]
                differences.append((base_line_num, base_line, None))
            # Add all numbered lines as insertions
            for idx in range(j1, j2):
                numbered_line = numbered_lines_for_diff[idx]
                numbered_line_num = numbered_line_map[idx]
                differences.append((numbered_line_num, None, numbered_line))
    
    # If no differences (other than class name), return None
    # We already skipped line 2, so if there are no other differences, 
    # the files only differ in class name
    if not differences:
        return None
    
    # Group consecutive differences together (multiline diffs)
    # Also group differences on the same line (replace operations)
    diff_groups = []
    current_group = []
    
    for i, (line_num, base_line, numbered_line) in enumerate(differences):
        if not current_group:
            current_group = [(line_num, base_line, numbered_line)]
        else:
            prev_line_num = current_group[-1][0]
            # If this difference is on the same line or consecutive to the previous one, add to current group
            if line_num == prev_line_num or line_num == prev_line_num + 1:
                current_group.append((line_num, base_line, numbered_line))
            else:
                # Start a new group
                diff_groups.append(current_group)
                current_group = [(line_num, base_line, numbered_line)]
    
    # Add the last group
    if current_group:
        diff_groups.append(current_group)
    
    # Collect all line numbers from all groups (comments are not required)
    # We still remove comments from lines that have them
    all_diff_line_nums = []
    for group in diff_groups:
        # Add all line numbers from this group
        for line_num, base_line, numbered_line in group:
            # For insertions and modifications, use numbered file line number
            # For deletions, we'll still track the line number for finding the feature
            if numbered_line is not None:
                all_diff_line_nums.append(line_num)
    
    # Find the feature that contains the first difference
    # For deletions, use base file line number and base features
    # For insertions/modifications, use numbered file line number and numbered features
    feature_name = None
    if differences:
        first_diff = differences[0]
        first_line_num, first_base_line, first_numbered_line = first_diff
        
        if first_numbered_line is None:
            # Deletion - find feature in base file
            feature_name = find_feature_for_line(first_line_num, base_features)
        else:
            # Insertion or modification - find feature in numbered file
            feature_name = find_feature_for_line(first_line_num, numbered_features)
        
        if not feature_name:
            feature_name = "unknown"
    
    # Return all difference line numbers, the numbered file lines, the class name line number, and feature name
    return (all_diff_line_nums, numbered_lines, numbered_class_line_num, feature_name)


def main():
    base_dir = Path(__file__).parent / "buggy-java-jml-eiffel"
    
    if not base_dir.exists():
        print(f"Directory {base_dir} does not exist", file=os.sys.stderr)
        return
    
    # Iterate through all folders in buggy-java-jml-eiffel
    for folder in sorted(base_dir.iterdir()):
        if not folder.is_dir() or folder.name.startswith('.'):
            continue
        
        # Find all .e files with numbers in their names
        # Exclude prepared files (files with _prepared in the name)
        numbered_files = []
        base_file = None
        
        for file in folder.glob("*.e"):
            filename = file.name
            # Skip prepared files
            if "_prepared" in filename:
                continue
            # Check if filename contains a number
            if re.search(r'_\d+\.e$', filename):
                numbered_files.append(file)
            elif not re.search(r'_\d+', filename):
                # This is likely the base file
                base_file = file
        
        if not base_file:
            continue
        
        # Compare each numbered file with the base file
        for numbered_file in sorted(numbered_files):
            try:
                result = compare_files(base_file, numbered_file)
                if result:
                    diff_line_nums, numbered_lines, class_name_line_num, feature_name = result
                    
                    # Extract the class name and remove any _PREPARED suffix to get original
                    class_name_with_suffix, _ = extract_class_name(numbered_lines)
                    # Remove _PREPARED suffix if present (handle multiple _PREPARED)
                    original_class_name = class_name_with_suffix
                    while original_class_name.endswith("_PREPARED"):
                        original_class_name = original_class_name[:-9]  # Remove "_PREPARED"
                    
                    # Modify file in place: remove comments only (don't change class name)
                    modified_lines = remove_comments_from_lines(numbered_lines, diff_line_nums)
                    
                    # Write modified content back to the original file
                    with open(numbered_file, 'w', encoding='utf-8') as f:
                        f.writelines(modified_lines)
                    
                    # Print class.feature format (using original class name)
                    if original_class_name:
                        # Print in format: CLASS_NAME.feature_name
                        if feature_name:
                            print(f"{original_class_name}.{feature_name}")
                        else:
                            print(original_class_name)
            except ValueError as e:
                print(f"Error: {e}", file=os.sys.stderr)
                raise


def test_alphabet_3():
    """Test that alphabet_3.e correctly identifies is_alphabetic as the feature."""
    base_file = Path(__file__).parent / "buggy-java-jml-eiffel" / "alphabet" / "alphabet.e"
    numbered_file = Path(__file__).parent / "buggy-java-jml-eiffel" / "alphabet" / "alphabet_3.e"
    
    if not base_file.exists() or not numbered_file.exists():
        print("Test files not found, skipping test")
        return True
    
    result = compare_files(base_file, numbered_file)
    if not result:
        print("ERROR: alphabet_3 test failed - no differences found")
        return False
    
    diff_line_nums, numbered_lines, class_name_line_num, feature_name = result
    
    if feature_name != "is_alphabetic":
        print(f"ERROR: alphabet_3 test failed - expected feature 'is_alphabetic', got '{feature_name}'")
        return False
    
    print("PASS: alphabet_3 test - correctly identified feature 'is_alphabetic'")
    return True


def run_tests():
    """Run all tests."""
    print("Running test suite...")
    tests_passed = 0
    tests_failed = 0
    
    # Test alphabet_3
    if test_alphabet_3():
        tests_passed += 1
    else:
        tests_failed += 1
    
    print(f"\nTest results: {tests_passed} passed, {tests_failed} failed")
    return tests_failed == 0


if __name__ == "__main__":
    import sys
    if len(sys.argv) > 1 and sys.argv[1] == "--test":
        success = run_tests()
        sys.exit(0 if success else 1)
    else:
        main()

