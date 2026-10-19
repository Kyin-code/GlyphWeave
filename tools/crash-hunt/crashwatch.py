#!/usr/bin/env python3
"""Launch a Windows process under the Win32 debug API and, on any fatal
exception, capture the exact fault (code, address, module+offset, access kind
and the accessed address) plus the faulting thread's stack, then symbolize it.

Why this exists: the machine has no procdump / cdb / WinDbg, and WER LocalDumps
needs an HKLM write we cannot do.  Everything here is user-mode and needs no
admin rights.

Usage
-----
  python crashwatch.py --out DIR [--symbolizer path\\llvm-symbolizer.exe] \\
      -- <exe> [args...]

Outputs into DIR, one file set per fatal exception:
  crash-<n>.json   exception record, module-relative offsets, registers,
                   RBP-chain return addresses, stack-scan candidates
  crash-<n>.stack  raw bytes of the faulting thread's stack (low, rsp, rbp, rip)
  crash-<n>.txt    human readable, already symbolized

Exit code: 0 when the child exited on its own, 3 when we caught a fatal
exception (so a loop driver can tell the two apart).
"""

import argparse
import ctypes
import ctypes.wintypes as w
import json
import os
import struct
import subprocess
import sys

k32 = ctypes.WinDLL("kernel32", use_last_error=True)
psapi = ctypes.WinDLL("psapi", use_last_error=True)

DEBUG_ONLY_THIS_PROCESS = 0x00000002
DBG_CONTINUE = 0x00010002

EXCEPTION_DEBUG_EVENT = 1
CREATE_PROCESS_DEBUG_EVENT = 3
EXIT_PROCESS_DEBUG_EVENT = 5
LOAD_DLL_DEBUG_EVENT = 6

EXCEPTION_BREAKPOINT = 0x80000003
EXCEPTION_SINGLE_STEP = 0x80000004
NAMES = {
    0xC0000005: "STATUS_ACCESS_VIOLATION",
    0xC00000FD: "STATUS_STACK_OVERFLOW",
    0xC0000409: "STATUS_STACK_BUFFER_OVERRUN",
    0xC000001D: "STATUS_ILLEGAL_INSTRUCTION",
    0xC0000094: "STATUS_INTEGER_DIVIDE_BY_ZERO",
    0xC000001E: "STATUS_INVALID_LOCK_SEQUENCE",
    0xC0000374: "STATUS_HEAP_CORRUPTION",
}
# exceptions the process itself is expected to survive -- report but do not stop
BENIGN = {EXCEPTION_BREAKPOINT, EXCEPTION_SINGLE_STEP, 0x4000001F,
          0x406D1388,   # MS_VC_EXCEPTION: thread name
          0xC000013A}   # control-C

CTX_FLAGS = 0x30
CTX_RSP, CTX_RBP, CTX_RIP = 0x98, 0xA0, 0xF8
CONTEXT_SIZE = 1232
CONTEXT_AMD64_FULL = 0x00100013
MAX_STACK_BYTES = 1 << 20
TH32CS_SNAPMODULE = 0x00000008


class STARTUPINFOW(ctypes.Structure):
    _fields_ = [
        ("cb", w.DWORD), ("lpReserved", w.LPWSTR), ("lpDesktop", w.LPWSTR),
        ("lpTitle", w.LPWSTR), ("dwX", w.DWORD), ("dwY", w.DWORD),
        ("dwXSize", w.DWORD), ("dwYSize", w.DWORD),
        ("dwXCountChars", w.DWORD), ("dwYCountChars", w.DWORD),
        ("dwFillAttribute", w.DWORD), ("dwFlags", w.DWORD),
        ("wShowWindow", w.WORD), ("cbReserved2", w.WORD),
        ("lpReserved2", ctypes.POINTER(ctypes.c_byte)),
        ("hStdInput", w.HANDLE), ("hStdOutput", w.HANDLE),
        ("hStdError", w.HANDLE),
    ]


