#!/usr/bin/env -S python3 -u
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
import datetime
import json
import os
import shutil
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
    "mistralai/codestral-2508",
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
    ("sig",   "--no-verbatim-signature"),
    ("syn",   "--no-syntax-guide"),
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


LOCK_FILE = ".experiment.lock"


def acquire_lock(dataset: Path) -> Path:
    lock = dataset / LOCK_FILE
    try:
        fd = os.open(str(lock), os.O_CREAT | os.O_EXCL | os.O_WRONLY)
        with os.fdopen(fd, "w") as f:
            f.write(f"pid={os.getpid()}\nstarted={datetime.datetime.now().isoformat()}\n")
        return lock
    except FileExistsError:
        try:
            info = lock.read_text().strip()
        except Exception:
            info = "(unreadable)"
        raise RuntimeError(f"Dataset locked by another process:\n  {info}\n  lock: {lock}")


def release_lock(lock: Path) -> None:
    lock.unlink(missing_ok=True)


def git_reset(dataset: Path) -> None:
    subprocess.run(["git", "reset", "--hard"], cwd=dataset, capture_output=True)


def setup_dataset(dataset: Path) -> None:
    """One-time setup per dataset: reset files, wipe EIFGENs, dry-run verification."""
    print(f"Setting up {dataset.name}...", flush=True)
    git_reset(dataset)
    shutil.rmtree(dataset / "EIFGENs", ignore_errors=True)
    print(f"  EIFGENs removed", flush=True)

    ap_cmd = os.environ.get("AP_COMMAND")
    if not ap_cmd:
        print(f"  WARNING: AP_COMMAND not set, skipping dry-run", flush=True)
        return

    # Pick first .e file to get a class name for the dry run
    e_files = sorted(dataset.glob("*.e")) or sorted(dataset.rglob("*.e"))
    if not e_files:
        print(f"  WARNING: no .e files found, skipping dry-run", flush=True)
        return
    first_class = e_files[0].stem.upper()

    def _run_ap(label: str) -> str:
        cmd = [ap_cmd, "-batch", "-autoproof", first_class]
        print(f"  [{label}] $ {' '.join(cmd)}  (cwd={dataset})", flush=True)
        proc = subprocess.Popen(cmd, cwd=dataset, stdout=subprocess.PIPE,
                                stderr=subprocess.STDOUT, text=True)
        lines = []
        for line in proc.stdout:
            line = line.rstrip("\n")
            print(f"  [{label}] {line}", flush=True)
            lines.append(line)
        proc.wait()
        return "\n".join(lines)

    output = _run_ap("dry-run")
    if any("VD01" in l for l in output.splitlines() if "Error code:" in l):
        # VD01 is a race condition artifact; the first compile after EIFGENs removal always
        # emits it but still builds EIFGENs. Run again — the second pass succeeds cleanly.
        print(f"  VD01 detected — recompiling from scratch (second pass)", flush=True)
        output = _run_ap("rebuild")

    config_errors = [l for l in output.splitlines()
                     if "Error code:" in l and any(c in l for c in ("VD01", "VD83", "VD21"))]
    if config_errors:
        for e in config_errors:
            print(f"  ERROR: {e.strip()}", file=sys.stderr, flush=True)
        raise RuntimeError(f"AutoProof dry-run failed for {dataset.name}: {config_errors}")
    print(f"  dry-run OK ({first_class})", flush=True)


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
            model: str, flags: list, output: Path, total_features: int = 0) -> int:
    import json as _json
    cmd = [
        str(binary),
        "--config",  "Ace.ecf",
        "--classes", str(features_file),
        "--model",   model,
        "--provider", "openrouter",
        *flags,
    ]
    n = 0
    with open(output, "w") as f:
        proc = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                text=True, cwd=dataset)
        for line in proc.stdout:
            # Progress lines (prefixed >>) go to stdout only, not the JSONL file
            if line.startswith(">>"):
                print(f"  [{n + 1}/{total_features}] {line.strip()[3:]}", flush=True)
                continue
            f.write(line)
            f.flush()
            try:
                r = _json.loads(line)
                n += 1
                status = "OK  " if r.get("success") else "FAIL"
                attempts = r.get("llm_interactions", "?")
                elapsed = r.get("total_elapsed_time_seconds", 0)
                print(f"    {status} {r['class_name']}.{r['feature_name']}  ({elapsed:.1f}s)", flush=True)
                for ix in r.get("interactions", []):
                    ix_num = ix.get("interaction_number", "?")
                    applied = ix.get("applied", False)
                    prompt = ix.get("prompt")
                    msg = ix.get("llm_message")
                    if prompt:
                        print(f"      --- attempt {ix_num} prompt ---", flush=True)
                        print(prompt, flush=True)
                    if msg:
                        result_str = "APPLIED" if applied else "rejected"
                        print(f"      --- attempt {ix_num} response [{result_str}] ---", flush=True)
                        print(msg, flush=True)
                print(f"    --> {attempts} attempt(s), {status.strip()}", flush=True)
            except Exception:
                pass
        proc.wait()
        if proc.returncode != 0:
            raise subprocess.CalledProcessError(proc.returncode, cmd)
    return n


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def write_progress(results_dir: Path, run_num: int, n_runs: int,
                   started_at: datetime.datetime, last_run: str = "") -> None:
    progress = {
        "started_at":       started_at.isoformat(),
        "updated_at":       datetime.datetime.now().isoformat(),
        "completed_runs":   run_num,
        "total_runs":       n_runs,
        "last_run":         last_run,
    }
    (results_dir / "progress.json").write_text(json.dumps(progress, indent=2))


