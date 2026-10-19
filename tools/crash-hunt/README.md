# r42-crash：把偶发 `STATUS_ACCESS_VIOLATION` 钉到函数、行、机器指令

本目录是 Round 41「偶发崩溃」调查的现场：工具、原始 dump、循环日志都在这里。
**没有任何东西被提交到 git**（`bevy/work/**` 是工作区产物）。

## 0. 结论速查

| 问题 | 答案 |
|---|---|
| 崩溃偏移 `0x348aeb` / `0x348a90` 是什么 | **只在"崩溃当时的 build"上才有意义**。崩溃 build（PE 时间戳 `0x6AD56321`/`0x6AD56B1E`，08:24/08:58）已被覆盖；用两侧的 git 重建（`9d0cd79` 07:54 与 `d40c2da` 09:40）交叉解析，两者一致地把两个偏移落在同一个 1 KB 函数：`<BaseProfileRejection as Serialize>::serialize`（`worldgen/road_network.rs` 的 serde 派生，三个 `String` 的具名拒绝记录）。**用 HEAD 的 PDB 解析会得到完全不同的答案**（见 §1 的对照表） |
| `glyphweave_core` 的 `0xd9e6bf` 是什么 | 当前 build 上是 `BTreeMap<String, MemberFrame>` 的 `search_tree`（`engineering_connections.rs` 的 `verify_bundle` 查表循环）；该 build 与被覆盖的历史 build 不同，只能作参考 |
| 偏移怎么解析 | Visual Studio 自带 `llvm-symbolizer.exe` 读 rustc 写的 PDB，`--relative-address` 正好吃 RVA（WER 的 “Fault offset”） |
| 崩溃怎么抓 | `crashwatch.py`：用 Win32 debug API 起子进程（可 `--tree` 连 cargo 的子进程一起看），致命异常时抓异常记录 + 故障线程栈并直接符号化。不需要 procdump / cdb / 管理员 |

> **必须用长格式符号名。** `--functions=short` 会把泛型里的类型压成 `impl$0`，
> 于是 `<BaseProfileRejection as Serialize>::serialize` 看起来只是
> `road_network::_::impl$0::serialize` —— 极易误判成"某个闭包"。

## 1. 把偏移变成函数名（可重跑）

```powershell
# 1) 找出 llvm-symbolizer（脚本会自己找 vswhere / G:\vs）
$env:GW_SYMBOLIZER = 'G:\vs\VC\Tools\MSVC\14.31.31103\bin\Hostx64\x64\llvm-symbolizer.exe'

# 2) 解析任意 RVA（等价于 WER 的 Fault offset = fault IP - image base）
python bevy\work\r42-crash\tools\resolve_crash.py `
  bevy\target\debug\deps\hillside_cut_city-d40cc3535583fa28.exe `
  0x348aeb 0x348a90 0x348a80 0x348ad0

python bevy\work\r42-crash\tools\resolve_crash.py `
  bevy\target\debug\deps\glyphweave_core-0582d505d1ea75c7.exe 0xd9e6bf
