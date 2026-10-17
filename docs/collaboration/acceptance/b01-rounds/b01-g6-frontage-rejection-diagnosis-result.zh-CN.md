# B01-G6 结果：G5 两态 frontage 拒绝逐站诊断

* 任务：`docs/collaboration/ds-v41f-b01-g6-frontage-rejection-diagnosis-task.zh-CN.md`；先读的独立审查：`docs/collaboration/b01-g5-junction9-control-inspection.zh-CN.md`
* HEAD：`fb36dc4f9fdd38e6aa9c5a89676a65b96e5001b1`；工作区 `G:/difyllmwiki/GlyphWeave`
* **状态：`diagnosis_complete_reason_identified`**；无参数变更、无新高程、无 fill 放宽、无 realize、无 bake。
* 本轮只做一次**有界诊断重放**（与 G5 历史运行分开计数），保留全部生产 trace；B partial、C 不推进、不签收、不生成世界。

## 1. 预注册与固定输入

`bevy/work/collaboration/ds-b01-g6-frontage-rejection-diagnosis/rule-table.json`（一次 `create_new` 写出；sha256/blake3 由新回执分别绑定）。两态沿用 G5 登记值：`J9-R1` B=3000 / C=−382、`J9-R2` B=3000 / C=3000；`junctionLengthM=9`、width 12、shoulder 3、fill 6000、rise 80、A=(288,96)@8000、C 坐标 (176,180) 全部不变。
一次 `infer_end` 交叉核对返回 **−382mm**，与 G5 记录的 R1 一致（记录为 `crossCheckedInferMinimum`，未据此另建状态）。

冻结 manifest `930bdc13…`、35 边/27 节点/21 义务、请求尺寸/insets/setback/顺序/预算均未变；基线路网 blake3 前后一致（`f9fff91c…`），BC 是本轮唯一测量道路。

## 2. 32 条生产 trace 的真实原因分布

每态 2 请求 × 4 站 × 2 侧 = 8 次尝试/请求，总计 **32 条**；`sample_cap = 8` 使 `samples` 与 `rejection_details` 都完整保存（四次测量全部 `detailComplete = true`，8 samples / 8 details）。

| note（生产 `support_check` 的真实子类） | 次数 | 出现位置 |
|---|---:|---|
| `natural_above_design_level` | **28** | J9-R1 全部 16 次；J9-R2 的 12 次（station 18/42/66） |
| `existing_support_changed` | **4** | J9-R2 的 station 90（x=196，两侧 × 两请求） |
| `fill_exceeds_roadbed_limit` | **0** | — |
| `fill_exceeds_zoning_capacity` | **0** | — |

总类 `natural_support_or_fill_limit` 覆盖前 28 次；另有 4 次是支撑变化类。因此 G5 保存的 `firstReason` 只是首个总类，**不代表** 32 次的原因分布。

按 state/request 汇总（`perRequest`）：

| state | request | attempted | accepted | rejected | note |
|---|---|---:|---:|---|---|
| J9-R1 | b04c1-homes | 8 | 0 | `natural_support_or_fill_limit` × 8 | `natural_above_design_level` × 8 |
| J9-R1 | b04c1-shops | 8 | 0 | 同 × 8 | `natural_above_design_level` × 8 |
| J9-R2 | b04c1-homes | 8 | 0 | `natural_support_or_fill_limit` × 6 + `existing_support_changed` × 2 | 6 + 2 |
| J9-R2 | b04c1-shops | 8 | 0 | 同 × 8 | 6 + 2 |

**与 G5 记录一致性**：每态 16 次尝试、每请求 8 次、全部 `accepted = 0`（故预测增量仍为 `[0,0]`）、首拒绝总类 `natural_support_or_fill_limit` 均一致；**无差异**。

## 3. 28 次 `natural_above_design_level` 的定量特征

每次违规都给出了生产 `at / naturalMm / designMm`（来自 `RejectionDetail`）与 `requiredFillMm = design − natural`（**负值表示需要挖方**）：

| 站点（centre） | at | natural | design | requiredFill | 解释 |
|---|---:|---:|---:|---:|---|
| st18 (270,157)/(270,203) | (275,150)/(272,208) | 2285 / 2283 | 2280 / 2280 | **−5 / −3mm** | 仅差毫米级 |
| st42 (246,157)/(246,203) | (238,150)/(242,196) | 1326 / 1118 | 1105 / 1105 | −221 / −13mm | |
| st66 (222,157)/(222,203) | (214,150)/(217,196) | 439 / 155 | 147 / 147 | −292 / −8mm | |
| st90 (198,157)（仅 J9-R1） | (190,150) | 203 | −493 | **−696mm** | 最大缺额 |
| st90 (198,203)（仅 J9-R1） | (190,196) | −332 | −493 | −161mm | |

其它关键读数：

* **道路填方上限**（`plan.max_roadbed_fill_m = 6m → 6000mm`）在 32 次尝试中**从未成为约束**（0 次 `fill_exceeds_roadbed_limit`）。
* **实际 zone 填方容量**在这些点上为 88–117 米（88000–117000mm，来自 `ZoningMask::fill_capacity_at = 保护距离 × 0.5m`），同样**从未成为约束**（0 次 `fill_exceeds_zoning_capacity`），`zoneCapacityExcessMm` 普遍为 −9 万毫米级。
* 因此唯一生效的规则是**no-cut（自然面不得高于设计面）**：街面中线所在的自然面低于平台，而 23m 外的**地块外侧点自然面略高**（毫米级到 0.7m），使 pad 落在自然面之下。
* `naturalMinMaxScope`：trace 里的 min/max 来自**完整 parcel**（生产样本）；`naturalMinMaxSupportRangeAux` 是测试侧按 `building + 2m padding` 范围另读的辅助值，两者已分别标记，未混用（生产 `support_check` 用的是后者）。
* 4 次 `existing_support_changed` 的 `at` 为 (196,171)/(196,188)，`RejectionDetail` 无 natural/design 字段 → 记为 `null`，并保留 zone 容量读数，不伪造 0。

