# B01-G4 结果：固定 V5 几何的共享路口高程实验（无联合通过）

* 任务：`docs/collaboration/ds-v41f-b01-g4-fixed-v5-joint-level-task.zh-CN.md`；先读的独立审查：`docs/collaboration/b01-g3-r1-closeout-inspection.zh-CN.md`
* HEAD：`fb36dc4f9fdd38e6aa9c5a89676a65b96e5001b1`；工作区 `G:/difyllmwiki/GlyphWeave`
* **状态：`experiment_complete_no_joint_pass`**，`bestLegalCandidate = null`，`incomplete = false`。
* 本轮**只**改 B/C 高程；A/B/C **坐标、街段长度、宽度、platform、fill、坡度、junction、seed/自然/zone/water、35 边/27 节点/21 义务、需求 16+8 与 insets/setback/预算/请求顺序**全部不变。
* 无 bake、无 128/clock80/carve/CLI 生成/gate/release/浏览器/最大图/workspace、无 Git 写操作、无 agent、未动 F/A 与旧证据；未改 carve/solver/profile/保护合同/生产默认/公开接口。

## 1. 预注册规则（执行前写入，`create_new` 拒覆盖）

`bevy/work/collaboration/ds-b01-g4-fixed-v5-joint-level/rule-table.json`（sha256 `54833ee3dcd500935edc4ca40e673ed49753e10f4b19e935f1926bf6f176ea00`；harness 内 blake3 `434a70e71f149932cc49b983d42cf2d5db4029d4f90108f155d41977d425c1dd`）。

几何（固定）：`A=(288,96)@8000mm` → `B=(288,180)` → `C=(176,180)`；AB 84m、BC 112m；12m local/urban、shoulder 3、junctionLength 8、maxFill 6000、maxRise 80。

B 只取两个有来源值：

1. **3000mm**：检查员已用公开生产 `design/verify/append` 证明**接入段**合法（完整束未测）。
2. **2640mm**：由固定根 8000、AB 84m、首端 8m 平台、B 支撑交集须匹配末端 9m 得到的必要下降下界 `8000−80×(84−8−9)=2640mm`；仅作必要候选，不预先声称合法，也未把 junctionLength 改成 9。

每个 B 的 C（坐标恒为 `(176,180)`，只改高程）最多三条规则：

* **R1** 生产 `infer_end` 返回的最小端点为 `−382mm`（`returnedMinimum=−382`，`chosenC=−382`）；
* **R2** `chosenC = B`（同高端点对照，不强制整段平坡）；
* **R3** 必要上界：端部 padding `x∈[172,180]`、`z∈[171,189]`（C 端平台 + 4m padding 全支撑）上 `min(自然 + 实际 zone 填方容量)` 与"从 B 可达上限 `B+80×(112−8)`"取小 → `5324mm`（两 B 相同，因为 zone 填方容量主导）。该值**不是**已证最大可行值。

## 2. 执行结果（1 次 harness 调用，6 个登记状态）

`run.log` / `run.json`；基线路网 blake3 前后一致（`f9fff91c…`），主基线未泄漏。

| 状态 | B | C | 阶段 | 首拒绝（生产原文） |
|---|---:|---:|---|---|
| B3000-R1 | 3000 | −382 | `bc_append_refused` | `extension overlaps road outside a matching endpoint junction` |
| B3000-R2 | 3000 | 3000 | `bc_append_refused` | 同上 |
| B3000-R3 | 3000 | 5324 | `bc_append_refused` | `corridor infeasible at station 104: required 5324mm exceeds 5314mm cap` |
| B2640-R1 | 2640 | −382 | `bc_append_refused` | `extension overlaps road outside a matching endpoint junction` |
| B2640-R2 | 2640 | 2640 | `bc_append_refused` | 同上 |
| B2640-R3 | 2640 | 5324 | `bc_append_refused` | `corridor infeasible at station 104: required 5324mm exceeds 5314mm cap` |

