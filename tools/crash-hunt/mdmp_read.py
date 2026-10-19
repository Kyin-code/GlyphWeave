#!/usr/bin/env python3
"""Read a Windows minidump (MDMP) far enough to recover the fault.

This machine has no cdb / WinDbg / dumpchk, and WER writes the dumps itself
(`memory.hdmp` in the ReportQueue).  A minidump is a small, documented container:
a header, a stream directory, and streams.  The three streams that matter here
are the exception stream (code, faulting address, access parameters), the thread
list (each thread's CONTEXT and stack bytes) and the module list (base, size,
timestamp, name), which together give the same picture a debugger would.

Usage:
  python mdmp_read.py DUMP.dmp [--stack-words N] [--symbolize EXE]
"""

import argparse
import struct
import subprocess
import sys

STREAM_THREAD_LIST = 3
STREAM_MODULE_LIST = 4
STREAM_MEMORY_LIST = 5
STREAM_EXCEPTION = 6
STREAM_MEMORY64_LIST = 9
STREAM_SYSTEM_INFO = 7
STREAM_MISC_INFO = 15

CODE_NAMES = {
    0xC0000005: "STATUS_ACCESS_VIOLATION",
    0xC00000FD: "STATUS_STACK_OVERFLOW",
    0xC0000409: "STATUS_STACK_BUFFER_OVERRUN",
    0xC000001D: "STATUS_ILLEGAL_INSTRUCTION",
    0xC0000374: "STATUS_HEAP_CORRUPTION",
    0x80000003: "EXCEPTION_BREAKPOINT",
}


class Dump:
    def __init__(self, data):
        self.data = data
        assert data[:4] == b"MDMP", "not a minidump"
        self.streams = {}
        number, directory = struct.unpack_from("<II", data, 8)
        for i in range(number):
            stype, size, rva = struct.unpack_from("<III", data, directory + i * 12)
            self.streams.setdefault(stype, []).append((size, rva))

    def raw(self, rva, size):
        return self.data[rva:rva + size]

    def string(self, rva):
        length = struct.unpack_from("<I", self.data, rva)[0]
        return self.data[rva + 4:rva + 4 + length].decode("utf-16-le", "replace")

    # ---- streams -------------------------------------------------------
    def modules(self):
        size, rva = self.streams[STREAM_MODULE_LIST][0]
        count = struct.unpack_from("<I", self.data, rva)[0]
        out = []
        for i in range(count):
            off = rva + 4 + i * 108
            (base, image_size, checksum, timestamp, name_rva) = struct.unpack_from(
                "<QIIII", self.data, off)
            cv_size, cv_rva = struct.unpack_from("<II", self.data, off + 0x24)
            out.append({"base": base, "size": image_size,
                        "timestamp": timestamp, "name": self.string(name_rva),
                        "cv_size": cv_size, "cv_rva": cv_rva})
        return out

    def threads(self):
        size, rva = self.streams[STREAM_THREAD_LIST][0]
        count = struct.unpack_from("<I", self.data, rva)[0]
        out = []
        for i in range(count):
            off = rva + 4 + i * 48
            tid, _suspend, _pc, _prio, teb = struct.unpack_from("<IIIIQ", self.data, off)
            stack_start, stack_size, stack_rva = struct.unpack_from(
                "<QII", self.data, off + 24)
            ctx_size, ctx_rva = struct.unpack_from("<II", self.data, off + 40)
            ctx = self.raw(ctx_rva, ctx_size)
            regs = {}
            for name, roff in (("rax", 0x78), ("rcx", 0x80), ("rdx", 0x88),
                               ("rbx", 0x90), ("rsp", 0x98), ("rbp", 0xA0),
                               ("rsi", 0xA8), ("rdi", 0xB0), ("r8", 0xB8),
                               ("r9", 0xC0), ("r10", 0xC8), ("r11", 0xD0),
                               ("r12", 0xD8), ("r13", 0xE0), ("r14", 0xE8),
                               ("r15", 0xF0), ("rip", 0xF8)):
                if roff + 8 <= len(ctx):
                    regs[name] = struct.unpack_from("<Q", ctx, roff)[0]
            out.append({"tid": tid, "stack_start": stack_start,
                        "stack_size": stack_size,
                        "stack": self.raw(stack_rva, stack_size), "regs": regs})
        return out

    def exception(self):
        size, rva = self.streams[STREAM_EXCEPTION][0]
        tid, _align = struct.unpack_from("<II", self.data, rva)
        rec = rva + 8
        code, flags, nested, addr, nparams, _unused = struct.unpack_from(
            "<IIQQII", self.data, rec)
        params = struct.unpack_from("<15Q", self.data, rec + 0x20)
        ctx_size, ctx_rva = struct.unpack_from("<II", self.data, rec + 0x98)
        ctx = self.raw(ctx_rva, ctx_size)
        regs = {}
        for name, roff in (("rsp", 0x98), ("rbp", 0xA0), ("rip", 0xF8),
                           ("rax", 0x78), ("rcx", 0x80), ("rdx", 0x88),
                           ("r8", 0xB8), ("r9", 0xC0)):
            if roff + 8 <= len(ctx):
                regs[name] = struct.unpack_from("<Q", ctx, roff)[0]
        return {"tid": tid, "code": code, "flags": flags, "address": addr,
                "nparams": nparams, "params": params, "regs": regs}