class PROCESS_INFORMATION(ctypes.Structure):
    _fields_ = [("hProcess", w.HANDLE), ("hThread", w.HANDLE),
                ("dwProcessId", w.DWORD), ("dwThreadId", w.DWORD)]


class MODULEENTRY32W(ctypes.Structure):
    _fields_ = [("dwSize", w.DWORD), ("th32ModuleID", w.DWORD),
                ("th32ProcessID", w.DWORD), ("GlblcntUsage", w.DWORD),
                ("ProccntUsage", w.DWORD), ("modBaseAddr", ctypes.c_void_p),
                ("modBaseSize", w.DWORD), ("hModule", w.HMODULE),
                ("szModule", w.WCHAR * 256), ("szExePath", w.WCHAR * 260)]


class MEMORY_BASIC_INFORMATION64(ctypes.Structure):
    _fields_ = [("BaseAddress", ctypes.c_ulonglong),
                ("AllocationBase", ctypes.c_ulonglong),
                ("AllocationProtect", w.DWORD),
                ("__alignment1", w.DWORD),
                ("RegionSize", ctypes.c_ulonglong),
                ("State", w.DWORD),
                ("Protect", w.DWORD),
                ("Type", w.DWORD),
                ("__alignment2", w.DWORD)]


k32.CreateProcessW.restype = w.BOOL
k32.WaitForDebugEvent.argtypes = [ctypes.c_void_p, w.DWORD]
k32.OpenThread.restype = w.HANDLE
k32.OpenThread.argtypes = [w.DWORD, w.BOOL, w.DWORD]
k32.GetThreadContext.argtypes = [w.HANDLE, ctypes.c_void_p]
k32.ReadProcessMemory.argtypes = [w.HANDLE, ctypes.c_void_p, ctypes.c_void_p,
                                  ctypes.c_size_t, ctypes.POINTER(ctypes.c_size_t)]
k32.CreateToolhelp32Snapshot.restype = w.HANDLE
k32.CreateToolhelp32Snapshot.argtypes = [w.DWORD, w.DWORD]
k32.Module32FirstW.argtypes = [w.HANDLE, ctypes.POINTER(MODULEENTRY32W)]
k32.Module32NextW.argtypes = [w.HANDLE, ctypes.POINTER(MODULEENTRY32W)]
k32.TerminateProcess.argtypes = [w.HANDLE, w.UINT]
k32.QueryFullProcessImageNameW.argtypes = [w.HANDLE, w.DWORD, w.LPWSTR,
                                           ctypes.POINTER(w.DWORD)]


def read_process(proc, addr, size):
    buf = ctypes.create_string_buffer(size)
    got = ctypes.c_size_t(0)
    if not k32.ReadProcessMemory(proc, ctypes.c_void_p(addr), buf, size,
                                 ctypes.byref(got)):
        return b""
    return buf.raw[:got.value]


def read_process_walk(proc, addr, size, chunk=0x1000):
    """Read [addr, addr+size) page by page; stop at the first unreadable page.

    A single ReadProcessMemory of the whole range fails outright when the range
    runs off the committed part of the stack, so walk it instead."""
    out = bytearray()
    done = 0
    while done < size:
        piece = read_process(proc, addr + done, min(chunk, size - done))
        if not piece:
            break
        out += piece
        done += len(piece)
        if len(piece) < min(chunk, size - done + len(piece)):
            break
    return bytes(out)


