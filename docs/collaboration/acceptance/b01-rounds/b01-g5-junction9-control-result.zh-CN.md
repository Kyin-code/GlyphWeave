# B01-G5 结果：junctionLengthM=9 最小参数对照（工程通过，地块未通过）

* 任务：`docs/collaboration/ds-v41f-b01-g5-junction9-control-task.zh-CN.md`；先读的独立审查：`docs/collaboration/b01-g4-fixed-v5-joint-level-inspection.zh-CN.md`
* HEAD：`fb36dc4f9fdd38e6aa9c5a89676a65b96e5001b1`；工作区 `G:/difyllmwiki/GlyphWeave`
* **状态：`engineering_pass_content_fail`**，`bestLegalCandidate = null`，`incomplete = false`。
* 唯一参数变更：**仅两条新增段**的 `junctionLengthM` 由 8 显式设为 **9**（现 `append_one` 已支持的输入参数）。`junctionRadius = max(junctionLength, width/2+3) = max(9,9) = **9`，与 G4 相同，**没有扩大豁免**；生产默认、solver、审核、共享支路合同、旧 35 路及其 profile 全部未改。
* 无 bake、无 G3/G4 重跑、无 128/clock80/carve/CLI 生成/gate/release/浏览器/最大图/workspace、无 Git 写操作、无 agent、未动 F/A 与旧证据。

## 1. 预注册规则（执行前写入，`create_new` 拒覆盖）

`bevy/work/collaboration/ds-b01-g5-junction9-control/rule-table.json`（sha256 `75900cd8564dcf897e5596790df3e14f4f02a37be3238ef065df8b9184981e89`；harness 内 blake3 `75900cd8…` 同值）。

固定：`A=(288,96)@8000mm`、`B=(288,180)@3000mm`、`C=(176,180)`（坐标不变）、AB 84m/BC 112m、12m local/urban、shoulder 3、fill 6000、rise 80；**两条新段 junctionLengthM = 9**。
端平台范围由行进方向与 `junctionLengthM` 统一推导：向西段 C 端平台 `x=176..185`，加端外 4m padding 的完整范围 **`[172,171,185,189]`**（横向 ±(12/2+3)=±9）。

两态：

| 状态 | B | C | 出处 |
|---|---:|---:|---|
| J9-R1 | 3000mm | **−382mm** | `junctionLength=9` 条件下生产 `infer_end` 的 `returnedMinimum`（未复用旧 8m 的同名值作为"新最小值"，而是重新派生后恰好相同） |
| J9-R2 | 3000mm | 3000mm | `chosenC = B` |

## 2. 结果：两段工程**通过**，地块内容**未通过**

`run.log` / `run.json`；基线路网 blake3 前后一致（`f9fff91c…`）。

| 状态 | AB append | BC append | 共享路口 196 格 | 测量 | 首拒绝 |
|---|---|---|---|---|---|
| J9-R1 | 通过 | **通过** | 196 比较 / **0 不匹配** / 0 缺失 | 进入 | `natural_support_or_fill_limit` |
| J9-R2 | 通过 | **通过** | 196 比较 / **0 不匹配** / 0 缺失 | 进入 | `natural_support_or_fill_limit` |

关键观测（对比 G4 的 26/14 不匹配）：

* 诊断字段（已按毫米整数序列化）：`abEndStation75Mm = 3000`、`bcStation9Mm = 3000`（G4 同位置为 `2.92/2.92` 米，单位 bug 已在勘误中修正）。
* **9m 端平台恰好覆盖 9m 支撑交集**，两侧 profile 在 `[279,171,292,184]` 全部 196 格逐格相等 → 生产 `append_one` 通过，`extension overlaps ...` 不再触发。这正是 G4 唯一阻塞点的解除，且**没有**扩大 junctionRadius。

随后进入既有 frontage 候选测量（两次状态各 2 个请求预算，共 `measureCalls = 4`、每态 16 次站点尝试）：两态 `predictedGains = [0, 0]`、`predictedTotal = 0`，首拒绝均为 **`natural_support_or_fill_limit`**（自然面/填方上限，出现在候选站点层面）。

按任务"测量无增量则拒绝，不跑 realize"：**未运行任何 realize**。`baselineRealizationCalls = 0`（惰性，未触发）、`candidateRealizeCalls = 0`；`bestLegalCandidate = null`；`incomplete = false`；用时 **2.47s**（上限 180s）。

## 3. 结论与边界（不越界）

