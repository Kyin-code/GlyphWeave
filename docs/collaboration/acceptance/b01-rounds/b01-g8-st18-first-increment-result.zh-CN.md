# B01-G8 结果：B=3110 的单次受控实验（无真实增量）

* 任务：`docs/collaboration/ds-v41f-b01-g8-st18-first-increment-task.zh-CN.md`；上游审查：`docs/collaboration/b01-g7-offline-next-increment-inspection.zh-CN.md`
* HEAD：`fb36dc4f9fdd38e6aa9c5a89676a65b96e5001b1`
* **resultClass = `frontage_measure_zero`**（工程阶段通过、测量零预测增益、未进入 realize）。B partial、C 不推进、不签收。
* 本轮自然流程**只运行一次**；旧 B=3000 对照直接引用 G6-R1，未重跑；0 bake、无 CLI/最大图/gate/浏览器、未改生产合同/默认值/旧证据。

## 1. 一次运行内的阶段结果

| 阶段 | 结果 | 证据 |
|---|---|---|
| 冻结基线重建 | 35 边 / 27 节点 / 21 义务，`fixedRootMm = 8000` | `run.json.frozen`、`rule-table.json` |
| 预注册两态 | ST18-R1 = B=3110 下 `infer_end` 派生 C = **−382**；ST18-R2 = C = B = **3110**；去重后 2 态 | `inferEndReturnedMinimum=-382` |
| AB/BC 生产 append | 两态各 AB+BC = **4/4 通过** | `callCounts.candidateStage.appendCalls=4` |
| 共享 196 格接点 | 两态均 **196 比较 / 0 不匹配 / 0 缺失**（rect `[279,171,292,184]`） | `stateRecords[].sharedJunction` |
| 完整 profile | AB 85 点、BC 113 点，逐点等于各自 design；含 road/support mesh 与 hash | `stateRecords[].ab/bcProfileMm` |
| 测量 | 4 次 measure / **32 attempt**，两态预测 **`[0,0]`**，零接受 | `stateRecords[].measure` |
| baseline / candidate realization | **0 / 0**（预测为零不进入 realize） | `baselineRealizations=0`、`candidateRealizations=0` |
| 五标志 schema | `traceCountComplete / sampleBindingValid / geometryValid / rawDetailPersisted / profilePersisted` 全 true | `schema` |

因此 `roadAppend = pass`、`junctionMatch = pass`、`frontageMeasurement = zero_predicted_gain`、`candidateFinalTerrain / development / landcover / access / yard / realization = not_run`。**不能**概括为"station18 工程通过"。

## 2. 目标窗 station18 的实测：no-cut 已解决，首拒绝换了类

生产 profile 实测（不是推算）：两态 `bcProfile[9] = 3110`、**`bcProfile[18] = 2390`**——与 G7 的端点传播预测 2390 完全一致；同时 42/66 仍是 1105 / 147–150（未随 B 变动），R2 的 90 站 = 2070（C 支配）。这既证实"B 只抬饱和段"的判断，也证实"升 B 不是整段均匀抬升"。

| 站点/侧 | 设计 | 支撑自然 min/max@坐标 | 必要高程区间 | 全支撑缺额 | 首拒绝 | 首拒绝点 |
|---|---|---:|---|---:|---|---|
| 18 北 | **2390** | 1726 / 2307@(278,150) | 2307…7726 | **−83（已满足 no-cut）** | `existing_support_changed` | (268,171) before **6502** → after **2390** |
| 18 南 | **2390** | 1773 / 2390@(278,210) | 2390…7773 | **0（恰好满足）** | `existing_support_changed` | (268,188) before **6987** → after **2390** |

即：G7 提出的 +110mm 确实把 station18 的 pad 抬到必要高度（北侧高出 83mm、南侧恰好相等），**no-cut 不再是这两窗的阻塞**；但候选连接走廊的硬支撑台面覆盖了 BC 路带内的既有填方采样，`existing_support_changed` 立即接管。两态、两请求重复同一对格点，故 4 次 attempt 触发同一类。

## 3. 8 窗 × 2 态的首拒绝分布（32 attempt）

| 状态 | no-cut 拒绝 | support-change 拒绝 | 合计 |
|---|---:|---:|---:|
| ST18-R1 (C=−382) | 12 | 4（station18 两格 ×2 请求） | 16 |
| ST18-R2 (C=3110) | 8 | 8（station18 + station90，各两格 ×2 请求） | 16 |
| 合计 | **20** | **12** | **32** |

* station18：**两态都从 no-cut 转为 support-change**（本提案的目标窗）。
* station42 / station66：设计不变（1105 / 147–150），仍是 `natural_above_design_level`（首点缺额 221/13/292/8 与全支撑 797/913/858/1002）。
* station90：R1 仍是 no-cut（设计 −493，缺额 743/887）；R2 的设计升到 2070（C=B=3110 → 2070），改为 support-change，且与 G6-R1 相同的两点 (196,171)/(196,188) before 5074/4265 → after 2070。

**结论：support-change 不是 station90 独有**——它在原 R2 与本次两态的 station18 都出现（同类机制、不同格点），因此 G7 §4"结果乙"成立：下一轮应转为**窄范围类别诊断**，而不是继续加大 B/C 搜索。

## 4. 预测 vs 实测

