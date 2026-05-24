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
│   └── run_experiments.py     Main experiment runner
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
