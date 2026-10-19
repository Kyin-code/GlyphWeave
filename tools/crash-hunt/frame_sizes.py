#!/usr/bin/env python3
"""Rank the functions of a PE image by how much stack they allocate.

Rust debug builds keep a .pdata table with one RUNTIME_FUNCTION per function;
each points at UNWIND_INFO, whose UNWIND_CODE list says exactly how the frame is
built.  Summing UWOP_PUSH_NONVOL (8 bytes each) and UWOP_ALLOC_* gives the stack
the function needs before it calls anything -- the number that decides how many
levels of a recursion fit in a thread stack.

Usage:
  python frame_sizes.py IMAGE.exe [top_n] [--symbolize DIR]
"""

import os
import struct
import subprocess
import sys

UWOP_PUSH_NONVOL = 0
UWOP_ALLOC_LARGE = 1
UWOP_ALLOC_SMALL = 2
UWOP_SAVE_NONVOL = 4


def parse_pe(data):
    pe = struct.unpack_from("<I", data, 0x3C)[0]
    nsec = struct.unpack_from("<H", data, pe + 6)[0]
    opt_size = struct.unpack_from("<H", data, pe + 20)[0]
    opt = pe + 24
    image_base = struct.unpack_from("<Q", data, opt + 24)[0]
    table = opt + opt_size
    secs = {}
    for i in range(nsec):
        off = table + i * 40
        name = data[off:off + 8].rstrip(b"\0").decode("ascii", "replace")
        vsize, vaddr, rawsize, rawptr = struct.unpack_from("<IIII", data, off + 8)
        secs[name] = (vaddr, rawsize, rawptr)
    return image_base, secs


def rva_to_off(secs, rva):
    for _name, (vaddr, rawsize, rawptr) in secs.items():
        if vaddr <= rva < vaddr + rawsize:
            return rawptr + (rva - vaddr)
    return None


def frame_size(data, secs, unwind_rva):
    off = rva_to_off(secs, unwind_rva)
    if off is None:
        return None
    version_flags = data[off]
    count = data[off + 2]
    codes = off + 4
    total = 0
    i = 0
    while i < count:
        b = data[codes + i * 2]
        opinfo = data[codes + i * 2 + 1]
        op = b & 0x0F
        info = b >> 4
        if op == UWOP_PUSH_NONVOL:
            total += 8
            i += 1
        elif op == UWOP_ALLOC_LARGE:
            if info == 0:
                total += struct.unpack_from("<H", data, codes + (i + 1) * 2)[0] * 8
                i += 2
            else:
                total += struct.unpack_from("<I", data, codes + (i + 1) * 2)[0]
                i += 3
        elif op == UWOP_ALLOC_SMALL:
            total += info * 8 + 8
            i += 1
        elif op in (3, 4):        # SET_FPREG / SAVE_NONVOL
            i += 2
        elif op == 5:             # SAVE_NONVOL_FAR
            i += 3
        elif op == 6:
            i += 2
        elif op in (8, 9, 10):    # SAVE_XMM128/256, PUSH_MACHFRAME
            i += 2
        else:
            i += 1
    return total


def main():
    path = sys.argv[1]
    top = int(sys.argv[2]) if len(sys.argv) > 2 and sys.argv[2].isdigit() else 25
    symbolize = "--symbolize" in sys.argv
    with open(path, "rb") as fh:
        data = fh.read()
    image_base, secs = parse_pe(data)
    if ".pdata" not in secs:
        print("no .pdata")
        return
    vaddr, rawsize, rawptr = secs[".pdata"]
    entries = []
    for off in range(rawptr, rawptr + rawsize, 12):
        begin, end, unwind = struct.unpack_from("<III", data, off)
        if begin == 0 or end == 0:
            continue
        size = frame_size(data, secs, unwind)
        if size:
            entries.append((size, begin, end))
    entries.sort(reverse=True)
    print("%d functions with unwind info; image base 0x%X" % (len(entries), image_base))
    print("%-12s %-10s %s" % ("stack", "code size", "function (rva)"))
    picked = entries[:top]
    names = {}
    if symbolize:
        sym = os.environ.get("GW_SYMBOLIZER", "")
        payload = "\n".join(hex(rva) for _s, rva, _e in picked) + "\n"
        out = subprocess.run([sym, "--obj=" + path, "--functions=short",
                              "--relative-address"], input=payload,
                             capture_output=True, text=True).stdout
        blocks = [b.strip() for b in out.split("\n\n")]
        for (s, rva, _e), blk in zip(picked, blocks):
            names[rva] = blk.splitlines()[0] if blk else ""
    for size, begin, end in picked:
        print("0x%08X  0x%08X  rva 0x%X  %s"
              % (size, end - begin, begin, names.get(begin, "")))


if __name__ == "__main__":
    main()
