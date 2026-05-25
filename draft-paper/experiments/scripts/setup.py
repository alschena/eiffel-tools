#!/usr/bin/env python3
"""Clone datasets and drop Ace.ecf into each cloned directory."""

import re
import shutil
import subprocess
import sys
import uuid
from pathlib import Path

SCRIPT_DIR      = Path(__file__).parent.resolve()
EXPERIMENTS_DIR = SCRIPT_DIR.parent
ACE_ECF         = SCRIPT_DIR / "Ace.ecf"

DATASETS = {
    "maple-recursive":       ("git@github.com:CI-CSE/maple-recursive-eiffel.git",   None),
    "buggy-java-jml-eiffel": ("git@github.com:CI-CSE/buggy-java-jml-eiffel.git",    "december"),
}


def main() -> None:
    datasets_dir = EXPERIMENTS_DIR / "datasets"
    datasets_dir.mkdir(exist_ok=True)

    for name, (remote, branch) in DATASETS.items():
        dest = datasets_dir / name
        if dest.exists():
            print(f"[{name}] already exists — skipping clone")
        else:
            print(f"[{name}] cloning {remote}" + (f" (branch {branch})" if branch else ""))
            cmd = ["git", "clone", remote, str(dest)]
            if branch:
                cmd += ["--branch", branch]
            subprocess.run(cmd, check=True)

        ecf_dst = dest / "Ace.ecf"
        text = re.sub(r'uuid="[^"]*"', f'uuid="{uuid.uuid4()}"', ACE_ECF.read_text())
        ecf_dst.write_text(text)
        print(f"[{name}] Ace.ecf written (fresh uuid)\n")


if __name__ == "__main__":
    try:
        main()
    except subprocess.CalledProcessError as e:
        sys.exit(f"Error: {e}")