def get_context(proc, tid):
    h = k32.OpenThread(0x0008 | 0x0040, False, tid)     # GET_CONTEXT|QUERY_INFO
    if not h:
        h = k32.OpenThread(0x1FFFFF, False, tid)
    if not h:
        return None
    try:
        ctx = ctypes.create_string_buffer(CONTEXT_SIZE)
        struct.pack_into("<I", ctx, CTX_FLAGS, CONTEXT_AMD64_FULL)
        if not k32.GetThreadContext(h, ctx):
            return None
        raw = ctx.raw
        regs = {}
        for name, off in (("rax", 0x78), ("rcx", 0x80), ("rdx", 0x88),
                          ("rbx", 0x90), ("rsp", CTX_RSP), ("rbp", CTX_RBP),
                          ("rsi", 0xA8), ("rdi", 0xB0), ("r8", 0xB8),
                          ("r9", 0xC0), ("r10", 0xC8), ("r11", 0xD0),
                          ("r12", 0xD8), ("r13", 0xE0), ("r14", 0xE8),
                          ("r15", 0xF0), ("rip", CTX_RIP)):
            regs[name] = struct.unpack_from("<Q", raw, off)[0]
        return regs
    finally:
        k32.CloseHandle(h)


def enum_modules(pid):
    snap = k32.CreateToolhelp32Snapshot(TH32CS_SNAPMODULE, pid)
    mods = []
    if not snap or snap == 0xFFFFFFFFFFFFFFFF:
        return mods
    try:
        me = MODULEENTRY32W()
        me.dwSize = ctypes.sizeof(MODULEENTRY32W)
        ok = k32.Module32FirstW(snap, ctypes.byref(me))
        while ok:
            mods.append({"base": int(me.modBaseAddr or 0),
                         "size": int(me.modBaseSize),
                         "name": me.szModule,
                         "path": me.szExePath})
            ok = k32.Module32NextW(snap, ctypes.byref(me))
    finally:
        k32.CloseHandle(snap)
    return mods


def module_for(mods, addr):
    for m in mods:
        if m["base"] and m["base"] <= addr < m["base"] + m["size"]:
            return m
    return None


MEM_COMMIT = 0x1000
MEM_RESERVE = 0x2000


def read_committed_stack(proc, rsp, cap=MAX_STACK_BYTES):
    """Read upward from the page holding rsp across every committed region.

    The stack's committed part is what a stack scan needs; uncommitted pages
    above it make a single ReadProcessMemory of a fixed-size window fail."""
    low = rsp & ~0xFFF
    out = bytearray()
    addr = low
    mbi = MEMORY_BASIC_INFORMATION64()
    while len(out) < cap:
        if not k32.VirtualQueryEx(proc, ctypes.c_void_p(addr),
                                  ctypes.byref(mbi), ctypes.sizeof(mbi)):
            break
        if mbi.State != MEM_COMMIT:
            break
        base = int(mbi.BaseAddress)
        size = int(mbi.RegionSize)
        if base > addr:      # hole: stop
            break
        take = min(size - (addr - base), cap - len(out))
        piece = read_process(proc, addr, take)
        if not piece:
            break
        out += piece
        addr = base + size
    return low, bytes(out)


def stack_bounds(proc, rsp):
    """Lowest address of the allocation holding rsp and how much is committed.

    For a stack-exhaustion fault `rsp - base` collapses toward zero and the
    access address sits at or below `base`; for a wild pointer it does not."""
    mbi = MEMORY_BASIC_INFORMATION64()
    if not k32.VirtualQueryEx(proc, ctypes.c_void_p(rsp), ctypes.byref(mbi),
                              ctypes.sizeof(mbi)):
        return None
    return int(mbi.AllocationBase), int(mbi.BaseAddress), int(mbi.RegionSize), int(mbi.State)


def walk_stack(proc, mods, regs, exe_mod):
    frames = []
    rbp, rsp = regs["rbp"], regs["rsp"]
    seen = set()
    while rbp and rbp not in seen and len(frames) < 256:
        seen.add(rbp)
        if rbp < rsp or rbp - rsp > MAX_STACK_BYTES or rbp % 8:
            break
        pair = read_process(proc, rbp, 16)
        if len(pair) < 16:
            break
        next_rbp, ret = struct.unpack("<QQ", pair)
        if ret == 0:
            break
        frames.append(ret)
        if next_rbp <= rbp:
            break
        rbp = next_rbp

    low, stack = read_committed_stack(proc, rsp)
    scanned = []
    if exe_mod and stack:
        base, size = exe_mod["base"], exe_mod["size"]
        for off in range(0, len(stack) - 8, 8):
            v = struct.unpack_from("<Q", stack, off)[0]
            if base <= v < base + size and v not in frames:
                scanned.append(v)
    return frames, scanned, low, stack


