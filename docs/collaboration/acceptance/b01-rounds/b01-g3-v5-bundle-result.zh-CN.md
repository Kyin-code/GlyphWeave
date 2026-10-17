# B01-G3 结果：V5 两段路束实验（无增量）

* 任务：`docs/collaboration/ds-v41f-b01-g3-v5-bundle-task.zh-CN.md`；先读的独立审查：`docs/collaboration/b01-g2-r1-design-inspection.zh-CN.md`
* HEAD：`fb36dc4f9fdd38e6aa9c5a89676a65b96e5001b1`；工作区 `G:/difyllmwiki/GlyphWeave`
* **状态：`experiment_complete_no_increment`**（任务失败分支），`bestLegalCandidate = null`，`incomplete = false`。
* 本轮**未** bake、未改 carve/保护合同/求解器/生产默认/公开配置、未新增公开开关、未跑 128/80clock/carve 诊断、未跑 CLI/gate/浏览器/最大图/workspace、无 Git 写操作、无 agent、未动 F/A 与旧证据。

## 1. 冻结几何与基线

* 路束：`A=(288,96)` 固定入口 **8000mm**（由既有节点实测，非硬编码）→ `B=(288,180)` 新增共享节点 → `C=(176,180)` 新终点。
* 两段均 12m `local`/urban 已授权、shoulder 3m、`junctionLengthM=8`、`maxFillMm=6000`、`maxRiseMmPerM=80`；A→B 长 84m，B→C 长 112m。
* 冻结输入：`bevy/work/collaboration/ds-b04-c1-r2-b02-w1/w1-collector-local.manifest.json`，SHA256 `930bdc13c6712cbe0dff120c914b366654da5425170e2d35d75b104c2a474e80`（与 `ds-b01-l1-supply` 副本逐字节相同）。
* 重建确认：**35 边 / 27 节点 / 21 义务**；`NaturalSource::AuthoredSeed { seed: 42, precise: true }`（真实 manifest 声明，未用 Undeclared/seed0/采样猜来源）；需求仍为 **24**（16 homes + 8 shops）。
* 基线实现（**仅 1 次**）：`frontage_requested = 24`、`realizedWithAccess = 0`、`finalSource = production-encoder`、`yardUnresolvedM2 = 13.0`，按 parcel 为 **9.0 + 4.0**（两块旧地块；与历史 9+4 交叉核对一致，未硬编码为豁免额度）。
* 基线 `plan` 的 blake3：前后均为 `f9fff91cb7cfc24a740ce5663cfbaf090ebced8a7dda91a1d26361c6ed35520e` → **整束回退无泄漏**。

## 2. 预注册表（先写表后评估，`wx` 拒覆盖）

`bevy/work/collaboration/ds-b01-g3-v5-bundle/candidate-table.json`（sha256 `900842c800494f6778a48a069ff592dcaa14dd3d6ae77a15c570ad1d99e21451`；harness 内部 blake3 `6e1787fe5ea5ef933a682e339c8b01bcd928657dfab2b7419ade455795699ded`）。
表在评估前由 harness 用 `create_new` 写出（已存在且**内容不同**则拒绝；内容相同视为重启并记 `tablePreExisting=true`），C 的派生规则与异常分支同时冻结，状态上限 12。

| 状态 | B 高程 | 出处 |
|---|---:|---|
| S01 | **2560mm** | 生产 `AlignmentContext::infer_end`（接入段最小可行平台） |
| S02 | 1920mm | G2-R1 披露的三带下界 |
| S03 | 7692mm | G2-R1 披露的三带上界 |
| S04 | 1812mm | 冻结自然面在 B 横断面的最大值（fill-only 最小平台） |
| S05 | 2471mm | 冻结自然面在 8 个 V5 窗口上的最大值 |

C 一律由生产叶子派生：街面请求以 `start_level_mm = Some(B)` 固定、`end_level_mm = None` 交给 `infer_end`。

## 3. 执行结果（append-only，两次运行累计 10 个状态尝试）

| 运行 | 状态尝试 | 候选完整 realize | 基线 realize | 用时 | 结果 |
|---|---:|---:|---:|---:|---|
| run 1（`run.log`/`run.json`） | 5 | 0 | 1 | 8.04s | 5×design 拒绝 |
| run 2（`run-r2.log`/`run-r2.json`，含共享路口逐格证据） | 5 | 0 | 1 | 7.94s | 5×design 拒绝 |

