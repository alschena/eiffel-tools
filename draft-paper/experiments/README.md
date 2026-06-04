# Experiments

Automated fix-feature experiments using `llm-correct-features` + AutoProof.

## Directory layout

```
experiments/
├── datasets/                  Gitignored; populated by setup.py
│   ├── maple-recursive/
│   └── buggy-java-jml-eiffel/
├── scripts/
│   ├── Ace.ecf                AutoProof ECF template (copied into each dataset)
│   ├── setup.py               git clone + copy Ace.ecf
│   ├── prepare.py             Identifies buggy features, outputs buggy_features.txt
│   ├── check_solvable.py      Pre-flight: verify reference implementations, write solvable/<dataset>.json
│   └── run_experiments.py     Main experiment runner
├── solvable/                  Per-feature solvability results (gitignored)
│   └── <dataset_name>.json    Written by check_solvable.py; read by run_experiments.py
├── results/                   Output JSONL files (gitignored)
│   └── <dataset>/<model_slug>/<ablation_tag>.jsonl
```

## Setup

### 1. Fetch datasets

```sh
./scripts/setup.py
```

Clones each dataset into `datasets/<name>/` (skips if the directory already
exists), then copies `scripts/Ace.ecf` into the cloned directory.

### 2. Build the tool

```sh
cargo build --release -p llm-correct-features
```

### 3. Set environment variables

```sh
export OPENROUTER_TOKEN=<your-token>
export AP=/path/to/autoproof          # e.g. ~/sci/reif/research/extension/autoproof
export ISE_EIFFEL=/path/to/Eiffel     # e.g. ~/sci/Eiffel_24.05
export ISE_PLATFORM=linux-x86-64
export ISE_LIBRARY=$ISE_EIFFEL/library
```

### 4. (Optional) Pre-flight solvability check

Some correct reference implementations cannot be verified by AutoProof — typically
because their postconditions require inductive reasoning that z3 cannot discharge
(e.g. proving equivalence between a recursive and an iterative factorial).
Running the LLM experiment on such features is pointless: even a perfect fix will time out.

Run this once per JML-style dataset before the experiment:

```sh
cd experiments
AP_COMMAND=$AP/EIFGENs/batch/F_code/ecb \
  python3 scripts/check_solvable.py datasets/buggy-java-jml-eiffel
```

This verifies every correct (unnumbered) reference class against AutoProof,
kills the entire ecb → boogie → z3 process tree on timeout, and writes:

```
datasets/buggy-java-jml-eiffel/solvable_features.json
```

The experiment runner reads this file automatically at startup and skips any
`CLASS_N.feature` whose base-class entry is absent or not `"verified"`.

**Findings for `buggy-java-jml-eiffel`** (timeout 120 s, run 2026-05-27):

- 163 / 163 features verified across 33 classes
- 1 class timed out: **`COMBINATION_PERMUTATION`** — its postconditions use
  `factorial_rec` (recursive) while the body calls `factorial_loop` (iterative);
  z3 cannot prove their equivalence without induction lemmas
- 9 buggy variants are skipped as a result: `COMBINATION_PERMUTATION_1` through
  `COMBINATION_PERMUTATION_9` (features `combination`, `permutation`, `select_either`)

**To inspect what is skipped**, compare `prepare.py` output against the solvable list:

```sh
cd experiments
python3 scripts/prepare.py datasets/buggy-java-jml-eiffel \
  | python3 - <<'EOF'
import sys, json, re
from pathlib import Path
data = json.loads(Path("solvable/buggy-java-jml-eiffel.json").read_text())
solvable = {k for k, v in data["results"].items() if v == "verified"}
for line in sys.stdin:
    cls, feat = line.strip().split(".", 1)
    base = re.sub(r"_\d+$", "", cls)
    status = "solvable" if f"{base}.{feat}" in solvable else "SKIPPED"
    print(f"{status:10s} {line.strip()}")
EOF
```

Or simply check which classes timed out:

```sh
python3 -c "
import json
from pathlib import Path
d = json.loads(Path('solvable/buggy-java-jml-eiffel.json').read_text())
print('Timed out:', d['timeout_classes'])
print('Generated:', d['generated_at'][:10])
"
```

## Running experiments

```sh
cd experiments
./scripts/run_experiments.py
```

This runs **128 combinations**: 2 models × 2 datasets × 32 ablation configurations.

### Ablation axes

The class invariant is always included. The other 5 prompt parts can be toggled:

| Short name | CLI flag                     | Description                              |
|------------|------------------------------|------------------------------------------|
| `task`     | `--no-task-instruction`      | "The following feature does not verify…" |
| `mod`      | `--no-modification-constraints` | "IMPORTANT: only modify body/locals…"  |
| `pre`      | `--no-precondition-identifiers` | Available identifiers in `require`      |
| `post`     | `--no-postcondition-identifiers`| Available identifiers in `ensure`      |
| `err`      | `--no-error-message`         | AutoProof error output                   |

Output filenames encode which parts are **disabled**. `full` means all parts enabled.

Examples:
- `full.jsonl` — all prompt parts enabled (baseline)
- `no_err.jsonl` — error message omitted
- `no_pre_no_post.jsonl` — identifier lists omitted

### Override models or datasets

```sh
./scripts/run_experiments.py --models openai/gpt-4o-mini
./scripts/run_experiments.py --datasets datasets/maple-recursive --models modelA,modelB
./scripts/run_experiments.py --dry-run   # preview all runs without executing
```

## Output format

Each `.jsonl` file contains one JSON object per feature:

```json
{
  "class_name": "BUBBLE_SORT_1",
  "feature_name": "sort",
  "model": "liquid/lfm-2.5-1.2b-instruct:free",
  "success": true,
  "max_retries_reached": false,
  "llm_interactions": 3,
  "final_status": "...",
  "total_elapsed_time_seconds": 12.4,
  "interactions": [
    {
      "interaction_number": 1,
      "error_message_before": "...",
      "error_message": "...",
      "prompt": "...",
      "llm_message": "...",
      "applied": true,
      "before_code": "...",
      "after_code": "..."
    }
  ]
}
```

## Dataset modes

`scripts/prepare.py` auto-detects the dataset structure:

- **JML** (`buggy-java-jml-eiffel`): nested subdirectories with a base file and
  numbered variants. Compares each variant to its base to identify the buggy
  feature and strips hint comments from the variant before the LLM sees it.
  Outputs `CLASS_N.feature_name` pairs.

- **Maple** (`maple-recursive`): flat directory of numbered `.e` files with no
  base counterpart. Outputs class names only; AutoProof identifies which
  features fail at runtime.
