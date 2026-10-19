# 已知缺陷：偶发访问违例（0xC0000005）

状态：**未修复**。已定位到崩溃点与触发形态，但 127 次定向复现无一命中，没有可验证的修复方案，因此一行代码未改。

## 现象

`cargo test --workspace --no-fail-fast` 偶发 `STATUS_ACCESS_VIOLATION`（0xC0000005），历史频率约 **4%**（Round 41 的 27 次中 1 次；Windows 事件日志 08:46–09:57 还有四次，其中 `glyphweave_core-*.exe` 的一次早于相关改动）。崩溃落在**稳定代码偏移**，不是随机野跳。

## 已确定

1. **不是深栈**。`RUST_MIN_STACK` 二分：`hillside_cut_city` 在 131072 / 196608 崩、**229376 通过** ⇒ 峰值 ∈ (192, 224) KiB；`glyphweave_core` 全部 750 个测试在 262144 起全过。默认 2 MiB 有 ≥8× 余量。
   **修正**：更早的一轮曾报"烘焙链最深 65,072 B"，实测低估约 3 倍，正确量级是 >192 KiB。
2. **崩溃点是 road-network 计划的 JSON 序列化**。把 `0x348a90` / `0x348aeb` 用 PDB 解析，崩溃期构建(`9d0cd79`)落在 `road_network.rs:115` 的 `<BaseProfileRejection as Serialize>::serialize` 函数范围内（`0x348A00..0x348E00`）；`d40c2da` 构建解析到 `<RoadNetworkPlan as Serialize>::serialize`。两侧一致落在同一个 serde 派生序列化模块。
3. **是读非法地址**。WER 保留的一份 09:57 minidump（已被清理，字段已抄录）显示 `0xC0000005`、**读地址 `0xFFFFFFFFFFFFFFFF`**、`rsp` 距栈顶 84 KiB，栈内返回地址符号化后为
   `bake_world → settlement_graph → plan_with_zones_demand → base_edge_profile → verify_impl → serialize`。
4. **仓库零 `unsafe`**（无 `get_unchecked` / `set_len` / `transmute` / `MaybeUninit`），safe 代码不会造出坏指针。输入每次完全相同 ⇒ 4% 只能来自 ASLR / 堆布局 / 并发分配交错。
5. **最可疑的是依赖的 `unsafe`**（`blake3` SIMD、`serde_json`）或编译器，而不是本仓库的逻辑。
6. **ASAN 不可用**：`-Zsanitizer=address` 能编译但运行时 `0xC0000142`（连 hello-world 都起不来）；WER LocalDumps / PageHeap 需要管理员权限，当前会话被拒。

## 复现与取证工具（已入库）

`tools/crash-hunt/`：

| 脚本 | 用途 |
|---|---|
| `crashwatch.py` | 自实现的 dump 捕获（Win32 调试 API），报异常码/IP/被访问地址/寄存器/栈扫描/线程栈边界；`--tree` 可连同 cargo 子进程一起盯 |
| `loop_repro.py` | 定向循环复现，记录每轮结果 |
| `suite_runs.py` | 整包循环，汇总崩溃率 |
| `resolve_crash.py` | 用 VS 自带 `llvm-symbolizer` 读 rustc PDB，把 WER 的 `Fault offset` 解析成 `模块::函数:行` |
| `mdmp_read.py` / `pe_bytes.py` / `frame_sizes.py` / `find_recursion.py` | minidump 读取、PE 头与时间戳、栈帧尺寸、递归扫描 |

关键用法：

```powershell
python tools\crash-hunt\resolve_crash.py bevy\target\debug\deps\hillside_cut_city-<hash>.exe 0x348aeb
python tools\crash-hunt\crashwatch.py --tree -- cargo test --workspace --no-fail-fast
```

**重要方法论提醒**：偏移只对**崩溃当时的那个 build** 有意义（PE 时间戳必须对上，否则解析出的是错答案）；`llvm-symbolizer --functions=short` 会把泛型压成 `impl$0`，看起来像无关闭包。

## 下一步（按优先级）

1. 用 `crashwatch.py --tree -- cargo test --workspace --no-fail-fast` **连跑 30–50 次整包**，命中后 `crash-N.txt` 直接给出故障指令的 `模块::函数:行` 与完整符号化栈——那是"谁在用坏指针、坏指针从哪来"的分水岭。
2. 安装 nightly 后用 `-Zsanitizer=address`（需匹配的 `clang_rt.asan`），或取得管理员权限配置 `WER LocalDumps`（`DumpType=2`）。
3. 在拿到 1 或 2 的证据前**不要**用 `RUST_MIN_STACK`、`catch_unwind` 或放宽断言来"修"它——那只会掩盖缺陷。

## 对当前验收的影响

该缺陷会让整包测试**随机变红**（约 1/27），因此任何"套件全绿"的结论都应以**多次连续运行**为准，而不是单次。Round 41 的默认环境整包结果是 `969 passed / 0 failed / 34 ignored`，属单次通过。
