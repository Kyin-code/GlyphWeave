#!/usr/bin/env python3
"""Find functions in a Rust source tree that call themselves.

The crash family under investigation is a stack overflow (STATUS_STACK_OVERFLOW
and its STATUS_ACCESS_VIOLATION sibling), which in an all-safe codebase means a
recursive function whose depth is data dependent.  This lists every candidate so
each one can be checked by hand.

Usage:
  python find_recursion.py DIR [DIR...]
"""

import os
import re
import sys

FN = re.compile(r"^\s*(?:pub(?:\([^)]*\))?\s+)?(?:const\s+)?(?:async\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)")


def scan(path):
    with open(path, "r", encoding="utf-8", errors="replace") as fh:
        lines = fh.readlines()
    hits = []
    depth = 0
    current = None
    start = 0
    for i, line in enumerate(lines, 1):
        m = FN.match(line)
        if m and depth == 0:
            current = m.group(1)
            start = i
            depth = 1
            continue
        if current is None:
            continue
        depth += line.count("{") - line.count("}")
        if depth <= 0:
            body = "".join(lines[start:i])
            if re.search(r"(?<![\w.])%s\s*(::<[^>]*>)?\s*\(" % re.escape(current), body):
                hits.append((current, start))
            current = None
            depth = 0
    return hits


def main():
    roots = sys.argv[1:] or ["."]
    total = 0
    for root in roots:
        for dirpath, _dirs, files in os.walk(root):
            if "target" in dirpath.split(os.sep):
                continue
            for name in files:
                if not name.endswith(".rs"):
                    continue
                path = os.path.join(dirpath, name)
                for fn, line in scan(path):
                    total += 1
                    print("%s:%d  fn %s" % (path, line, fn))
    print("# %d recursive functions" % total)


if __name__ == "__main__":
    main()