两次运行合计 **10 个 attempted bundle states**（上限 12），候选完整 realize **0 次**（上限 3），基线 realize **每个进程各 1 次**（进程内上限 1）。两次运行均在 180s 预算内，`incomplete = false`。运行 2 的表已存在且内容一致（`tablePreExisting = true`），即两次使用**同一张**预注册表。

### 3.1 逐状态首拒绝

| 状态 | B | 阶段 | 首拒绝（生产原文） |
|---|---:|---|---|
| **S01** | 2560 | `street_design_refused`（**匹配类**） | `extension overlaps road outside a matching endpoint junction` |
| S02 | 1920 | `approach_design_refused` | `corridor infeasible at station 76: required 2560mm exceeds 1920mm cap` |
| S03 | 7692 | `approach_design_refused` | `extension padded footprint violates natural fill envelope` |
| S04 | 1812 | `approach_design_refused` | `corridor infeasible at station 76: required 2560mm exceeds 1812mm cap` |
| S05 | 2471 | `approach_design_refused` | `corridor infeasible at station 76: required 2560mm exceeds 2471mm cap` |

分类统计：设计拒绝 **5**（其中 S01 属**匹配类**拒绝，其余 4 个为接入段工程拒绝）、测量零地块 **0**、实现拒绝 **0**、提交 **0**。也就是说：**没有任何状态走到测量/实现阶段**，正路径 `not_exercised`，不存在被伪造的自然正例。

### 3.2 S01 的共享路口逐格证据（本轮关键观测）

S01 是唯一在接入段可行的 B（见 §3.3）。街面段被拒后，harness 用生产的 `AlignmentContext::design` 取出街面 profile，并与已暂存接入段的 profile 在支撑交集 **`[279,171,292,184]`** 上逐格比较（`junctionRadius = 9m`、平台 `junctionLengthM = 8m`）：

* 比较 **140** 格，**23** 格高度不匹配；
* 样例：`(279,171)` 街面 `2480mm` vs 接入 `2640mm`；`(280..286,171)` 街面 `2560mm` vs 接入 `2640mm`。

这正是 G2-R1 审查 §3.2 预言的情形：支撑交集半径 9m **大于** 8m 路口平台，第 9 米处两段 profile 已不同，而 `append_one` 要求交集内**逐格** `|Δ| ≤ 0.01mm`。本轮**没有**扩大 `junctionRadius`、**没有**改动第 9 米、**没有**跳过任何相交格，因此该拒绝如实保留为实验结果。

### 3.3 B 维度事实上被钉死

S02/S04/S05 三个低于 2560mm 的候选都在 station 76 被拒：固定根 8000mm 经 8m 平台后最多下降 `80 × 68 = 5440mm`，因此接入段在 B 的**可达下限恰为 2560mm**；S03(7692mm) 则超出自然填方包络。即：在本轮冻结几何与合同下，**B 只有一个可行值（2560mm）**，而该值下街面段被共享路口逐格规则拒绝。

## 4. 信用与承诺（全部未受损）

* 旧实体/parcel/owner/几何/固定入口/access：未丢失、未转移；基线 `plan` 序列化 bytes/hash 前后一致。
* 旧 yard：仍为 13.0m²（9+4），**未恶化**；新 yard 缺额：不存在（无新增 parcel）。
* 需求仍为 24，未按 8 个窗口改写；未宣称 24。
* 未写冻结世界；整束未提交，`bestLegalCandidate = null`。

## 5. 守卫测试（合成，单列）

`worldgen::auto_roads::tests::a_mismatched_shared_bundle_node_is_refused_by_production`（非 ignored，1 passed）：在扁平合成 fixture 上先接一段（沿既有线共线），再以**不同**的 `start_level_mm` 接第二段 → 生产 `append_one` 返回 `extension start junction height mismatch`；且该复核确认暂存副本只多出第一段、基线 `plan` 未被修改。这证明"共享 B 值不一致"无法被偷渡，同时**不**替代本轮自然实验。

## 6. 结论与下一步（不越界）

* **结论**：V5 在当前冻结几何与工程合同下**无法产出一个真实增量**；但失败点是**共享路口逐格高度一致性**（工程/拓扑类），不是"零地块"，也不是实现或审核拒绝。**这不能推断全局无解**，也不自动转设计 B。
* 已登记集合**按唯一状态已耗尽**：B 维度只剩 2560mm 一个可行值，且该值被拒；剩余自由度只有 C/宽度/坐标/合同参数，均**不在本轮授权内**。
* 若仍需此方向，下一步必须先做**新的候选合同设计**（例如显式修改 `junctionLengthM` 或定义一个"共享支路"的版本化合同），并按任务书要求另行提案、由用户批准——本轮**没有**私下扩大半径或改求解器，也**没有**改 carve/保护合同。
* 未完成项：无增量；24 目标未变；B 未签收、C 未推进。

