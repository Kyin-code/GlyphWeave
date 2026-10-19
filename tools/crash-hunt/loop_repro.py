#!/usr/bin/env python3
"""Loop test binaries under crashwatch.py until a fatal exception is caught.

Every iteration runs one test binary in its own process; crashwatch reports
exit code 3 when the child died on an exception it could not survive, and that
is the only condition that stops the loop early.

Usage
-----
  python loop_repro.py --out RUNS --worker 1 --rounds 40 \\
      --exe a.exe --exe b.exe

Each iteration appends one line to RUNS/worker-<id>/loop.log with the wall time
and the child's own "test result:" line, so a 200-run pass record can be
rebuilt from the log without trusting this script.
"""

import argparse
import os
import re
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
WATCH = os.path.join(HERE, "crashwatch.py")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", required=True)
    ap.add_argument("--worker", default="0")
    ap.add_argument("--rounds", type=int, default=10)
    ap.add_argument("--python", default=sys.executable)
    ap.add_argument("--exe", action="append", default=[])
    ap.add_argument("--arg", action="append", default=[],
                    help="extra argument appended after every exe (e.g. a libtest filter)")
    args = ap.parse_args()
    if not args.exe:
        ap.error("at least one --exe")

    wdir = os.path.join(args.out, "worker-%s" % args.worker)
    os.makedirs(wdir, exist_ok=True)
    log = open(os.path.join(wdir, "loop.log"), "a", encoding="utf-8", buffering=1)
    log.write("# worker %s start %s\n" % (args.worker, time.strftime("%Y-%m-%d %H:%M:%S")))

    runs = 0
    for rnd in range(args.rounds):
        for exe in args.exe:
            base = os.path.basename(exe).replace(".exe", "")
            dump = os.path.join(wdir, "dump-%03d-%s" % (runs, base))
            started = time.time()
            proc = subprocess.run(
                [args.python, WATCH, "--out", dump, "--", exe] + args.arg,
                capture_output=True, text=True)
            elapsed = time.time() - started
            out = proc.stdout + proc.stderr
            with open(os.path.join(wdir, "iter-%03d-%s.log" % (runs, base)),
                      "w", encoding="utf-8") as fh:
                fh.write(out)
            results = re.findall(r"test result: ([^\n]+)", out)
            verdict = "; ".join(results) if results else "no test-result line"
            crashed = proc.returncode == 3
            log.write("%03d %-52s %6.1fs rc=%d %s %s\n"
                      % (runs, base, elapsed, proc.returncode,
                         "CRASH" if crashed else "ok", verdict))
            runs += 1
            if crashed:
                log.write("# CRASH captured in %s\n" % dump)
                print("CRASH: %s" % dump, flush=True)
                print(out[-4000:], flush=True)
                return 3
    log.write("# worker %s done, %d runs, no crash\n" % (args.worker, runs))
    print("worker %s: %d runs, no crash" % (args.worker, runs), flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