**只有两段都通过生产 append 才进入地块链，因此本轮没有进入测量**：`measureCalls = 0`、`candidateRealizeCalls = 0`、`baselineRealizeCalls = 0`（惰性基线只在首个预测有增量时才计算，本轮不需要）。`incomplete = false`，用时 **2.30s**（上限 180s）。

## 3. 共享路口 196 格诊断（生产 `height_at` 两侧）

| 状态 | 闭区间格数 | 已比较 | 不匹配 | 缺失 | `diagnostic_incomplete` | AB station75 | BC station9 |
|---|---:|---:|---:|---:|---|---:|---:|
| B3000-R1/R2 | 196 | 196 | **26** | 0 | false | 2920mm | 2920mm |
| B2640-R1/R2 | 196 | 196 | **14** | 0 | false | 2640mm | 2560mm |
| B*‑R3 | — | — | — | — | — | — | （街段在 station 104 先被 fill/rise 拒绝，未产生设计交集诊断） |

用生产 profile 可直接解释这 26/14 不匹配的**位置与成因**（`[279,171,292,184]`，`junctionRadius=9m`，平台 `junctionLength=8m`）：

* **B=3000**：AB 沿 `x=288`，其平台覆盖末端 8m（z=172..180），第 9 米（z=171）仍在下坡 → 2920mm；BC 沿 `z=180`，平台覆盖首端 8m（x=280..288），第 9 米（x=279）→ 2920mm。于是 `z=171 且 x≥280`（AB 2920 vs BC 3000）13 格 + `x=279 且 z≥172`（AB 3000 vs BC 2920）13 格 = **26**。
* **B=2640**：AB 在该窗口已到平台（2640mm，含第 9 米），BC 第 9 米仍在下坡 → 2560mm；不匹配只在 `x=279` 的 14 行 = **14**。

即：**两个各 8m 的平台无法满足 9m 支撑交集内的逐格高度相等** —— 拒绝来自现行共享路口合同本身，而不是地块或测量。

## 4. R3 的另一种拒绝

C=5324mm 时街段在 **station 104** 被拒：`required 5324mm exceeds 5314mm cap`（路口平台传播 + 自然/填方上下界），说明这条"必要上界候选"对街段本身不可行（与共享路口无关）。该值仍按必要界给出，**不**当作已证最大可行值，也**不**据此扩大区间。

## 5. 结论与边界（不越界）

* 本轮回答任务的首要问题：**在固定 V5 几何与现行生产求解/匹配合同下，这两个有来源的 B 值与三条 C 规则都不能让两段同时通过**；原因是共享路口 196 格逐格一致规则与 8m 平台之间的结构性差异，以及 R3 的街段内 fill/rise 冲突。**没有进入地块阶段，因此没有任何增量观测**（不是"测量 0 地块"）。
* **不能**由此推断高程域穷尽或全局无解：本轮只登记 2 个 B × ≤3 个 C；检查员已证 B=3000 的**接入段**合法，而完整束在本轮因街段共享路口规则被拒。
* 若下一步仍走这条几何，唯一能改变该结论的是**路口/共享支路合同本身**（例如 junctionLength 或显式共享支路语义）——那必须作为**新的候选参数方案**单独提案、单独审批；本轮**不**实施、**不**声称必须修改、**不**改 carve、**不**转设计 B。
* 已登记小集合按**唯一状态**关闭；未测项明确：C 端更高/更低的其它高程、其它 B 值、以及"合同变更后"的一切组合。

## 6. 交付、预算与计数

目录 `bevy/work/collaboration/ds-b01-g4-fixed-v5-joint-level/`：规则表、run 日志与抽取 JSON、源码/输入 hash、编译日志、git/doc-tree 日志、旧证据未变检查。