* **工程层面**：`junctionLengthM=9` 使固定 V5 几何的两段在现行生产合同下**可以同时通过**（196/196 逐格一致，junctionRadius 仍为 9）。这直接否证了 G4 阶段"必须改共享支路合同才能推进"的外推；本轮只改了**输入参数**，没有改任何合同或默认值。
* **内容层面**：两段工程通过后，**地块测量首拒绝为 `natural_support_or_fill_limit`**，即该街面在当前自然面与填方合同下没有可用增量；因此**没有真实增量**，不生成见证，也不能算整体成功。
* 未测项：其它 C 高程（本轮仅 R1/R2 两态）、其它 B 值、以及填方/自然面条件改变后的组合；**不**据此推断全域无解。
* 本轮不继续扩大搜索、不改合同、不转设计 B、不 bake、不签收 B、不推进 C。

## 4. 调用计数（准备阶段与候选阶段分列）

| 阶段 | 计数 |
|---|---|
| 准备（冻结基线重建） | `road_network::plan_with_zones` 2、`road_extensions::append` 1、`append_declared` 1、`trusted_base_contract` 1、`frontage::append` 1、`AlignmentContext::new` 1、`ZoningMask::new` 1 |
| 候选阶段（worker 显式发出） | `infer_end` 1、`design` 2、`append_one` 4、`measure_road_for_request` 4、candidate realize 0、baseline realize 0 |
| 叶子内部调用 | **`not_instrumented`**（未知）：生产函数内部还会各自调用 `plan/design/verify` 等，未插桩，**不声称 0** |

## 5. 交付与预算

目录 `bevy/work/collaboration/ds-b01-g5-junction9-control/`：规则表、run 日志与抽取 JSON、源码/输入 hash、编译与小测试日志、git/doc-tree 日志。

| 项 | 值 |
|---|---|
| 自然 harness 调用 | 1（本轮唯一） |
| unique 状态 / 重复 | 2 / 0（上限 2 / 0） |
| baseline realize / candidate realize | 0 / 0（上限 1 / 2；地块未通过故未触发） |
| 用时 | 2.47s（上限 180s）；编译另计 |
| 小测试 | 2 个（`height_mm` 转换、端平台范围）前置通过 |
| bake / G3 重跑 / G4 重跑 / 128 / clock80 / carve / CLI 生成 / gate / release / 浏览器 / 最大图 / workspace | 0 |
| 生产源码改动 | 仅 `auto_roads.rs` 的 `#[cfg(test)]`（新增 G5 入口与两个最小诊断帮助 + 修复 G4 `Mm` 字段单位）；生产逻辑/默认/审核/求解未改 |

`git diff --check` exit 0；`node scripts/generate-tree.mjs --check` PASS；G4 旧表/run/receipt 与更早回执字节未变。

## 6. 勘误（B01-G6 轮追加，2026-09-19）

独立审查 `docs/collaboration/b01-g5-junction9-control-inspection.zh-CN.md` 接收本轮为**限定规划阶段进展**，同时要求纠正下列记录口径。本节只追加勘误，**旧 rule-table / run / receipt 保持原字节**。

1. **算法名标注错误。** `rule-table.json`（1465 bytes）是**同一份文件**的两种算法值：
   * **SHA256** = `8f2efc8c52315765bb3230eec1584207848cf95bce2f56600a74ea5ccb5d8327`
   * **BLAKE3** = `75900cd8564dcf897e5596790df3e14f4f02a37be3238ef065df8b9184981e89`（harness 内 `tableHash` 用的就是它）
   本文 §1 与回执把 BLAKE3 写成了 SHA256，并称"同值"，这是错的；两者不是同一个值，只是同一文件。新回执分别绑定两种算法。
2. **状态分层必须写清。** 本轮结论应记为：`roadAppend = pass`、`junctionMatch = pass`、`frontageMeasurement = zero_predicted_gain`，而
   `candidateFinalTerrain / development / landcover / accessJointAudit = not_run`。因此**不能**概括为"最终工程全部通过"；
   `candidate_final_terrain` 与 `realize_candidate` 未执行，旧路在新 carve 叠加后的支撑保持、真实 yard/access 均未测。
3. **`firstReason` 只是保存的首个总类。** 它是每条 trace 的第一个 `rejection_details[0].reason`（总类 `natural_support_or_fill_limit`），
   **不代表** 32 次尝试的原因分布；生产 `frontage::support_check` 至少把三种不同情况都归入该总类。具体分布见 G6 诊断。
4. 需求 16 homes + 8 shops 与请求尺寸/insets/setback/顺序/预算保持未变；本勘误未改任何参数、合同或世界输入。