```

`llvm-symbolizer` 必须配合**同一个 build 的 PDB**（rustc 默认就写在 exe 旁边）。
换 build 后偏移会漂移：`hillside_cut_city` 的两个历史崩溃（`0x348a90` 与 `0x348aeb`）
就是同一处代码在不同 build 上的两个地址，相差 `0x5b`。

### 相邻符号表（当前 build，用符号化器逐 16 字节探测得到）

| RVA | 符号 | 源码 |
|---|---|---|
| `0x348a30` / `0x348a60` | `bounded_cut_ledger_for::{closure}` | `worldgen.rs` |
| `0x348a80` | `retain_shore_clearance::{closure}` | `worldgen.rs:8893` |
| `0x348ad0` | `terrain_disturbance_at::{closure}` | `worldgen.rs:4349` |
| `0x348b90` | `declared_natural_source::{closure}` | `worldgen.rs` |

这一片 `.text` 全是 `sub rsp,N; 存参数; call 辅助函数; add rsp,N; ret` 的小 shim，
所以崩溃落点相差 `0x10`/`0x1b` 就是两个相邻 shim 的第 2~4 条指令。

### 机器码（`pe_bytes.py`，无需反汇编器）

```
0x348ad0  48 83 ec 68            sub  rsp,0x68
0x348ad4  48 89 54 24 38         mov  [rsp+0x38],rdx
0x348ad9  48 89 4c 24 28         mov  [rsp+0x28],rcx
0x348ade  48 89 4c 24 40         mov  [rsp+0x40],rcx
0x348ae3  48 8b 44 24 40         mov  rax,[rsp+0x40]      ; 闭包环境（&[TerrainCarve]）
0x348ae8  48 8b 00               mov  rax,[rax]           ; 切片 data 指针
0x348aeb  48 89 44 24 48         mov  [rsp+0x48],rax      ; ← 历史崩溃偏移
```

## 2. 抓崩溃（可重跑）

```powershell
$env:GW_SYMBOLIZER = '...\llvm-symbolizer.exe'
# 单个测试二进制
python bevy\work\r42-crash\tools\crashwatch.py --out bevy\work\r42-crash\runs\one `
  -- bevy\target\debug\deps\hillside_cut_city-d40cc3535583fa28.exe

# 整包（cargo test 的子进程一起调试）
python bevy\work\r42-crash\tools\crashwatch.py --tree --out bevy\work\r42-crash\runs\suite `
  -- cargo test --workspace --no-fail-fast
```

每次致命异常写三件东西：`crash-N.json`（异常码、故障 IP、模块+偏移、读/写、
被访问地址、寄存器、RBP 链、栈扫描出的返回地址、线程栈分配边界）、
`crash-N.stack`（故障线程栈的原始字节）、`crash-N.txt`（已符号化的可读栈）。
退出码：子进程自己结束 = 0，抓到致命异常 = 3。

没有 procdump / cdb / WinDbg，也没有管理员权限写
`HKLM\...\WER\LocalDumps`，所以用 `CreateProcess(DEBUG_ONLY_THIS_PROCESS|
DEBUG_PROCESS)` + `GetThreadContext` + `ReadProcessMemory` 自己实现；
`MiniDumpWriteDump` 不必要——栈字节 + PDB 已经够符号化。

## 3. 循环复现

```powershell
python bevy\work\r42-crash\tools\loop_repro.py --out bevy\work\r42-crash\runs\w1 `
  --worker 1 --rounds 200 --exe bevy\target\debug\deps\hillside_cut_city-d40cc3535583fa28.exe
```

每轮一行 `loop.log`（耗时、退出码、该轮自己的 `test result:` 行），崩溃时把 dump
目录打出来并以 3 退出。并行起多个 worker 就是并行多份，互不干扰
（测试目录是 `%TEMP%\glyphweave-<label>-<pid>-<nonce>`，不共享）。

## 4. 整包证据

```powershell
python bevy\work\r42-crash\tools\suite_runs.py --out bevy\work\r42-crash\runs\final-suite `
  --runs 3 --cwd bevy
```

`suite.log` 记每次的 cargo 退出码、`test result:` 行；完整输出在 `suite-NN.log`。

## 5. 其它工具

| 工具 | 用途 |
|---|---|
| `find_recursion.py` | 找出源码里自己调用自己的函数（深栈/无限递归排查的第一步） |
| `frame_sizes.py` | 解析 PE `.pdata` 的 unwind 信息，按栈帧大小给函数排序（找大栈帧） |
| `pe_bytes.py` | 按 RVA 打印 PE 里的机器码，并标注常见 x86-64 前缀指令 |

## 6. 证据出处

- Windows 事件日志 `Application Error`(id 1000) 的 `Fault offset` 就是 RVA，
  与本目录的工具完全对得上：`Get-WinEvent -LogName Application |
  Where-Object { $_.Id -eq 1000 -and $_.Message -match 'hillside_cut_city' }`。
- 事件日志里 `0xc00000fd`（栈溢出）那几条都是**人为压小 `RUST_MIN_STACK`** 的探针，
  默认 2 MiB 下没有出现过 `0xc00000fd`；默认环境下出现的是 `0xc0000005`。