def symbolize_by_module(mods, addr_list):
    """{(base,offset): [frames]} -- one llvm-symbolizer run per module."""
    symbolizer = os.environ.get("GW_SYMBOLIZER", "")
    groups = {}
    for a in addr_list:
        m = module_for(mods, a)
        if m:
            groups.setdefault(m["path"], []).append(a - m["base"])
    out = {}
    if not symbolizer:
        return out
    for path, rels in groups.items():
        rels = sorted(set(rels))
        payload = "\n".join(hex(r) for r in rels) + "\n"
        try:
            p = subprocess.run(
                [symbolizer, "--obj=" + path, "--inlines", "--functions=short",
                 "--relative-address"],
                input=payload, capture_output=True, text=True, timeout=240)
        except Exception as exc:                                # noqa: BLE001
            out[path] = {"__error__": str(exc)}
            continue
        blocks, cur = {}, []
        for line in p.stdout.splitlines():
            if line.strip():
                cur.append(line.strip())
            else:
                if cur:
                    blocks[rels[len(blocks)]] = cur
                cur = []
        if cur:
            blocks[rels[len(blocks)]] = cur
        out[path] = blocks
    return out


def dump_crash(outdir, idx, proc, pid, tid, exc, exe_path):
    code, flags, addr, nparams, params = exc
    mods = enum_modules(pid)
    exe_mod = None
    exe_base_name = os.path.basename(exe_path).lower()
    for m in mods:
        if m["name"].lower() == exe_base_name:
            exe_mod = m
            break
    if exe_mod is None and mods:
        exe_mod = mods[0]
    regs = get_context(proc, tid) or {}
    frames, scanned, low, stack = walk_stack(proc, mods, regs, exe_mod)

    fm = module_for(mods, addr) or exe_mod
    rec = {
        "exception_code": "0x%08X" % code,
        "exception_name": NAMES.get(code, "?"),
        "exception_address": "0x%016X" % addr,
        "module": fm["name"] if fm else None,
        "module_base": "0x%016X" % fm["base"] if fm else None,
        "module_offset": "0x%X" % (addr - fm["base"]) if fm else None,
        "faulting_thread": tid,
        "first_chance": flags,
        "parameters": ["0x%016X" % p for p in params[:nparams]],
        "registers": {k: "0x%016X" % v for k, v in regs.items()},
        "rbp_chain": ["0x%016X" % a for a in frames],
        "stack_scan_exe": ["0x%016X" % a for a in scanned[:400]],
        "stack_low": "0x%016X" % low,
        "stack_bytes_read": len(stack),
        "modules": [[m["name"], "0x%X" % m["base"], "0x%X" % m["size"]]
                    for m in mods],
    }
    if code == 0xC0000005 and nparams >= 2:
        rec["access_kind"] = {0: "read", 1: "write", 8: "execute"}.get(
            int(params[0]), str(params[0]))
        rec["access_address"] = "0x%016X" % params[1]
        try:
            rec["access_offset_from_rsp"] = params[1] - regs["rsp"]
        except Exception:                                       # noqa: BLE001
            pass

    bounds = stack_bounds(proc, regs.get("rsp", 0)) if regs else None
    if bounds:
        base, region_base, region_size, state = bounds
        rec["stack_allocation_base"] = "0x%016X" % base
        rec["stack_committed_from_base"] = region_base - base
        rec["stack_region_size"] = region_size
        rec["rsp_minus_stack_base"] = regs.get("rsp", 0) - base
        if code == 0xC0000005 and nparams >= 2:
            rec["access_minus_stack_base"] = params[1] - base

    with open(os.path.join(outdir, "crash-%d.json" % idx), "w",
              encoding="utf-8") as fh:
        json.dump(rec, fh, indent=2)
    with open(os.path.join(outdir, "crash-%d.stack" % idx), "wb") as fh:
        struct.pack("<QQQQ", low, regs.get("rsp", 0), regs.get("rbp", 0),
                    regs.get("rip", 0))
        fh.write(stack)

    addrs = [addr] + frames + scanned[:64]
    uniq = []
    for a in addrs:
        if a and a not in uniq:
            uniq.append(a)
    syms = symbolize_by_module(mods, uniq)

    def sym_of(a):
        m = module_for(mods, a)
        if not m:
            return []
        return syms.get(m["path"], {}).get(a - m["base"], [])

    def name_of(a):
        m = module_for(mods, a)
        return "%s+0x%X" % (m["name"], a - m["base"]) if m else "0x%016X" % a

    lines = ["crash #%d   pid=%d  tid=%d  first_chance=%d" % (idx, pid, tid, flags),
             "exception : 0x%08X  %s" % (code, NAMES.get(code, "?")),
             "fault ip  : %s" % name_of(addr)]
    if "access_kind" in rec:
        lines.append("access    : %s @ %s  (rsp%+d)" % (
            rec["access_kind"], rec["access_address"], params[1] - regs.get("rsp", 0)))
    if bounds:
        lines.append("stack     : allocation base %s, rsp-base 0x%X, committed 0x%X"
                     % (rec["stack_allocation_base"], rec["rsp_minus_stack_base"],
                        rec["stack_committed_from_base"]))
        if "access_minus_stack_base" in rec:
            lines.append("            access-base 0x%X (%s the stack allocation)"
                         % (abs(rec["access_minus_stack_base"]),
                            "inside" if rec["access_minus_stack_base"] >= 0
                            else "BELOW"))
    lines.append("registers : " + ", ".join(
        "%s=%s" % (k, "0x%016X" % v) for k, v in sorted(regs.items())))
    lines.append("")
    lines.append("--- fault location ---")
    for ln in sym_of(addr):
        lines.append("   " + ln)
    lines.append("")
    lines.append("--- rbp chain (%d) ---" % len(frames))
    for i, a in enumerate(frames):
        s = sym_of(a)
        lines.append("  #%-2d %-50s %s" % (i, name_of(a), s[0] if s else ""))
        for extra in s[1:3]:
            lines.append("        " + extra)
    lines.append("")
    lines.append("--- stack scan candidates ---")
    for a in scanned[:64]:
        s = sym_of(a)
        lines.append("   %-50s %s" % (name_of(a), s[0] if s else ""))
    text = "\n".join(lines) + "\n"
    with open(os.path.join(outdir, "crash-%d.txt" % idx), "w",
              encoding="utf-8") as fh:
        fh.write(text)
    return text