| 项 | 值 |
|---|---|
| 自然 harness 调用 | **1**（本轮唯一） |
| unique 状态 / 实际尝试 | 6 / 6（无重复） |
| `infer_end` / `design` / `append` / `measure` / candidate realize / baseline realize（worker 自身调用） | 2 / 6 / 12 / 0 / 0 / 0 |
| 函数内部调用 | 未插桩，**不声称 0**（`append_one`/`design`/`verify` 内部会再次调用生产叶子） |
| baseline plan 前后 blake3 | `f9fff91c…` 一致 |
| 用时 | 2.30s（上限 180s）；编译单列 |
| bake / 128 / clock80 / carve / CLI 生成 / gate / release / 浏览器 / 最大图 / workspace | 0 |
| 生产源码改动 | 仅 `auto_roads.rs` 的 `#[cfg(test)]` 新增 G4 入口（独立于 G3 用例，未自动重跑旧 G3） |

`git diff --check` exit 0；`node scripts/generate-tree.mjs --check` PASS；冻结 manifest `930bdc13…` 与全部旧回执/旧表/旧 run 文件字节未变。

## 7. 勘误（B01-G5 轮追加，2026-09-18）

独立审查 `docs/collaboration/b01-g4-fixed-v5-joint-level-inspection.zh-CN.md` 限定接受六态负观测，同时要求纠正下列表述与记录。本节只追加勘误，**旧表/run/receipt 保持原字节**。

1. **撤回结论外推。** 本文 §3 原写"两个 8m 平台无法满足 9m 交集"、§5 原写"唯一能改变该结论的是路口/共享支路合同"，这超出了六点实验的证据，也与同文"不穷尽/不声称必须修改"矛盾。8m 是**最低**固定平台长度，并不要求第 9 米必然变高；本次确定性 solver 在本次输入上输出的第 9 米有高差，是**实际**失败原因，不能仅由 `8 < 9` 推出所有输入、所有允许 profile 都不可能相等。**正确结论**：登记的六个状态均未联合通过（4 个在共享交集逐格不匹配、2 个在街段 station 104 先工程拒绝）；未测的 B/C 值**仍未被排除**。
2. **`Mm` 字段单位 bug 已修复（旧 JSON 不改）。** 原 `diagnostic[abEndStation75Mm] / [bcStation9Mm]` 直接序列化了 `height_at` 的**米**值：B3000 行为 `2.92 / 2.92`、B2640 行为 `2.64 / 2.56`；按字段约定应为 `2920 / 2920` 与 `2640 / 2560`（mm）。现在统一经 `height_mm()` 转整数毫米，并补了纯转换小测试；196/26/14 计数不受影响（mismatch 样本里的 `approachMm/streetMm` 原本已乘 1000）。
3. **C 端完整平台范围与更紧必要上界。** 原 R3 采样取 `x=172..180`，漏掉 C 端平台内侧 4 米：向西街段（288→176）在 `junctionLength=8` 时 C 端平台覆盖 `x=176..184`，加端外 4m padding 的完整范围是 **`x=172..184`**（不是 172..180）。检查员用冻结 natural 逐点读取并按同一 Chebyshev 保护距离 ×0.5m 的 zone 填方规则复算：完整范围给出**更紧的必要上界 5314mm**（限制点 `(184,177)`，natural −686mm，fillCap 6000），而原部分范围给出的 5324mm（限制点 `(180,178)`）只是**较松的必要上界候选**。两者都不是已证全走廊最大可行值；本轮**没有**据此补跑 G4。
4. **调用计数分列。** 原表 `infer2/design6/append12` 只是**候选阶段**显式发出的调用，**不含**准备阶段（`road_network::plan_with_zones`、`road_extensions::append`、`append_declared`、`frontage::append` 等）以及任何叶子内部调用。原表写成"worker 自身调用"范围过宽；后续报告把准备阶段与候选阶段分列，并把叶子内部调用明确标为 `not_instrumented`（未知），不能因候选 `measureCalls=0` 就宣称整条准备链从未运行。
5. 六态失败与"未穷尽"的边界、以及"未进入测量/realize"的 `not_exercised` 结论保持不变。
