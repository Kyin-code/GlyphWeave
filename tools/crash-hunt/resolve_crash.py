#!/usr/bin/env python3
"""Resolve image offsets in a Rust MSVC binary to module::function at file:line.

The tool chain on this machine has no cdb / WinDbg / rustfilt, but Visual Studio
ships `llvm-symbolizer.exe`, which reads the PDB that rustc writes next to every
debug binary.  `--relative-address` makes it interpret the argument as an RVA,
which is exactly what WER's "Fault offset" and a PE image base give you.

Usage
-----
  python resolve_crash.py IMAGE.exe 0x348aeb 0x348a90 ...

The symbolizer is found from $GW_SYMBOLIZER, then from the VS install reported
by vswhere, so the command is re-runnable on this machine without edits.
"""

import glob
import os
import shutil
import subprocess
import sys


def find_symbolizer():
    env = os.environ.get("GW_SYMBOLIZER")
    if env and os.path.isfile(env):
        return env
    found = shutil.which("llvm-symbolizer")
    if found:
        return found
    vswhere = r"C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe"
    roots = []
    if os.path.isfile(vswhere):
        try:
            out = subprocess.run([vswhere, "-all", "-products", "*",
                                  "-property", "installationPath"],
                                 capture_output=True, text=True).stdout
            roots += [line.strip() for line in out.splitlines() if line.strip()]
        except OSError:
            pass
    roots += [r"G:\vs", r"C:\Program Files\Microsoft Visual Studio\2022",
              r"C:\Program Files (x86)\Microsoft Visual Studio\2022"]
    for root in roots:
        for pattern in ("VC/Tools/MSVC/*/bin/Hostx64/x64/llvm-symbolizer.exe",
                        "VC/Tools/Llvm/x64/bin/llvm-symbolizer.exe"):
            hits = sorted(glob.glob(os.path.join(root, pattern)))
            if hits:
                return hits[-1]
    return None


def main():
    if len(sys.argv) < 3:
        print(__doc__)
        return 2
    image, addrs = sys.argv[1], sys.argv[2:]
    if not os.path.isfile(image):
        print("no such image: %s" % image)
        return 2
    pdb = os.path.splitext(image)[0] + ".pdb"
    sym = find_symbolizer()
    print("image : %s (%d bytes, mtime %s)"
          % (image, os.path.getsize(image),
             __import__("time").strftime("%Y-%m-%d %H:%M:%S",
                                         __import__("time").localtime(
                                             os.path.getmtime(image)))))
    print("pdb   : %s%s" % (pdb, "" if os.path.isfile(pdb) else "  [MISSING]"))
    print("tool  : %s" % sym)
    if not sym:
        print("llvm-symbolizer not found; set GW_SYMBOLIZER")
        return 2
    payload = "\n".join(addrs) + "\n"
    out = subprocess.run([sym, "--obj=" + image, "--inlines",
                          "--functions=short", "--relative-address"],
                         input=payload, capture_output=True, text=True)
    if out.returncode != 0:
        print(out.stderr)
        return out.returncode
    blocks = out.stdout.split("\n\n")
    for addr, block in zip(addrs, blocks):
        lines = [l for l in block.splitlines() if l.strip()]
        print("\n%s" % addr)
        for line in lines or ["<no symbol: offset outside any function>"]:
            print("   " + line)
    return 0


if __name__ == "__main__":
    sys.exit(main())