def find(modules, addr):
    for m in modules:
        if m["base"] and m["base"] <= addr < m["base"] + m["size"]:
            return m, addr - m["base"]
    return None, None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("dump")
    ap.add_argument("--stack-words", type=int, default=250)
    ap.add_argument("--symbolize", default="")
    args = ap.parse_args()
    with open(args.dump, "rb") as fh:
        d = Dump(fh.read())

    modules = d.modules()
    exc = d.exception()
    threads = d.threads()

    print("streams: %s" % sorted(d.streams))
    print("modules (%d):" % len(modules))
    for m in modules[:12]:
        print("  0x%016X +0x%-8X ts=0x%08X %s"
              % (m["base"], m["size"], m["timestamp"], m["name"]))
    if len(modules) > 12:
        print("  ... %d more" % (len(modules) - 12))

    code = exc["code"]
    print("\nexception: 0x%08X %s" % (code, CODE_NAMES.get(code, "?")))
    m, off = find(modules, exc["address"])
    print("fault ip : 0x%016X  %s + 0x%X"
          % (exc["address"], m["name"] if m else "<no module>", off or 0))
    if code == 0xC0000005 and exc["nparams"] >= 2:
        kind = {0: "read", 1: "write", 8: "execute"}.get(exc["params"][0],
                                                         str(exc["params"][0]))
        rsp = exc["regs"].get("rsp", 0)
        print("access   : %s @ 0x%016X  (rsp%+d)" % (kind, exc["params"][1],
                                                    exc["params"][1] - rsp))
    print("regs     : %s" % ", ".join("%s=0x%X" % kv for kv in exc["regs"].items()))

    tid = exc["tid"]
    thread = next((t for t in threads if t["tid"] == tid), None)
    if thread is None:
        print("faulting thread %d not in dump" % tid)
        return 0
    start = thread["stack_start"]
    print("\nfaulting thread %d stack: 0x%X .. 0x%X (%d bytes)"
          % (tid, start, start + thread["stack_size"], thread["stack_size"]))

    words = []
    blob = thread["stack"]
    for i in range(min(args.stack_words, len(blob) // 8)):
        v = struct.unpack_from("<Q", blob, i * 8)[0]
        m, off = find(modules, v)
        if m:
            words.append((start + i * 8, v, m, off))
    print("stack words inside a module: %d" % len(words))

    names = {}
    if args.symbolize:
        target = os.path.basename(args.symbolize).lower()
        offs = sorted({off for _a, _v, m, off in words
                       if os.path.basename(m["name"]).lower() == target})
        if offs:
            payload = "\n".join(hex(o) for o in offs) + "\n"
            out = subprocess.run(
                [args.symbolize, "--obj=" + args.symbolize, "--functions=short",
                 "--relative-address"],
                input=payload, capture_output=True, text=True).stdout
            for o, blk in zip(offs, out.split("\n\n")):
                names[o] = blk.splitlines()[0] if blk.strip() else ""

    for addr, v, m, off in words:
        label = names.get(off, "") if os.path.basename(
            m["name"]).lower() == os.path.basename(args.symbolize).lower() else ""
        print("  stack+0x%-6X 0x%016X  %s+0x%-8X %s"
              % (addr - start, v, os.path.basename(m["name"]), off, label))
    return 0


if __name__ == "__main__":
    sys.exit(main())