## 7. 证据与成本

目录 `bevy/work/collaboration/ds-b01-g3-v5-bundle/`：预注册表、两次 run 日志与抽取 JSON、守卫测试日志、源码/输入 hash、编译日志、git/doc-tree 日志。

| 项 | 值 |
|---|---|
| bake / 正式世界 | 0 / 0 |
| 128 扫描 / carve 诊断 / 80 点时钟 | 0 / 0 / 0 |
| CLI 世界生成 / gate / release / 浏览器 / 最大图 / workspace | 0 |
| 自然实验状态尝试（累计，两次运行） | 10（上限 12） |
| 候选完整 realize / 基线 realize | 0 / 每进程 1 |
| 实验用时 | 8.04s + 7.94s（各自上限 180s） |
| 生产源码改动 | 仅 `auto_roads.rs` 的 `#[cfg(test)]` 新增（1 ignored 实验 + 1 守卫测试）；其余生产/CLI 源码与 G1 逐字节相同 |
| 旧证据 | candidate-table / run JSON / 旧 receipt 均未覆盖；G2-R1 文档只追加勘误 |

`git diff --check` exit 0；`node scripts/generate-tree.mjs --check` PASS。

## 8. 勘误（B01-G3-R1 收口轮追加，2026-09-18）

独立审查 `docs/collaboration/b01-g3-v5-bundle-inspection.zh-CN.md` 接受本轮的五个负观测，但驳回"B 只有 2560mm / B 轴已耗尽"的结论。本节只追加勘误，**不改旧 receipt、旧候选表与两次 run 文件**。

1. **结论纠正：2560mm 是下界，不是唯一可行值。** `8000 − 80×68 = 2560mm` 只证明接入段在 B 的**可达下界**；1920/1812/2471 低于下界、7692 高于填方包络，只能说明这四个采样点被拒，**不能**推出 `(2560, 7692)` 内其它整数高程不可行。`infer_end` 给的是最小可行平台，不是唯一解证明。检查员已用当前重建库的公开 `road_alignment::design/verify` 与 `road_extensions::append`，在冻结 35 边原 plan/原 natural/原 zone/水体/原 scene 上证明 **B=3000mm 的接入段 design/verify/append 全部通过（35→36 边）**；该探针**没有**运行 B=3000 的街段或完整 realize，所以不得据此宣称街段或 8 地块通过。
2. **正确口径**：**预注册的五点集合已执行完；B 的取值域未穷尽。** 没有证据表明完整 V5 必然不可能，也没有取得真实增量。
3. **完整交集诊断数字纠正。** 旧 §3.2 的"140 格比较 / 23 格不匹配"来自 `(288 − x) as usize` 后对 profile 原始数组 `get`：x=289..292 时站点为负，转 `usize` 后取值为 `None`，那 4 列被静默跳过。生产 `RoadProfile::height_at` 会把端外投影**钳制**到端点而不是跳过。按闭区间 14×14 重算应为 **196 格比较 / 27 格不匹配**，漏掉的 4 个不匹配点是 `(289..292,171)`（approach 2640mm、street 2560mm）。**街段仍被生产拒绝**，9m 支撑 / 8m 平台的风险结论不变；生产容差与 profile 未被改动。
4. **累计预算纠正。** 两次执行的去重状态 **unique 5**、实际尝试 **attempts 10**、重复执行 **5**、candidate realization **0**；**baseline realization = 2，超过授权 1 次**；总耗时约 **8.04 + 7.94 = 15.98s**（在整轮 180s 内）。授权与预算是**整轮**口径，不能用"每进程各 180s / 各 1 次"替代。第二次运行用于补诊断，其 baseline 与状态成本如实计入，不回退、不抹去；旧回执不改，纠正在本收口轮新回执中给出。
5. **保留未观测项。** 五个状态全部停在设计/匹配阶段，**没有任何状态进入测量或 realize**，因此正路径仍是 `not_exercised`；不得用"无增量"代替"测量 0 地块"。
6. 本节未改变任何生产算法、合同、容差或世界输入；也没有据此批准修改 junction/carve 合同。