| G7 预测 | 实测 |
|---|---|
| B=3110 使 station18 设计 = 2390（必要条件） | ✅ 生产 profile `bcProfile[18]=2390`（两态） |
| B 只影响饱和段，42/66 不动 | ✅ 42 仍 1105；66 仍 147（R1）/150（R2） |
| st18 走廊可能触发同类 support-change | ✅ 触发于 (268,171)/(268,188)，两态 |
| 9m 共享路口需在新 B 下重验 | ✅ 196/196 逐格一致、0 缺失 |

## 5. 预算与调用（单一自然运行，如实计入）

| 项 | 值 | 来源 |
|---|---|---|
| 自然 harness | **1 次**（唯一一次；无失败/无中止/无重跑） | `run-reservation.json`（create-new）+ `run.log` |
| 候选状态 | 2（去重后；未创建第三态） | `statesEvaluated=2` |
| design / append / measure | 4 / 4 / 4 | `callCounts.candidateStage` |
| station attempt | 32（4×8） | `stateRecords[].measure.stationAttempts` |
| `infer_end` 交叉核对 | 1 | 同上 |
| 准备阶段 | 冻结重建 1、plan 2、road_extensions 1、append_declared 1、frontage 验证 1、trusted 1、Context 1、Mask 1 | `callCounts.preparationStage` |
| baseline realize / candidate realize / bake | **0 / 0 / 0** | 同上 |
| 叶子内部调用 | `not_instrumented`（未知，不称 0） |  |
| 耗时 | harness 内部 **3.1173241s**；cargo 自报单测 **3.15s**；进程墙钟 4.16s（含编译） | `run.json.seconds`、`run.log` |
| 编译 | 多次 `cargo test --no-run`（单列，不计入自然预算） | `build.log` |
| 合成自检 | G8 schema 契约 1 个 + G6-R1 回归 2 个，全通过（无自然输入） | `preflight-schema.log`、`preflight-helper-regression.log` |

## 6. 结论与后续边界

* 首个真实增量**未取得**；原 16 住宅 + 8 店铺需求不变，也未降低。
* 阻塞从 no-cut 转移到 `existing_support_changed`（本态新增 BC 自身的支撑带被候选连接走廊台面覆盖）。本轮只保存该 trace 的完整字段，**未**另起 carve 诊断、未改 carve/审核/no-cut/fill。
* 下一轮如获授权，方向应是**窄范围类别诊断**（候选连接走廊 carve 与既有路带过渡填方的相互作用），而不是继续抬 B/C；H1 的 +696 与 B+1002 仍未获批准。
* B partial、C 不推进；未发布世界、未跑 CLI 生成/最大图/gate/浏览器；不签收。

## 7. 证据清单

`bevy/work/collaboration/ds-b01-g8-st18-first-increment/`：`run-reservation.json`（create-new 预约）、`rule-table.json`（预注册，本轮首次写入）、`run.log`、`run.json`（含两态完整 design/profile、196 格接点、32 行候选实体与支撑扫描、4 份 raw trace、五标志、预算与 `best`）、四个 `raw-trace-*.json`（自 `run.json` 分包）、`preflight-schema.log`、`preflight-helper-regression.log`、`build.log`、`source-hashes.txt`（SHA256 与 BLAKE3 分列）、`git-status.log`、`git-diff-check.log`、`doc-tree-check.log`、`receipt.json`。

G7 报告已按本轮要求追加勘误（§8）：B 在东端/C 在西端、18m 只是 blend 初值（原 J9-R2 BC 实际 97.626m）、纵向 solver 不含 carve 合成、两旧点自然值 −606/−535mm 可读、冻结 `scene.json` 保留实体几何、北窗上界 7726；旧回执不覆盖。

## 8. 勘误（B01-G9 轮追加，2026-09-19）

独立审查 `b01-g8-st18-first-increment-inspection.zh-CN.md` 验收两态负结果，并列出四项非阻断勘误。本节只追加，**旧 run/receipt 与旧回执文件均不改写、不重跑 G8**。

1. **station18 的 attempt 数写少了。** §2 末"两态、两请求重复同一对格点，故 4 次 attempt 触发同一类"应为：每态 4 次（2 侧 × 2 请求），两态合计 **8 次 attempt**；全文 32 次总表 20/12 的分布本身正确。
2. **station66 并非"完全未动"。** R1 设计仍 147mm，但 R2 为 **150mm**（C 从 3000→3110 后西端坡度传播开始支配）；对应 R2 的首点缺额为 **289 / 5**，完整支撑最大缺额为 **855 / 999**，不能沿用 R1 的 292/8 与 858/1002。本文"42/66 仍是 1105 / 147"的整句应理解为 st42 仍 1105、st66 随态而变。
3. **源码绑定版本标注错位。** `receipt.supersededBindings` 把 G6 的 294054B/`1994ef49…` 误写成 G6-R1 的源码绑定。实际 **G6-R1 回执绑定的是 330309B/`319bdceb5b0a6d414b494a5e471d2dd6344a5175272dcc9bb4d64b9ccd97eb3b`**（G7 的 source 记录亦指该版本）；294054B/`1994ef49…` 是更早的 G6 版本。两个版本必须分别标注，不能串用。
4. **生产前缀的字节口径。** `#[cfg(test)]` 之前的实际 **UTF-8 字节数为 81685**（此前记的 81677 是字符串字符长度，不是字节长度）；前缀 SHA256 `ba6deb959c3e3b913ed2c65a887f4860b1cddf505cfd2ae2fb71758997224d34` 本身没有算错。
