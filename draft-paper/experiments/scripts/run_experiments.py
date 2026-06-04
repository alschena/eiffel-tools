#!/usr/bin/env -S python3 -u
"""
Run fix-feature experiments across all model × ablation combinations.

Always resumes: already-completed (class, feature) pairs are skipped.
To start fresh, delete the results/ directory.

Usage:
  ./run_experiments.py
  ./run_experiments.py --models liquid/lfm-2.5-1.2b-instruct:free
  ./run_experiments.py --datasets datasets/maple-recursive --models modelA,modelB
"""

import argparse
import datetime
import errno
import json
import os
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import threading
from itertools import product
from pathlib import Path

# ---------------------------------------------------------------------------
# Graceful shutdown
# ---------------------------------------------------------------------------

_shutdown_requested = False

Z3_WATCHDOG_INTERVAL = 5  # seconds


def _request_shutdown(signum, frame):
    global _shutdown_requested
    if not _shutdown_requested:
        print("\nShutdown requested — finishing current run, then stopping.", flush=True)
    _shutdown_requested = True


def _read_ppid(pid: int) -> int | None:
    """Return the PPID of pid by reading /proc, or None if the process is gone."""
    try:
        with open(f"/proc/{pid}/status") as f:
            for line in f:
                if line.startswith("PPid:"):
                    return int(line.split()[1])
    except (FileNotFoundError, OSError, ValueError):
        pass
    return None


def _z3_watchdog():
    """Kill z3 instances that are orphaned: either z3 itself or its parent (boogie)
    has been re-parented to init (PPID == 1), meaning the owning ecb has exited."""
    while not _shutdown_requested:
        try:
            result = subprocess.run(
                ["pgrep", "-f", r"z3.*-smt2.*-in"],
                capture_output=True, text=True,
            )
            pids = [int(p) for p in result.stdout.split() if p.strip()]
            for pid in pids:
                ppid = _read_ppid(pid)
                if ppid is None:
                    continue
                # z3 directly orphaned, or its parent (boogie) is orphaned
                parent_ppid = _read_ppid(ppid) if ppid != 1 else None
                if ppid == 1 or parent_ppid == 1:
                    try:
                        os.kill(pid, signal.SIGKILL)
                        print(f"[watchdog] killed orphaned z3 pid={pid}", flush=True)
                    except (ProcessLookupError, OSError):
                        pass
        except Exception:
            pass
        time.sleep(Z3_WATCHDOG_INTERVAL)

# ---------------------------------------------------------------------------
# Solvable-feature filter
# ---------------------------------------------------------------------------

def _base_class(class_name: str) -> str:
    """Strip trailing _N suffix: COMBINATION_PERMUTATION_5 → COMBINATION_PERMUTATION."""
    return re.sub(r"_\d+$", "", class_name)


def load_solvable_filter(dataset: Path) -> "set[str] | None":
    """
    Load experiments/solvable/<dataset_name>.json and return the set of
    'BASE_CLASS.feature' keys that are verified. Returns None if the file
    doesn't exist (no filtering). Run check_solvable.py to generate this file.
    """
    path = EXPERIMENTS_DIR / "solvable" / f"{dataset.name}.json"
    if not path.exists():
        return None
    with open(path) as f:
        data = json.load(f)
    solvable = {k for k, v in data.get("results", {}).items() if v == "verified"}
    print(f"  Loaded solvable filter: {len(solvable)} verified features "
          f"(generated {data.get('generated_at', '?')[:10]})", flush=True)
    return solvable


def apply_solvable_filter(
    all_features: list, solvable: "set[str] | None"
) -> list:
    """Filter (class_name, feature_name) pairs to only solvable ones."""
    if solvable is None:
        return all_features
    filtered = [
        (cls, feat) for cls, feat in all_features
        if f"{_base_class(cls)}.{feat}" in solvable
    ]
    n_dropped = len(all_features) - len(filtered)
    if n_dropped:
        print(f"  Skipping {n_dropped} unsolvable feature(s) "
              f"(correct reference doesn't verify)", flush=True)
    return filtered


# ---------------------------------------------------------------------------
# Paths
# ---------------------------------------------------------------------------