def process_image_name(proc):
    buf = ctypes.create_unicode_buffer(1024)
    size = w.DWORD(1024)
    if k32.QueryFullProcessImageNameW(proc, 0, buf, ctypes.byref(size)):
        return buf.value
    return ""


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", required=True)
    ap.add_argument("--tree", action="store_true",
                    help="also debug child processes (needed for cargo test)")
    ap.add_argument("--max-crashes", type=int, default=1)
    ap.add_argument("cmd", nargs=argparse.REMAINDER)
    args = ap.parse_args()
    cmd = args.cmd[1:] if args.cmd and args.cmd[0] == "--" else args.cmd
    if not cmd:
        ap.error("no command")
    os.makedirs(args.out, exist_ok=True)

    si = STARTUPINFOW()
    si.cb = ctypes.sizeof(STARTUPINFOW)
    pi = PROCESS_INFORMATION()
    command_line = ctypes.create_unicode_buffer(subprocess.list2cmdline(cmd))
    cwd = ctypes.create_unicode_buffer(os.getcwd())
    flags = 0x00000001 if args.tree else DEBUG_ONLY_THIS_PROCESS   # DEBUG_PROCESS
    if not k32.CreateProcessW(None, command_line, None, None, True, flags, None,
                              cwd, ctypes.byref(si), ctypes.byref(pi)):
        print("CreateProcessW failed: %d" % ctypes.get_last_error(),
              file=sys.stderr)
        return 2

    root_pid = pi.dwProcessId
    procs = {root_pid: {"handle": pi.hProcess, "path": os.path.abspath(cmd[0])}}
    print("[crashwatch] root pid=%d tree=%s cmd=%s"
          % (root_pid, args.tree, subprocess.list2cmdline(cmd)), flush=True)

    crashed = 0
    exit_code = None
    while True:
        buf = ctypes.create_string_buffer(1024)
        if not k32.WaitForDebugEvent(buf, 0xFFFFFFFF):
            print("WaitForDebugEvent failed: %d" % ctypes.get_last_error(),
                  file=sys.stderr)
            return 2
        raw = buf.raw
        evcode = struct.unpack_from("<I", raw, 0)[0]
        pid = struct.unpack_from("<I", raw, 4)[0]
        tid = struct.unpack_from("<I", raw, 8)[0]
        cont = DBG_CONTINUE

        if evcode == EXCEPTION_DEBUG_EVENT:
            code = struct.unpack_from("<I", raw, 16)[0]
            addr = struct.unpack_from("<Q", raw, 32)[0]
            nparams = struct.unpack_from("<I", raw, 40)[0]
            params = list(struct.unpack_from("<15Q", raw, 48))
            first = struct.unpack_from("<I", raw, 168)[0]
            fatal = code not in BENIGN and not (code & 0x80000000 and not first)
            if fatal and crashed < args.max_crashes:
                crashed += 1
                info = procs.get(pid, {})
                proc = info.get("handle")
                print("[crashwatch] FATAL 0x%08X at 0x%X (pid %d tid %d first=%d)"
                      % (code, addr, pid, tid, first), flush=True)
                if proc:
                    try:
                        print(dump_crash(args.out, crashed, proc, pid, tid,
                                         (code, first, addr, nparams, params),
                                         info.get("path", cmd[0])), flush=True)
                    except Exception as exc:                    # noqa: BLE001
                        print("[crashwatch] dump failed: %r" % (exc,), flush=True)
                    k32.TerminateProcess(proc, 1)
        elif evcode == CREATE_PROCESS_DEBUG_EVENT:
            hfile = struct.unpack_from("<Q", raw, 16)[0]
            hproc = struct.unpack_from("<Q", raw, 24)[0]
            if hfile:
                k32.CloseHandle(w.HANDLE(hfile))
            procs[pid] = {"handle": w.HANDLE(hproc),
                          "path": process_image_name(w.HANDLE(hproc))}
        elif evcode == LOAD_DLL_DEBUG_EVENT:
            hfile = struct.unpack_from("<Q", raw, 16)[0]
            if hfile:
                k32.CloseHandle(w.HANDLE(hfile))
        elif evcode == EXIT_PROCESS_DEBUG_EVENT:
            code_out = struct.unpack_from("<I", raw, 16)[0]
            if pid == root_pid:
                exit_code = code_out
            procs.pop(pid, None)

        if not k32.ContinueDebugEvent(pid, tid, cont):
            break
        if evcode == EXIT_PROCESS_DEBUG_EVENT and pid == root_pid:
            break

    print("[crashwatch] exit_code=%s crashes=%d"
          % ("0x%X" % exit_code if exit_code is not None else "-", crashed),
          flush=True)
    return 0 if not crashed else 3


if __name__ == "__main__":
    sys.exit(main())

