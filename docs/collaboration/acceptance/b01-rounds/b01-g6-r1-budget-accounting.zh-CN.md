# B01-G6-R1 执行预算台账（补记）

* 性质：**仅补记**。本轮 0 次 cargo 调用、0 次自然动作；未重放、未覆盖任何旧证据文件。
* 适用范围：`B01-G6-R1` 诊断修复轮的实际执行过程。
* 结论：授权为**一次**原两态诊断补证（≤4 measure / ≤32 attempt / ≤120s），实际执行**四次**自然重放（3 次完整、1 次部分），维持 **budget-noncompliant**；参数相同、状态相同、最终成功都不抵消超额。
* 机器可读台账：`bevy/work/collaboration/ds-b01-g6-r1-budget-accounting/invocation-ledger.json`；字段来源说明见同目录 `derivation.md`。

## 1. 逐次执行

| # | 阶段到达 | 实际调用（自然侧） | 耗时 | 产物 | 结果 | 记录来源 |
|---|---|---|---|---|---|---|
| 1 | **仅编译**，harness 主体未进入 | 全部 0（未生成 `rule-table.json`，日志只有编译告警） | 进程墙钟 0.33s（被杀死）；harness 内部秒数 unknown | `run.log` 当时 186 B，**已被后续调用覆盖** | 被 PowerShell 管道（`Select-Object -First`）提前杀死 | 会话记录 |
| 2 | **完整 harness 主体**（两态、四次 measure、32 行全建）后失败于收尾断言 | 重建 1、验证 append 1、Context/Mask 各 1、`infer_end` 1、design 4、append_one 4、**measure 4 / attempt 32** | 进程墙钟 2.7s；cargo 自报 2.41s；**harness 内部秒数 unknown**（payload 未打印） | `run.log` 当时 4035 B，**已被覆盖**；未写 `run.json` | 测试侧完备性断言失败（AB 剖面长度被误当 113，实际 85 点） | 会话记录（panic 文本与五标志已读出） |
| 3 | **完整 harness 主体**，payload 已打印 | 同第 2 次：design 4、append_one 4、**measure 4 / attempt 32**、`infer_end` 1、准备阶段 4 | 进程墙钟 13.68s；cargo 自报 2.44s；**harness 内部 2.4041826s** | `superseded-schema-defect.run.json`（443864 B）、`.run.log`（470036 B），**原样保留** | 通过后被自检替换：7 个几何字段经 `Result::map` 写成 `{"Ok":…}` | 保留文件（自带 `callCounts`） |
| 4 | **部分**：冻结阶段、验证 append、首态 AB/BC design+append、首次 measure、首行断言 | 重建 1、验证 append 1、Context/Mask 各 1、`infer_end` 1、design 2、append_one 2、**measure 1 / attempt 8** | 进程墙钟 11.51s；cargo 自报 2.19s；harness 内部秒数 unknown | `superseded-nested-assertion.run.log`（3989 B），原样保留；未写 `run.json` | 形状断言键表误含两个本就嵌套的对象字段 | 保留文件 |
| 5 | **完整 harness 主体**，本轮采用 | design 4、append_one 4、**measure 4 / attempt 32**、`infer_end` 1、准备阶段 4 | 进程墙钟 12.08s；cargo 自报 2.45s；**harness 内部 2.412542s** | `run.json`（437783 B）、`run.log`（463506 B） | 通过并采用 | 保留文件（自带 `callCounts`） |

阶段判定依据：第 1 次未生成 `rule-table.json`（该表在 harness 中段写出）；第 2 次 panic 中 `traceCountComplete/sampleBindingValid/geometryValid/rawDetailPersisted` 全为 true、仅 `profilePersisted=false`，而这些标志只有在两态与 32 行全部构建后才可能为真；第 4 次断言紧跟在每次行 push 之后且每行都带嵌套支撑对象，故判定在首行即中止。

## 2. 汇总

| 项 | 值 | 来源 |
|---|---|---|
| cargo 进程调用 | 5（含 1 次编译期被杀） | 会话记录 |
| 执行的自然重放 | **4**（完整 3：第 2/3/5 次；部分 1：第 4 次） | 混合 |
| measure 合计 | 4 + 4 + 1 + 4 = **13** | 混合（第 3/5 次为实测计数，第 2/4 次为代码路径推导） |
| station attempt 合计 | 32 + 32 + 8 + 32 = **104** | 同上 |
| design / append_one 合计 | 各 **14** | 同上 |
| `infer_end` 交叉核对合计 | **4** | 同上 |
| 准备阶段（重建 / 验证 append / Context / Mask） | 各 **4** | 同上 |
| candidate realize / candidate_final_terrain / bake | **0 / 0 / 0**（四次执行均无） | 保留文件 + 代码路径 |
| harness 内部耗时（可读部分） | 2.4041826 + 2.412542 = **4.8167246s**；第 2、4 次 **unknown** | 保留文件 |
| cargo 自报单测耗时合计 | 2.41 + 2.44 + 2.19 + 2.45 = **9.49s** | 混合 |
| 进程墙钟合计 | **40.30s**（每次还编译了当时的 `auto_roads.rs`，非自然阶段耗时） | 会话记录 |
| 授权 | 1 次重放 / ≤4 measure / ≤32 attempt / ≤120s | 任务书 |

**不得只引用采用那次的 2.41s/2.45s**：那只是第 5 次的 harness 内部与 cargo 读数。

## 3. unknown（不用 0 或推测值冒充）

1. 第 1 次：日志正文（除当时打印的 5 行编译告警）与其编译进度；已消耗的编译墙钟。
2. 第 2 次：harness 内部秒数；被覆盖日志 4035 B 中除 panic 文本与五标志之外的其余内容。
3. 第 2、4 次**未打印任何调用计数**，其计数表是代码路径推导，不是实测。
4. 第 4 次：断言具体发生在第几行（仅有“首行”这一代码路径论证）；harness 内部秒数。
5. 五次调用均未记录 CPU 时间、峰值内存、IO 计数。
6. 第 1、2 次的进程墙钟来自会话内计时，缺少可复算文件支撑。

## 4. 与既有文档的关系

`b01-g6-r1-diagnostic-repair-result.zh-CN.md` §7 的表与结论（共 5 次进程调用、自然重放 4 次、其中 2 遍未发布、1 遍被替换保留、1 遍采用）**保持不变**，其回执字节未动；本台账是该节的细化补全：把每次调用的阶段、调用数与三种耗时口径逐条列清，并把无法复原的部分标为 unknown。本目录 `source-bindings.json` 绑定两者的哈希，证明补记未改动它们。

## 5. 结论与后续边界

* 技术修复项已由独立复验通过并关闭；**执行过程维持 budget-noncompliant**，不申诉、不抵消。
* 本轮不再重放自然场景。H1、新 B/C 值、carve 重放、realize、bake 均**未获批准**。
* 后续先使用**已有的完整 profile 与 trace**做离线分析，再提交范围明确的提案。
* B 仍 partial，C 不推进；不签收里程碑。