SCRIPT_DIR      = Path(__file__).parent.resolve()
EXPERIMENTS_DIR = SCRIPT_DIR.parent
REPO_ROOT       = EXPERIMENTS_DIR.parent.parent   # experiments → draft-paper → eiffel-tools

DEFAULT_MODELS = [
    "openai/gpt-5-nano",
    "anthropic/claude-sonnet-4.6",
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
    ("task",    ["--no-task-instruction"]),
    ("prepost", ["--no-precondition-identifiers", "--no-postcondition-identifiers"]),
    ("err",     ["--no-error-message"]),
    ("sig",     ["--no-verbatim-signature"]),
    ("syn",     ["--no-syntax-guide"]),
    ("fsig",    ["--no-full-sig"]),
]

# Default: full prompt and minimal prompt only.
DEFAULT_ABLATIONS = [
    ("full",    []),
    ("minimal", [f for _, flist in ABLATION for f in flist]),
]

# Explicit (model, ablation_tag) execution order when using defaults.
# Unlisted pairs are appended in natural models × ablations product order.
DEFAULT_RUN_ORDER = [
    ("anthropic/claude-sonnet-4.6", "full"),
    ("poolside/laguna-xs.2:free",   "full"),
    ("openai/gpt-5-nano",           "full"),
    ("openai/gpt-5-nano",           "minimal"),
    ("anthropic/claude-sonnet-4.6", "minimal"),
    ("poolside/laguna-xs.2:free",   "minimal"),
]


def all_combinations():
    """Yield (tag, flags) for every 2^n ablation combination, ordered by number of disabled parts."""
    n = len(ABLATION)
    masks = sorted(range(1 << n), key=lambda m: bin(m).count('1'))
    for mask in masks:
        disabled = [(short, flags) for i, (short, flags) in enumerate(ABLATION) if (mask >> i) & 1]
        tag   = "_".join(f"no_{s}" for s, _ in disabled) or "full"
        flags = [f for _, flist in disabled for f in flist]
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
        # Check whether the owning PID is still alive before failing
        try:
            info = lock.read_text().strip()
        except Exception:
            info = "(unreadable)"
        pid = None
        for line in info.splitlines():
            if line.startswith("pid="):
                try:
                    pid = int(line.split("=", 1)[1])
                except ValueError:
                    pass
        if pid is not None:
            try:
                os.kill(pid, 0)
                # EPERM means process exists but we can't signal it — still alive
            except OSError as exc:
                if exc.errno == errno.EPERM:
                    pass  # alive
                else:
                    # ESRCH: process gone — stale lock
                    print(f"  Removing stale lock (pid {pid} no longer running): {lock}",
                          flush=True)
                    lock.unlink(missing_ok=True)
                    return acquire_lock(dataset)
        raise RuntimeError(f"Dataset locked by another process:\n  {info}\n  lock: {lock}")


def release_lock(lock: Path) -> None:
    lock.unlink(missing_ok=True)


def git_reset(dataset: Path) -> None:
    subprocess.run(["git", "reset", "--hard"], cwd=dataset, capture_output=True)


