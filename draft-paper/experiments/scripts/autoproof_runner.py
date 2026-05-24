"""
Shared AutoProof runner for check scripts.

Launches ecb -autoproof CLASS, polls the Boogie proof output file,
kills ecb+boogie on completion or timeout, and returns the result.
"""

import os
import subprocess
import time
from pathlib import Path

PROOF_OUT = Path("EIFGENs/experiment_example/Proofs/output0.txt")
TIMEOUT_SECS = 120


def run(class_name: str) -> tuple[bool, str]:
    """
    Run AutoProof on class_name.
    Returns (passed, result_line) where:
      passed      — True if Boogie reported 0 errors
      result_line — the "Boogie program verifier finished …" line, or "TIMEOUT"
    """
    ecb = Path(os.environ["AP"]) / "EIFGENs/batch/F_code/ecb"

    PROOF_OUT.unlink(missing_ok=True)
    proc = subprocess.Popen(
        [str(ecb), "-autoproof", class_name],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )

    result_line = ""
    elapsed = 0
    while elapsed < TIMEOUT_SECS:
        if not proc.poll() is None:
            # ecb exited on its own — give Boogie a moment to flush
            time.sleep(1)
        try:
            for line in PROOF_OUT.read_text().splitlines():
                if "Boogie program verifier finished" in line:
                    result_line = line.strip()
                    break
        except OSError:
            pass
        if result_line:
            break
        time.sleep(2)
        elapsed += 2

    # Always kill ecb and any lingering boogie processes
    try:
        proc.kill()
    except OSError:
        pass
    subprocess.run(["pkill", "-9", "-f", "boogie"], capture_output=True)
    proc.wait()

    if not result_line:
        return False, "TIMEOUT"

    passed = ", 0 errors" in result_line
    return passed, result_line
