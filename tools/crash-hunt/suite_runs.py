#!/usr/bin/env python3
"""Run the whole workspace test suite N times and record one line per run.

Used for the "the fix holds" evidence: every run's cargo exit code, the per-target
`test result:` lines and the wall time go to suite.log, and the full output of
each run is kept beside it.

Usage:
  python suite_runs.py --out RUNS --runs 3 --cwd <repo>/bevy [--extra-arg ...]
"""

import argparse
import os
import re
import subprocess
import sys
import time


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", required=True)
    ap.add_argument("--runs", type=int, default=3)
    ap.add_argument("--cwd", default=".")
    ap.add_argument("--cargo", default="cargo")
    args = ap.parse_args()
    os.makedirs(args.out, exist_ok=True)
    log = open(os.path.join(args.out, "suite.log"), "a", encoding="utf-8", buffering=1)
    log.write("# suite runs start %s\n" % time.strftime("%Y-%m-%d %H:%M:%S"))

    failures = 0
    for i in range(1, args.runs + 1):
        started = time.time()
        out_path = os.path.join(args.out, "suite-%02d.log" % i)
        with open(out_path, "w", encoding="utf-8") as fh:
            proc = subprocess.run(
                [args.cargo, "test", "--workspace", "--no-fail-fast"],
                cwd=args.cwd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                text=True)
            fh.write(proc.stdout)
        elapsed = time.time() - started
        results = re.findall(r"test result: ([^\n]+)", proc.stdout)
        passed = sum(int(m) for m in re.findall(r"ok\. (\d+) passed", proc.stdout))
        failed = sum(int(m) for m in re.findall(r"(\d+) failed", proc.stdout))
        crash = "0xc000" in proc.stdout.lower() or "STATUS_ACCESS" in proc.stdout
        log.write("run %02d  rc=%d  %6.1fs  passed=%d failed=%d crash=%s\n"
                  % (i, proc.returncode, elapsed, passed, failed, crash))
        for line in results:
            log.write("        test result: %s\n" % line)
        print("run %02d rc=%d %.1fs passed=%d failed=%d"
              % (i, proc.returncode, elapsed, passed, failed), flush=True)
        if proc.returncode != 0:
            failures += 1
    log.write("# %d/%d runs clean\n" % (args.runs - failures, args.runs))
    return 0 if failures == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