def setup_dataset(dataset: Path) -> None:
    """One-time setup per dataset: reset files, wipe EIFGENs, dry-run verification."""
    print(f"Setting up {dataset.name}...", flush=True)
    git_reset(dataset)

    ap_cmd = os.environ.get("AP_COMMAND")
    if not ap_cmd:
        shutil.rmtree(dataset / "EIFGENs", ignore_errors=True)
        print(f"  WARNING: AP_COMMAND not set, skipping dry-run", flush=True)
        return

    # Pick first .e file to get a class name for the dry run
    e_files = sorted(dataset.glob("*.e")) or sorted(dataset.rglob("*.e"))
    if not e_files:
        shutil.rmtree(dataset / "EIFGENs", ignore_errors=True)
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

    def _has_config_errors(output: str) -> list:
        return [l for l in output.splitlines()
                if "Error code:" in l and any(c in l for c in ("VD01", "VD83", "VD21"))]

    # Try dry-run with existing EIFGENs first (avoids cold-build failures on some datasets).
    eifgens = dataset / "EIFGENs"
    if eifgens.exists():
        output = _run_ap("dry-run")
        if not _has_config_errors(output):
            print(f"  dry-run OK ({first_class})", flush=True)
            return
        print(f"  dry-run with existing EIFGENs failed — wiping and rebuilding", flush=True)

    shutil.rmtree(eifgens, ignore_errors=True)
    print(f"  EIFGENs removed", flush=True)
    output = _run_ap("dry-run")
    if any("VD01" in l for l in output.splitlines() if "Error code:" in l):
        # VD01 is a race condition artifact; the first compile after EIFGENs removal always
        # emits it but still builds EIFGENs. Run again — the second pass succeeds cleanly.
        print(f"  VD01 detected — recompiling from scratch (second pass)", flush=True)
        output = _run_ap("rebuild")

    config_errors = _has_config_errors(output)
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
            model: str, flags: list, output: Path,
            total_features: int = 0, append: bool = False) -> int:
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
    with open(output, "a" if append else "w") as f:
        proc = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                text=True, cwd=dataset, start_new_session=True)
        pgid = os.getpgid(proc.pid)
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
        # Kill any z3 children ecb left running (ecb exits without cleaning up z3 on timeout)
        try:
            os.killpg(pgid, signal.SIGKILL)
        except (ProcessLookupError, OSError):
            pass
        if proc.returncode != 0:
            raise subprocess.CalledProcessError(proc.returncode, cmd)
    return n


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

RATE_LIMIT_RETRY_WAIT = 60  # seconds to wait before retrying a previously rate-limited run


def completed_features_in_jsonl(path: Path) -> set:
    """Return set of (class_name, feature_name) already recorded in a JSONL file."""
    done = set()
    if not path.exists():
        return done
    try:
        for line in path.read_text().splitlines():
            s = line.strip()
            if s and not s.startswith(">>"):
                try:
                    rec = json.loads(s)
                    cn = rec.get("class_name", "")
                    fn = rec.get("feature_name", "")
                    # rate_limited records are not considered done — they must be re-run
                    if cn and fn and not rec.get("rate_limited", False):
                        done.add((cn, fn))
                except json.JSONDecodeError:
                    pass
    except OSError:
        pass
    return done


def rate_limited_timestamps(path: Path) -> dict:
    """Return {(class_name, feature_name): completed_at} for entries that were rate-limited
    and have not since succeeded."""
    limited = {}
    succeeded = set()
    if not path.exists():
        return {}
    try:
        for line in path.read_text().splitlines():
            s = line.strip()
            if not s or s.startswith(">>"):
                continue
            try:
                rec = json.loads(s)
                cn = rec.get("class_name", "")
                fn = rec.get("feature_name", "")
                if not cn or not fn:
                    continue
                if rec.get("rate_limited", False):
                    limited[(cn, fn)] = rec.get("completed_at", 0)
                elif not rec.get("rate_limited", False):
                    succeeded.add((cn, fn))
            except json.JSONDecodeError:
                pass
    except OSError:
        pass
    return {k: v for k, v in limited.items() if k not in succeeded}


def parse_features_file(path: Path) -> list:
    """Parse CLASS.feature lines; return list of (class_name, feature_name) tuples."""
    result = []
    for line in path.read_text().splitlines():
        s = line.strip()
        if not s:
            continue
        if "." in s:
            cls, feat = s.split(".", 1)
            result.append((cls.upper(), feat))
        else:
            result.append((s.upper(), None))
    return result


def write_meta(results_dir: Path, total_runs: int) -> None:
    """Write experiment_meta.json with total_runs only; started_at is derived from JSONL mtimes."""
    mf = results_dir / "experiment_meta.json"
    mf.write_text(json.dumps({"total_runs": total_runs}, indent=2))


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
    p.add_argument("--dry-run",   action="store_true",
                   help="Print what would run without executing")
    return p.parse_args()


