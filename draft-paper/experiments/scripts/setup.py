#!/usr/bin/env python3
"""Clone datasets and drop Ace.ecf into each cloned directory."""

import shutil
import subprocess
import sys
from pathlib import Path

SCRIPT_DIR      = Path(__file__).parent.resolve()
EXPERIMENTS_DIR = SCRIPT_DIR.parent
ACE_ECF         = SCRIPT_DIR / "Ace.ecf"

DATASETS = {
    "maple-recursive":       "git@github.com:CI-CSE/maple-recursive-eiffel.git",
    "buggy-java-jml-eiffel": "git@github.com:CI-CSE/buggy-java-jml-eiffel.git",
}


def main() -> None:
    datasets_dir = EXPERIMENTS_DIR / "datasets"
    datasets_dir.mkdir(exist_ok=True)

    for name, remote in DATASETS.items():
        dest = datasets_dir / name
        if dest.exists():
            print(f"[{name}] already exists — skipping clone")
        else:
            print(f"[{name}] cloning {remote}")
            subprocess.run(["git", "clone", remote, str(dest)], check=True)

        shutil.copy(ACE_ECF, dest / "Ace.ecf")
        print(f"[{name}] Ace.ecf copied\n")


if __name__ == "__main__":
    try:
        main()
    except subprocess.CalledProcessError as e:
        sys.exit(f"Error: {e}")