## 4. 下一步最小候选调整建议（只建议，本轮不执行）

**待验证假设 H1**：把**街面平台整体抬高**（等价于提高共享 B，使 BC 的 pad 整体上升）**≥ 观测到的最大缺额 696mm**，可消除这 28 次 no-cut 拒绝——因为它们的 `requiredFill` 全为负且幅度 ≤696mm，而填方侧的两条上限（6000mm 路床、88–117m zone 容量）从未被触及。

**影响与必须先算清的事**：

1. 提高 B 会改变 AB（从固定根 8000mm 下降）的纵向 profile 与所需填方，必须重新检查 AB 的自然/填方包络与 9m 共享路口的逐格匹配（G5 已证 B=3000 的匹配，B 变化后需重验）；
2. 4 次 `existing_support_changed`（station 90，x=196）属于**拓扑/支撑**类，单靠抬高程**未必**能解，需单独诊断；
3. 本轮**没有**证据支持提高 fill 上限、削平自然地形、缩小建筑/院落或放宽保护——这些都不在建议内；
4. 本轮只登记**一**条建议；是否进入下一轮、以何范围、是否改参数，仍待审批。禁止据总类就提高 fill 或改合同。

## 5. 计数与预算

| 项 | 值 |
|---|---|
| 自然 harness 调用 | 1（本轮唯一；与 G5 历史运行分开计数） |
| 候选状态 | 2（无新取值、无重启、无重复） |
| measure 调用 | 4；站点尝试 32 |
| 候选阶段其它调用 | `infer_end` 交叉核对 1、`design` 2、`append_one` 4 |
| 叶子内部调用 | `not_instrumented`（未知，不声称 0） |
| baseline realize / candidate realize | 0 / 0 |
| 用时 | 2.38s（上限 120s）；编译另计 |
| bake / G3 / G4 重跑 / 128 / clock80 / carve / CLI 生成 / gate / release / 浏览器 / 最大图 / workspace | 0 |
| 生产源码改动 | 仅 `auto_roads.rs` 的 `#[cfg(test)]`（新增 G6 诊断入口与两个最小辅助函数）；生产 frontage/frontage_supply/solver/审核/公开接口未改 |

`git diff --check` exit 0；`node scripts/generate-tree.mjs --check` PASS；G5 旧表/run/receipt 与更早回执字节未变。

## 6. 勘误（B01-G6-R1 轮追加，2026-09-19）

独立审查 `docs/collaboration/b01-g6-frontage-rejection-diagnosis-inspection.zh-CN.md` 判定本轮 **needs-changes**：28 次 `natural_above_design_level` 与 4 次 `existing_support_changed` 的**生产首拒绝分布**、零接受、未 realize 这些观测保留，但**本文的辅助几何证据与 H1 量化结论作废**。本节只追加勘误，**旧 rule-table / run / receipt 保持原字节**。

1. **候选实体绑定错误（P1）。** 本文 32 行的 `buildingFootprintM`（全写 112×12）、`supportCheckRange`（全写 [174,172,290,188]）与 `naturalMinMaxSupportRangeAux` 来自 `sample_entity_for` 返回的 **BC 道路**而非候选建筑。真实请求建筑是 12×10，各 station/side 有各自中心与支撑范围（北侧 yaw 180、南侧 yaw 0）。因此 §3 中“辅助值”一栏与 §4 的 H1 量化依据均不成立。
2. **H1 撤回。** 696mm 只是生产扫描**首个违规点**的最大差值；全支撑矩形的最大缺额最高为 1002mm（st66 南侧，自然 1149@(230,210) 对设计 147），28 条 no-cut 中有 20 条超过 696。并且“提高 B 等价于 BC 所有 pad 均匀抬高”不成立：solver 是逐点下界坡度包络，B=3000 时 station 42 设计 1105，B 抬到 3696 只给出 1056 的下界，推不出 +696。**H1 未获批准、未执行**；§4 的“待验证假设 H1”应按上述限定阅读，不得当作 sufficiency 依据。
3. **原始字段与会话完整性（P2）。** §2 的 per-request 统计丢弃了 `RejectionDetail` 本已产生的 `conflicting_entity`/`before_mm`/`after_mm`；`planProfiles` 只存长度与首末值，不是完整 profile 包；`detailComplete` 只核对数量，不代表几何/绑定正确或原始字段已持久化。
4. 上述三项在 `docs/collaboration/b01-g6-r1-diagnostic-repair-result.zh-CN.md` 与 `bevy/work/collaboration/ds-b01-g6-r1-diagnostic-repair/` 中修正：候选实体改用生产 `frontage::candidate_site` 绑定，首点缺额与全支撑最大缺额分列，四次生产 trace 原样分包保存，两态完整 AB/BC profile 落盘，完整性改为 5 个独立标志。
5. 需求 16 homes + 8 shops、请求尺寸/insets/setback/顺序/预算、自然/分区/旧 35 边均未变；本勘误未改任何参数、合同或世界输入。