def main():
    args = parse_args()

    models = [m.strip() for m in args.models.split(",")] if args.models \
        else DEFAULT_MODELS

    datasets = [Path(d.strip()) for d in args.datasets.split(",")] if args.datasets \
        else DEFAULT_DATASETS

    combos = list(all_combinations()) if args.ablations else list(DEFAULT_ABLATIONS)
    if args.ablations:
        wanted = {t.strip() for t in args.ablations.split(",")}
        combos = [(tag, flags) for tag, flags in combos if tag in wanted]
        if not combos:
            sys.exit(f"No ablations matched: {args.ablations}")
    binary = None if args.dry_run else find_binary()

    signal.signal(signal.SIGINT, _request_shutdown)
    signal.signal(signal.SIGTERM, _request_shutdown)

    threading.Thread(target=_z3_watchdog, daemon=True, name="z3-watchdog").start()

    n_runs = 0  # accumulated after loading each dataset's feature list
    print(f"Planned: {len(datasets)} dataset(s) × {len(models)} model(s) "
          f"× {len(combos)} ablation(s) (feature count TBD per dataset)", flush=True)

    if args.dry_run:
        run_num = 0
        for model in models:
            for tag, _ in combos:
                for dataset in datasets:
                    run_num += 1
                    output = EXPERIMENTS_DIR / "results" / dataset.name / slug(model) / f"{tag}.jsonl"
                    print(f"[{run_num}/{n_runs}] {dataset.name}  {model}  {tag}"
                          f"  →  {output.relative_to(EXPERIMENTS_DIR)}", flush=True)
        return

    results_dir = EXPERIMENTS_DIR / "results"

    errors = []
    run_num = 0
    results_dir.mkdir(parents=True, exist_ok=True)

    # ── Phase 1: setup + prepare all datasets to know the full total_runs upfront ──
    prepared: list = []  # list of (dataset, all_features)
    for dataset in datasets:
        if not dataset.is_dir():
            print(f"[SKIP] dataset not found: {dataset}", flush=True)
            continue
        try:
            setup_dataset(dataset)
        except RuntimeError as e:
            print(f"ERROR: dataset setup failed for {dataset.name}: {e}", file=sys.stderr, flush=True)
            errors.append((dataset.name, str(e)))
            continue
        features_file = None
        try:
            features_file = run_prepare(dataset)
            all_lines = Path(features_file).read_text().splitlines(keepends=True)
            if args.limit_features and args.limit_features < len(all_lines):
                limited = features_file.parent / (features_file.stem + "_limited.txt")
                limited.write_text("".join(all_lines[:args.limit_features]))
                features_file.unlink()
                features_file = limited
            all_features = parse_features_file(features_file)
            solvable = load_solvable_filter(dataset)
            all_features = apply_solvable_filter(all_features, solvable)
            n_runs += len(all_features) * len(models) * len(combos)
            print(f"  {dataset.name}: {len(all_features)} feature(s) → "
                  f"{len(all_features) * len(models) * len(combos)} runs "
                  f"(total so far: {n_runs})", flush=True)
            prepared.append((dataset, all_features))
        except RuntimeError as e:
            print(f"ERROR in prepare for {dataset.name}: {e}", file=sys.stderr, flush=True)
            errors.append((dataset.name, str(e)))
        finally:
            if features_file:
                features_file.unlink(missing_ok=True)

    write_meta(results_dir, n_runs)
    print(f"Total: {n_runs} runs across {len(prepared)} dataset(s).", flush=True)

    # ── Phase 2: run experiments ──
    # Loop order: model → ablation (Hamming-ordered) → dataset → feature
    # This ensures run #1 (first model × ablation pair) is applied to every feature
    # across every dataset before starting run #2.

    # Pre-build completed set per output file (avoid re-reading on every skip)
    completed_cache: dict = {}
    def get_completed(path):
        if path not in completed_cache:
            completed_cache[path] = completed_features_in_jsonl(path)
        return completed_cache[path]

    rate_limited_cache: dict = {}
    def get_rate_limited(path):
        if path not in rate_limited_cache:
            rate_limited_cache[path] = rate_limited_timestamps(path)
        return rate_limited_cache[path]

    # cooldowns[model] = timestamp after which the model may be retried.
    cooldowns: dict = {}

    # Build flat ordered list of (model, tag, flags) for Phase 2.
    combo_map = {tag: flags for tag, flags in combos}
    if args.models or args.ablations:
        run_order = [(m, t, f) for m in models for t, f in combos]
    else:
        run_order = [
            (m, t, combo_map[t])
            for m, t in DEFAULT_RUN_ORDER
            if m in models and t in combo_map
        ]

    while True:
        models_to_retry: set = set()
        rate_hit_models: set = set()

        for model, tag, flags in run_order:
            if _shutdown_requested:
                break
            if model in rate_hit_models:
                continue

            # Model is cooling down — skip this pass, revisit later.
            if cooldowns.get(model, 0) > time.time():
                models_to_retry.add(model)
                continue
            cooldowns.pop(model, None)

            for dataset, all_features in prepared:
                if _shutdown_requested or model in rate_hit_models:
                    break
                # Acquire the dataset lock only for the duration of this sweep.
                try:
                    lock = acquire_lock(dataset)
                except RuntimeError as e:
                    print(f"ERROR: {e}", file=sys.stderr, flush=True)
                    errors.append((dataset.name, str(e)))
                    continue

                try:
                    for class_name, feature_name in all_features:
                        if _shutdown_requested or model in rate_hit_models:
                            break
                        run_num += 1
                        feat_id  = (class_name, feature_name)
                        feat_str = f"{class_name}.{feature_name}" if feature_name else class_name

                        output_dir = EXPERIMENTS_DIR / "results" / dataset.name / slug(model)
                        output     = output_dir / f"{tag}.jsonl"
                        prefix     = f"[{run_num}/{n_runs}] {dataset.name}  {model}  {tag}  {feat_str}"
                        output_dir.mkdir(parents=True, exist_ok=True)

                        if feat_id in get_completed(output):
                            print(f"{prefix}  [SKIP]", flush=True)
                            continue

                        print(prefix, flush=True)

                        # If previously rate-limited, wait before retrying
                        rl_map = get_rate_limited(output)
                        if feat_id in rl_map:
                            wait_until = rl_map[feat_id] + RATE_LIMIT_RETRY_WAIT
                            delay = wait_until - time.time()
                            if delay > 0:
                                print(f"  Previously rate-limited; waiting {delay:.0f}s before retry...",
                                      flush=True)
                                time.sleep(delay)

                        git_reset(dataset)

                        run_fd, run_features_path = tempfile.mkstemp(
                            suffix=".txt", prefix="run_features_")
                        with os.fdopen(run_fd, "w") as f:
                            f.write(f"{feat_str}\n")
                        run_features = Path(run_features_path)

                        try:
                            run_one(binary, dataset, run_features, model, flags,
                                    output, total_features=1, append=True)
                            completed_cache.pop(output, None)
                            rate_limited_cache.pop(output, None)
                            render_html(results_dir)
                        except subprocess.CalledProcessError as ex:
                            if ex.returncode == 2:
                                print(f"  RATE LIMITED — will retry {model} after "
                                      f"{RATE_LIMIT_RETRY_WAIT}s", flush=True)
                                completed_cache.pop(output, None)
                                rate_limited_cache.pop(output, None)
                                cooldowns[model] = time.time() + RATE_LIMIT_RETRY_WAIT
                                models_to_retry.add(model)
                                rate_hit_models.add(model)
                            else:
                                print(f"  ERROR: {ex}", file=sys.stderr, flush=True)
                                errors.append((f"run {run_num}", str(ex)))
                        finally:
                            run_features.unlink(missing_ok=True)

                finally:
                    release_lock(lock)

        if _shutdown_requested or not models_to_retry:
            break

        # If every model that needs retrying is still cooling down, sleep until
        # the earliest cooldown expires before starting the next pass.
        if all(cooldowns.get(m, 0) > time.time() for m in models_to_retry):
            wait = min(cooldowns[m] for m in models_to_retry) - time.time()
            if wait > 0:
                print(f"All active models rate-limited; sleeping {wait:.0f}s before retry...",
                      flush=True)
                time.sleep(wait)

        completed_cache.clear()
        rate_limited_cache.clear()

    print(flush=True)
    render_html(results_dir)

    if _shutdown_requested:
        print("Stopped after graceful shutdown. Re-run to continue from where it left off.",
              flush=True)

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