def purge_results(results_dir: Path) -> None:
    removed = 0
    for jf in results_dir.rglob("*.jsonl"):
        jf.unlink()
        removed += 1
    if removed:
        print(f"Purged {removed} JSONL file(s) from {results_dir}", flush=True)


def render_html(results_dir: Path) -> None:
    print("Generating HTML interaction viewer…", flush=True)
    result = subprocess.run(
        [sys.executable, str(SCRIPT_DIR / "render_interactions.py"),
         "--results", str(results_dir)],
        capture_output=True, text=True,
    )
    if result.returncode == 0:
        print(result.stdout.strip(), flush=True)
    else:
        print(f"  WARNING: render_interactions.py failed:\n{result.stderr}", flush=True)


def parse_args():
    p = argparse.ArgumentParser(description=__doc__,
                                formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--models",    help="Comma-separated model names (overrides default)")
    p.add_argument("--datasets",  help="Comma-separated dataset paths (overrides default)")
    p.add_argument("--ablations", help="Comma-separated ablation tags to run (e.g. full,no_err)")
    p.add_argument("--limit-features", type=int, metavar="N",
                   help="Run only the first N features per dataset (for quick tests)")
    p.add_argument("--no-purge",  action="store_true",
                   help="Skip purging old JSONL results before running")
    p.add_argument("--dry-run",   action="store_true",
                   help="Print what would run without executing")
    return p.parse_args()


def main():
    args = parse_args()

    models = [m.strip() for m in args.models.split(",")] if args.models \
        else DEFAULT_MODELS

    datasets = [Path(d.strip()) for d in args.datasets.split(",")] if args.datasets \
        else DEFAULT_DATASETS

    combos = list(all_combinations())
    if args.ablations:
        wanted = {t.strip() for t in args.ablations.split(",")}
        combos = [(tag, flags) for tag, flags in combos if tag in wanted]
        if not combos:
            sys.exit(f"No ablations matched: {args.ablations}")
    binary = None if args.dry_run else find_binary()

    n_runs = len(datasets) * len(models) * len(combos)
    print(f"{n_runs} runs: {len(datasets)} dataset(s) × {len(models)} model(s) "
          f"× {len(combos)} ablation(s)", flush=True)

    if args.dry_run:
        run_num = 0
        for dataset in datasets:
            for model in models:
                for tag, _ in combos:
                    run_num += 1
                    output = EXPERIMENTS_DIR / "results" / dataset.name / slug(model) / f"{tag}.jsonl"
                    print(f"[{run_num}/{n_runs}] {dataset.name}  {model}  {tag}"
                          f"  →  {output.relative_to(EXPERIMENTS_DIR)}", flush=True)
        return

    results_dir = EXPERIMENTS_DIR / "results"
    if not args.no_purge:
        purge_results(results_dir)

    errors = []
    run_num = 0
    started_at = datetime.datetime.now()
    results_dir.mkdir(parents=True, exist_ok=True)
    write_progress(results_dir, 0, n_runs, started_at)

    for dataset in datasets:
        if not dataset.is_dir():
            print(f"[SKIP] dataset not found: {dataset}", flush=True)
            run_num += len(models) * len(combos)
            continue

        # --- one-time setup per dataset ---
        try:
            setup_dataset(dataset)
        except RuntimeError as e:
            print(f"ERROR: dataset setup failed for {dataset.name}: {e}", file=sys.stderr, flush=True)
            errors.append((dataset.name, str(e)))
            run_num += len(models) * len(combos)
            continue

        try:
            lock = acquire_lock(dataset)
        except RuntimeError as e:
            print(f"ERROR: {e}", file=sys.stderr, flush=True)
            errors.append((dataset.name, str(e)))
            run_num += len(models) * len(combos)
            continue

        features_file = None
        try:
            try:
                features_file = run_prepare(dataset)
            except RuntimeError as e:
                print(f"ERROR in prepare for {dataset.name}: {e}", file=sys.stderr, flush=True)
                errors.append((dataset.name, str(e)))
                continue

            all_lines = Path(features_file).read_text().splitlines(keepends=True)
            if args.limit_features and args.limit_features < len(all_lines):
                limited = features_file.parent / (features_file.stem + "_limited.txt")
                limited.write_text("".join(all_lines[:args.limit_features]))
                features_file.unlink()
                features_file = limited
            n_features = sum(1 for _ in open(features_file))
            print(f"  {dataset.name}: {n_features} feature(s) to fix", flush=True)

            # --- per (model × ablation) run ---
            for model in models:
                for tag, flags in combos:
                    run_num += 1
                    output_dir = EXPERIMENTS_DIR / "results" / dataset.name / slug(model)
                    output     = output_dir / f"{tag}.jsonl"
                    prefix     = f"[{run_num}/{n_runs}] {dataset.name}  {model}  {tag}"
                    print(prefix, flush=True)
                    output_dir.mkdir(parents=True, exist_ok=True)

                    git_reset(dataset)

                    try:
                        n_records = run_one(binary, dataset, features_file, model, flags,
                                            output, total_features=n_features)
                        rel = output.relative_to(EXPERIMENTS_DIR)
                        print(f"  → {rel}  ({n_records} records)", flush=True)
                        write_progress(results_dir, run_num, n_runs, started_at,
                                       last_run=f"{dataset.name}  {model}  {tag}")
                        render_html(results_dir)
                    except subprocess.CalledProcessError as e:
                        print(f"  ERROR: {e}", file=sys.stderr, flush=True)
                        errors.append((f"run {run_num}", str(e)))

        finally:
            if features_file:
                features_file.unlink(missing_ok=True)
            release_lock(lock)

    print(flush=True)
    render_html(results_dir)

    if errors:
        print(f"{len(errors)} failure(s):", file=sys.stderr)
        for label, msg in errors:
            print(f"  {label}: {msg}", file=sys.stderr)
        sys.exit(1)
    else:
        print(f"Done. Results in {results_dir}", flush=True)
        print(f"      HTML viewer: {results_dir / 'html' / 'index.html'}", flush=True)


if __name__ == "__main__":
    main()
