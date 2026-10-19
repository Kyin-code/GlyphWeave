#!/usr/bin/env python3
"""Dump the raw bytes of a PE image at a given RVA and decode the handful of
x86-64 instructions that matter for prologue analysis.

Why: the machine has llvm-symbolizer (PDB -> file:line) but no disassembler, and
`dumpbin /DISASM` over a 30 MB debug exe is impractical.  For "what does the
faulting instruction do" the first few dozen bytes of a function are enough, and
those decode by hand.

Usage:
  python pe_bytes.py IMAGE.exe RVA_START RVA_END
"""

import struct
import sys


def sections(data):
    pe = struct.unpack_from("<I", data, 0x3C)[0]
    assert data[pe:pe + 4] == b"PE\0\0", "not a PE"
    nsec = struct.unpack_from("<H", data, pe + 6)[0]
    opt_size = struct.unpack_from("<H", data, pe + 20)[0]
    opt = pe + 24
    image_base = struct.unpack_from("<Q", data, opt + 24)[0]
    table = opt + opt_size
    out = []
    for i in range(nsec):
        off = table + i * 40
        name = data[off:off + 8].rstrip(b"\0").decode("ascii", "replace")
        vsize, vaddr, rawsize, rawptr = struct.unpack_from("<IIII", data, off + 8)
        out.append((name, vaddr, max(vsize, rawsize), rawptr, rawsize))
    return image_base, out


def rva_to_off(secs, rva):
    for name, vaddr, size, rawptr, rawsize in secs:
        if vaddr <= rva < vaddr + size:
            return rawptr + (rva - vaddr), name
    return None, None


OPS = [
    (b"\x55", "push rbp"),
    (b"\x48\x89\xe5", "mov rbp, rsp"),
    (b"\x48\x8b\xec", "mov rbp, rsp"),
    (b"\x48\x83\xec", "sub rsp, imm8"),
    (b"\x48\x81\xec", "sub rsp, imm32"),
    (b"\x48\x89\x4d", "mov [rbp+disp8], rcx  ; spill arg1"),
    (b"\x48\x89\x55", "mov [rbp+disp8], rdx  ; spill arg2"),
    (b"\x48\x89\x4c\x24", "mov [rsp+disp8], rcx"),
    (b"\x48\x89\x54\x24", "mov [rsp+disp8], rdx"),
    (b"\x4c\x89\x44\x24", "mov [rsp+disp8], r8"),
    (b"\x4c\x89\x4c\x24", "mov [rsp+disp8], r9"),
    (b"\x57", "push rdi"),
    (b"\x56", "push rsi"),
    (b"\x53", "push rbx"),
    (b"\xc3", "ret"),
    (b"\xcc", "int3"),
    (b"\xe8", "call rel32"),
    (b"\xff\x15", "call [rip+disp32]  ; import"),
    (b"\x0f\x1f", "nop"),
    (b"\x66\x90", "nop"),
    (b"\x90", "nop"),
    (b"\x48\x8b\x01", "mov rax, [rcx]"),
    (b"\xf3\x0f\x10", "movss/movups xmm"),
    (b"\xf2\x0f\x10", "movsd xmm"),
]


def main():
    path, start, end = sys.argv[1], int(sys.argv[2], 0), int(sys.argv[3], 0)
    with open(path, "rb") as fh:
        data = fh.read()
    image_base, secs = sections(data)
    print("image base 0x%X   sections: %s" % (image_base,
          ", ".join("%s@0x%X+0x%X" % (n, v, s) for n, v, s, _r, _rs in secs)))
    for rva in range(start, end):
        pass
    off, name = rva_to_off(secs, start)
    print("rva 0x%X -> %s file offset 0x%X" % (start, name, off))
    blob = data[off:off + (end - start)]
    for i in range(0, len(blob), 16):
        chunk = blob[i:i + 16]
        print("  +0x%03X  %-48s %s" % (i, " ".join("%02x" % b for b in chunk),
                                       "".join(chr(b) if 32 <= b < 127 else "." for b in chunk)))
    print()
    pc = 0
    while pc < len(blob):
        matched = False
        for pat, text in OPS:
            if blob.startswith(pat, pc):
                print("  +0x%03X  %-24s %s" % (pc, blob[pc:pc + len(pat)].hex(" "), text))
                pc += len(pat)
                matched = True
                break
        if not matched:
            pc += 1


if __name__ == "__main__":
    main()
