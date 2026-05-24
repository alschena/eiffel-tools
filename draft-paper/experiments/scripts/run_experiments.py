#!/usr/bin/env python3
"""
Run fix-feature experiments across all model × ablation combinations.

Ablation axes (class_invariant is NEVER disabled):
  task   --no-task-instruction
  mod    --no-modification-constraints
  pre    --no-precondition-identifiers
  post   --no-postcondition-identifiers
  err    --no-error-message

2^5 = 32 ablation combinations × models × datasets.

Usage:
  ./run_experiments.py
  ./run_experiments.py --models liquid/lfm-2.5-1.2b-instruct:free
  ./run_experiments.py --datasets datasets/maple-recursive --models modelA,modelB
"""

import argparse
import os
import subprocess
import sys
import tempfile
from itertools import product
from pathlib import Path

# ---------------------------------------------------------------------------
# Paths
# ---------------------------------------------------------------------------

SCRIPT_DIR      = Path(__file__).parent.resolve()
EXPERIMENTS_DIR = SCRIPT_DIR.parent
REPO_ROOT       = EXPERIMENTS_DIR.parent.parent   # experiments → draft-paper → eiffel-tools

DEFAULT_MODELS = [
    "liquid/lfm-2.5-1.2b-instruct:free",
    "poolside/laguna-xs.2:free",
]

DEFAULT_DATASETS = [
    EXPERIMENTS_DIR / "datasets" / "maple-recursive",
    EXPERIMENTS_DIR / "datasets" / "buggy-java-jml-eiffel",
]

# ---------------------------------------------------------------------------
# Ablation definitions — class_invariant is never toggled
# ---------------------------------------------------------------------------

ABLATION = [
    ("task",  "--no-task-instruction"),
    ("mod",   "--no-modification-constraints"),
    ("pre",   "--no-precondition-identifiers"),
    ("post",  "--no-postcondition-identifiers"),
    ("err",   "--no-error-message"),
]


def all_combinations():
    """Yield (tag: str, flags: list[str]) for every 2^5 ablation combination."""
    n = len(ABLATION)
    for mask in range(1 << n):
        disabled = [(short, flag) for i, (short, flag) in enumerate(ABLATION) if (mask >> i) & 1]
        tag   = "_".join(f"no_{s}" for s, _ in disabled) or "full"
        flags = [flag for _, flag in disabled]
        yield tag, flags


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def slug(s: str) -> str:
    return s.replace("/", "_").replace(":", "_")


def find_binary() -> Path:
    if env := os.environ.get("EIFFEL_TOOLS_BIN"):
        b = Path(env)
        if not b.is_file():
            sys.exit(f"EIFFEL_TOOLS_BIN points to non-existent file: {b}")
        return b
    b = REPO_ROOT / "target" / "release" / "llm-correct-features"
    if not b.is_file():
        print("Binary not found — building...", flush=True)
        subprocess.run(
            ["cargo", "build", "--release", "-p", "llm-correct-features",
             "--manifest-path", str(REPO_ROOT / "Cargo.toml")],
            check=True,
        )
    return b


def git_restore(dataset: Path) -> None:
    subprocess.run(["git", "restore", "."], cwd=dataset, capture_output=True)


def run_prepare(dataset: Path) -> Path:
    """Run prepare.py, write stdout to a temp file, return its path."""
    fd, path = tempfile.mkstemp(suffix=".txt", prefix="buggy_features_")
    with os.fdopen(fd, "w") as f:
        result = subprocess.run(
            [sys.executable, str(SCRIPT_DIR / "prepare.py"), str(dataset)],
            stdout=f,
            stderr=subprocess.PIPE,
            text=True,
        )
    if result.returncode != 0:
        Path(path).unlink(missing_ok=True)
        raise RuntimeError(f"prepare.py failed:\n{result.stderr}")
    return Path(path)


def run_one(binary: Path, dataset: Path, features_file: Path,
            model: str, flags: list, output: Path) -> int:
    cmd = [
        str(binary),
        "--config",  str(dataset / "Ace.ecf"),
        "--classes", str(features_file),
        "--model",   model,
        "--provider", "openrouter",
        *flags,
    ]
    with open(output, "w") as f:
        subprocess.run(cmd, stdout=f, check=True)
    return sum(1 for _ in open(output))


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def parse_args():
    p = argparse.ArgumentParser(description=__doc__,
                                formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--models",   help="Comma-separated model names (overrides default)")
    p.add_argument("--datasets", help="Comma-separated dataset paths (overrides default)")
    p.add_argument("--dry-run",  action="store_true",
                   help="Print what would run without executing")
    return p.parse_args()


def main():
    args = parse_args()

    models = [m.strip() for m in args.models.split(",")] if args.models \
        else DEFAULT_MODELS

    datasets = [Path(d.strip()) for d in args.datasets.split(",")] if args.datasets \
        else DEFAULT_DATASETS

    combos = list(all_combinations())
    binary = None if args.dry_run else find_binary()

    runs = [(ds, m, tag, flags)
            for ds in datasets
            for m in models
            for tag, flags in combos]

    print(f"{len(runs)} runs: {len(datasets)} dataset(s) × {len(models)} model(s) "
          f"× {len(combos)} ablation(s)", flush=True)

    errors = []

    for i, (dataset, model, tag, flags) in enumerate(runs, 1):
        dataset_name = dataset.name
        output_dir   = EXPERIMENTS_DIR / "results" / dataset_name / slug(model)
        output       = output_dir / f"{tag}.jsonl"

        prefix = f"[{i}/{len(runs)}] {dataset_name}  {model}  {tag}"

        if args.dry_run:
            print(f"{prefix}  →  {output.relative_to(EXPERIMENTS_DIR)}", flush=True)
            continue

        if not dataset.is_dir():
            print(f"{prefix}  [SKIP] dataset not found", flush=True)
            continue

        print(prefix, flush=True)
        output_dir.mkdir(parents=True, exist_ok=True)

        git_restore(dataset)

        try:
            features_file = run_prepare(dataset)
        except RuntimeError as e:
            print(f"  ERROR in prepare: {e}", file=sys.stderr, flush=True)
            errors.append((i, str(e)))
            continue

        n_features = sum(1 for _ in open(features_file))
        print(f"  features: {n_features}", flush=True)

        try:
            n_records = run_one(binary, dataset, features_file, model, flags, output)
            rel = output.relative_to(EXPERIMENTS_DIR)
            print(f"  → {rel}  ({n_records} records)", flush=True)
        except subprocess.CalledProcessError as e:
            print(f"  ERROR: {e}", file=sys.stderr, flush=True)
            errors.append((i, str(e)))
        finally:
            features_file.unlink(missing_ok=True)

    print(flush=True)
    if errors:
        print(f"{len(errors)} run(s) failed:", file=sys.stderr)
        for idx, msg in errors:
            print(f"  run {idx}: {msg}", file=sys.stderr)
        sys.exit(1)
    else:
        print(f"Done. Results in {EXPERIMENTS_DIR / 'results'}", flush=True)


if __name__ == "__main__":
    main()
