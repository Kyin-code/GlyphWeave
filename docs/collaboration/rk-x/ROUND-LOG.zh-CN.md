# B/C 主线执行日志（自主轮次）

记录规则：每完成一项功能改动 = 一次提交 + 独立测试证据。不以文档数量代替验收。
基线需求见 `../requirements-and-progress-master.zh-CN.md`。

---

## Round 1 · 2026-10-18 · 提交 `0fe6ebd`

**问题**：`baked_audit::joint_program_recovers_housing_minimum_and_preserves_source_obligations`
期望 18 户、实际 1 户。

**实证**（不是读文档推断）：

| 事实 | 数值 / 出处 |
|---|---|
| 冻结夹具生成的世界 | `bevy/work/rk-x/joint-a01`（512m，29 边，22 站点） |
| 带 frontage point / 分级基准的站点 | **0 / 22** |
| 已实现住宅的连通情况 | 17 个 `connected=true`，但 **roadId 全部不是被指定的那条** |
| 基础边 profile 形状 | 1000 → −600 → 1000 mm（1.6 m 落差） |
| 引道可承载落差 | `WALKABLE_GRADE × ENTRANCE_APRON_RUN_M` = 0.25 × 2 = **0.5 m** |
| 受控实验：拒绝全部基础边分级 | **18 / 18 通过** → 病根是分级，不是垫层规则 |
| 锥形夹具每条边最小落差 | **0.627–0.640 m**（5% 坡），同样超过 0.5 m |

**改动**：`road_network.rs` 新增生成侧闸门。有聚落需求的场景里，若一条基础边的分级使其
24×24 路边垫层在任何站位都无法被 2 m 引道进入，则该边分级被**具名拒绝**
（`grading_leaves_no_enterable_pad`，带实测最小落差），并保持平坦的既有 `level`。
无需求的纯规划场景保留全部几何合法分级（其路边扫描是投机性的）。

**为何只在有需求时生效**：全场景生效会改变 `c3_base_identity_is_provenance_and_the_contract_locks_profiles`
与 `w4_chain_digest_and_golden_summary_are_stable` 两条金标，而 T-0013 明令"牵动金标即停下"。
两次尝试（给中心退化点补 frontage → 0/18；全场景生效 → 5 个 core 测试红）都被实测否决并回退。

**独立测试**：`baked_audit` 26/26（第 7 条 18/18）；core lib 626/626，29 ignored；两条金标未改。

---

## Round 2 · 2026-10-18 · 提交 `7a713c2`

**问题**：`morphology_compile::the_real_binary_binds_and_rejects_a_tampered_effective_document`
无法烘焙控制世界。

**根因**（T-0012 已定位，本轮核实）：`assets/objects/storefront.object.toml` 要求立面到路
≤ 8 m 且 `fallback = "skip"`；夹具给商铺的 `setbackM = 12`，立面间隙约 14 m，必然违反。
`engineered_placement::reconcile` 把**任何**带 `frontage_road_point` 的实体当 fixed，
命中拒绝直接 `Err`，不读描述符自己的 fallback，因此一个小卖部让整张世界烘不出来。

**改动**：`engineered_placement.rs` 遵守 `fallback = skip`——**只有** `generated.` 前缀、
非地标、非基础设施、且描述符声明 skip 的实体可被跳过；跳过写入 `skips[]`（entityId/kind/
round/reason/descriptorFallback）并把 `removedItems` 计入报告。authored 实体、地标、
道路/桥梁/铁路/运河/堤道/人行道，以及要求 move/shrink 的描述符仍是硬错误。

**独立测试**：
- `morphology_compile` **2/2**（原 1/2）
- `baked_audit` 26/26
- core lib 626/626，29 ignored
- **`cargo test --workspace` exit 0：全部 target 全绿**（44 / 101 / 26 / 1 / 2 / 1 / 2 / 4 / 1 / 626）——
  这是 T-0013 的 AC-1 首次达成。

---

## Round 3 · 2026-10-18 · 真实 1km 世界对照（只读证据，无代码改动）

用 b8 自己的 manifest（`ds-b-mainline-resume-20260929/runs/b8-current-a01/world/glyphweave.manifest.json`）
在当前 exe 上重新生成：`bevy/work/rk-x/b8-with-r13`（512×512 视作 1 km 场景，51.7s，exit 0）。

| 指标 | b8（修复前，文档记录） | 修复后（同输入，本次实测） |
|---|---:|---:|
| road edges | 46 | 46 |
| 有分级 profile 的边 | 20 | **3** |
| `grading_leaves_no_enterable_pad` 拒绝 | — | **23** |
| roadside sites | 22 | 25 |
| 带 frontage point 的站点 | 1 | **5** |
| 有入口的 frontage 实现 | 1 | **3** |
| quality status | fail | warn |

**结论 1**：该闸门在真实场景同样成立——23 条基础边的分级会让自己的路边垫层无法进入，
被具名拒绝后临街实现从 1 提升到 3，quality 从 fail 升到 warn。

**结论 2（更重要）**：`frontageAudit` 显示 **requested 144 / planned 3 / realizedWithAccess 3 / unfilled 141**。
即真实 1 km 世界的临街供给只有需求的 **2%**，瓶颈已经不在"垫层可进入性"，而在
**frontage 候选供给**（`frontage_supply.rs`）：绝大多数请求连候选都没找到。
这是下一轮 B-01 的主攻点，优先级高于继续调垫层。

**额度记账**：本次 1 km 生成 1 次（R11 池 2/6 → 3/6）。调试 exe，不是 release。

---

## Round 4 · 2026-10-18 · frontage 供给诊断（只读证据，无代码改动）

数据源：Round 3 重新生成的 1 km 世界 `bevy/work/rk-x/b8-with-r13/world`
（`road-network-plan.json → frontage.outcomes`）。

| 请求 | count | siteIds | attempted | unfilled | stopReason |
|---|---:|---:|---:|---:|---|
| `urban-homes` | 120 | **0** | 412 | 120 | `no_feasible_frontage` |
| `urban-shops` | 24 | 1 | 458 | 21 | `no_feasible_frontage` |

拒绝原因（两个请求合计）：

| 原因 | homes | shops |
|---|---:|---:|
| `wrong_purpose_or_protected` | **403** | **441** |
| `occupied_or_road_support` | 9 | 7 |
| `natural_support_or_fill_limit` | 0 | 6 |
| `engineered_platform_not_flat` | 0 | 1 |

**结论**：临街供给的瓶颈不是几何、不是支撑、不是填方，而是**用途**——
87–96% 的候选站点落在"非 urban 或受保护"的用地上。
道路（低地骨架网）穿过的地绝大多数是 rural/nature/unassigned，
所以 `urban` 的住宅/商铺请求在站台上无地可用。

这与真实城市过程一致：道路建成后，**沿路地块被细分并转为城市用途**（ribbon development），
而不是先有城市地块再修路。当前生成器缺的正是这一步"沿路地块的显式用途转换"。

**下一轮要做的**（需要项目负责人确认语义）：允许临街请求在**紧邻道路的 rural 地块**上落地，
但必须把转换**显式记账**（该地块的 rural 面积减少、urban 面积等量增加，保护地/水体仍然绝对禁止），
而不是默默把 rural 当 urban 用。这样 B-G3 的三账本仍然守恒，且可审计。
若不接受"用途转换"，替代方案是让 road-first 规划器沿新路**预先切出城市地块**（更慢、更大改动）。

---

## Round 6 · 2026-10-18 · 沿路用途转换的落点定位（只读，无代码改动）

本轮只做一件事：把 Round 4 结论落到具体代码位置，避免下一轮再从零找。

**闸门位置**：`bevy/crates/core/src/worldgen/frontage.rs`

- `envelope_reason()`（:324）先判场景边界（`outside_scene`），再判用途（`wrong_purpose_or_protected`，:338-339）
- 用途判定走 `in_purpose_core(parcel, insets, rotation_y_deg, zones, purpose)`
- 注释（:344-354）已经把这件事说清楚：24×24 住宅中心落在 `road/2 + setback + depth/2`，
  即离路 12–36 m 的一条带；143 m 的 urban zone 被道路走廊切开后，这条带的外缘常常已经在区外，
  于是"建址在 urban、地块矩形跨出区"也被判成 `wrong_purpose_or_protected`。
  — 这条注释本身就是 B01 的历史证据（0/120 住宅）。

**实现方案（待批准语义后执行）**

1. **开关**：`style.frontageProgram.ruralConversion`，取 `"explicit"` 才启用，缺省关闭（旧行为逐字节不变）。
2. **闸门**：`in_purpose_core` 在该开关下，对 `purpose == "urban"` 的请求，额外接受
   "**可建核心完整落在 rural 区**"的地块；`nature` / `unassigned` / 保护 / 水体仍然拒绝。
   返回一个转换记录 `{parcelId, from:"rural", to:"urban", areaM2}`。
3. **记账**：`settlement/landuse_ledger.rs` 在计算行之前应用转换——
   `urban.realizedM2 += area`、`rural.realizedM2 -= area`，并在账本里输出 `conversions[]`；
   三条守恒等式必须仍成立，且 `Σconversions.area == urban 增量 == rural 减量`。
4. **审计**：`world-quality-gate` 增加一条——任何转换不得触及保护/nature/unassigned/水体；
   账本 `conversions` 与 frontage plan 的转换记录必须逐条一致（防伪造）。
5. **测试**：1 km 冻结输入上 `urban-homes` 的 `planned` 必须 > 0 且账本守恒；
   再加负例（保护地/水体地块必须仍被拒绝）。

**风险与边界**：这是"用途转换"，不是放宽几何合同；坡度、填方、退界、引道、保护区一律不动。

---

## Round 8 · 2026-10-18 · 沿路用途转换在真实 1 km 世界上的效果（只读验证）

同一冻结输入 `b8-current-a01/world/glyphweave.manifest.json`，只在 `style.frontageProgram`
里插入 `"ruralConversion": "explicit"`（文件 28,787 → 28,824 字节，删掉这 37 字节后与原文件逐字节相同）。
输出：`bevy/work/rk-x/b8-conv/world`。基线的 gate 退出码在副本 `b8-with-r13-gatecopy` 上取得。

| 字段 | 关闭（基线） | 开启 |
|---|---:|---:|
| `frontageAudit.planned` | 3 | **67** |
| `frontageAudit.realizedWithAccess` | 3 | **67** |
| `frontageAudit.unfilled`（requested 144） | 141 | **77** |
| `urban-homes` siteIds | 0 | **43** |
| `urban-homes` unfilled | 120 | 77 |
| `urban-shops` siteIds / unfilled | 3 / 21 | **24 / 0**（`demand_satisfied`） |
| `frontage.conversions` | 0 | **64 条，Σ 22,492 m²**（484×43 住宅 + 80×21 商铺） |
| `purposeConversionAudit` | 不存在 | planned 64 / applied 64 / convertedM2 22,492 / **pass** |
| 账本 urban realized | 8,648 | **30,988** |
| 账本 rural realized | 168,635 | **141,577** |
| gate 退出码 / 失败条数 | 1 / 7 | 1 / **7（同一组，与转换无关）** |

**守恒（精确，全部差 0）**：每行 `realized + unresolved == plannedM2`；全图
`Σrealized + Σunresolved == 1,000,000`；转换额双向相等——urban `30,988 − 8,496 physical = 22,492`
= rural `164,069 physical − 141,577 = 22,492` = `Σconversions`。gate 未出现
`rural-to-urban purpose conversion is not provable`。

**结论**：沿路用途转换把真实 1 km 世界的临街实现从 **3 栋提到 67 栋**（22 倍），
商铺需求 100% 满足，且账本三账本守恒、转换可独立复核。人工/自然边界未破：
nature 行前后完全不变（162,735 / 162,735 / 0），保护地未被转换。

**剩余缺口（下一轮主攻）**：`urban-homes` 仍差 77 栋，拒绝原因已从"用途"转为工程类：
`natural_support_or_fill_limit 233`、`occupied_or_road_support 90`、`existing_support_changed 21`、
`engineered_platform_not_flat 6`、`wrong_purpose_or_protected` 仅剩 19。
即下一步是**支撑/填方包络**，不是用途。

**额度记账**：1 km 生成 1 次（R11 池 3/6 → 4/6），调试 exe；gate 在副本上跑 1 次（不占额度）。

---

## Round 9 · 2026-10-18 · 支撑/填方拒绝的可复算诊断（无生产语义改动）

诊断对象：`urban-homes` 412 次尝试里的 233 次 `natural_support_or_fill_limit`。
方法：新增 ignored 测试 `support_gate_audit_of_a_published_world`
（`worldgen/frontage_supply.rs`，只读已发布世界，剥掉 frontage 地块后用生产 `frontage::append`
复跑），**逐请求复现了生产结果**（attempted/accepted/rejected/stopReason 全等，`shadowReproducesProduction=true`）。

| 请求 | attempts | accepted | 该族 | 需挖方 | 填方>6m | zoning 容量 |
|---|---:|---:|---:|---:|---:|---:|
| urban-homes | 412 | 43 | 233 | **211（90.6%）** | 23 | 0 |
| urban-shops | 176 | 24 | 79 | 78（98.7%） | 1 | 0 |

关键佐证：233 例在**临街点**上 `natural − level ≤ 0`（0 例路面自身欠挖），问题出在 28 m 平台
向内的 20–38 m；切深 ≤0.5 m 25 例、≤1 m 29、≤2 m 40、≤4 m 94、>4 m 23（最大 6.04 m）；
两侧均衡（neg 103 / pos 130）；候选搜索已穷尽（8192 预算、16 m 间距、双侧全试）。

**结论：这不是实现缺陷，而是"只填不挖"合同与真实横坡相撞。** 全部 233 例中，
平台的临街边常常已经高于设计面（155 例，其中 134 例 ≤0.5 m），深挖量出现在后缘。

**已量化的三条出路（都需授权，故本轮未改生产代码）**：

| 方案 | 改动 | 可救（共 233） | 代价 |
|---|---|---:|---|
| ① 有界挖方 C | `frontage.rs:807` 的 `h > level` 放宽到 `h − level ≤ C`，并让 `apply_terrain_carves` 支持下挖 | C=1m→54、2m→94、4m→188、不限→211 | 新增能力：挖方量台账 + gate 规则；突破"只填高"合同 |
| ② 加长引道 | 只改 `ENTRANCE_APRON_RUN_M`（现 2 m），纯填平台抬到自然最高点 | 3m→159、6m→207 | 会改既有世界输出；引道本身也是受保护数值，需授权 |
| ③ 不动 | — | 0 | 1 km 临街实现停在 67/144 |

**判定**：②在"合同不动"的叙述上是伪命题——`ENTRANCE_APRON_RUN_M` 就在不可动清单里；
所以真实选择只有 ①（有界挖方，最接近真实城市的削坡建台）或接受 67/144 的现状。

**附带发现（未修，记录）**：`frontage_supply::diagnose()` 的注释声称与 `append` 同序，
但 `append` 已改为逐站位轮转；该函数仅测试在用（`#[allow(dead_code)]`），无生产影响，
且本世界的预测（homes 50 / shops 317）与实际（43 / 176）不符。

**验证**：完整 workspace exit 0；core lib 628 passed / **30 ignored**（+1 是本次的只读审计测试）。

---

## Round 11 · 2026-10-18 · 农田共享农道接入（B-03 第一切片）

实现：`style.agricultureProgram.scenes.<id>.trackNetwork = "shared-farm-track-v1"`（默认关闭=现状逐字节不变）。
农道成为合法接入终点，预算按**到最近真实道路的累计米数**算（不是跳数），160 m / 8% / 12% / 4 m
全部未动；审计新增 `verify_access_network`，从已发布路径独立重建归属表并逐字段复算。

1 km 世界实测（`b8-conv/world`，只改策略，terrain/plan/entities 全同）：

| | 关闭 | 开启 |
|---|---:|---:|
| 田块 | 100（45 直达 + 55 绕行） | **103**（+3 共享 spur） |
| rural realized | 141,577 m² | **143,585**（+2,008） |
| rural unresolved | 654,790 m² | **652,782** |
| farm track 面积 | 16,182 m² | 16,462 |
| 旧田块被改写 | — | **0** |

**重要诊断（比功能本身更有价值）**：增量只有 3 块，因为**旧规则下共享已经发生**——
`check_cell` 从不拒绝已铺农道格，实测 100 条车道里 **51 条已与更早车道走廊重叠（11,481 格）**。
所以"以农道为终点"只补搜索完备性，不扩大可达范围。
rural 缺额的真正主因是物理约束：`field_grade_exceeds_12_percent` **264 次**、
`source_too_small_for_requested_field_and_margin`（小 cell）、`existing_land_use_reserved` 26 次；
预算类只有 21 次 + ~80 次（其中 66 次 160 m 内根本没有已实现农道）。

**结论：B-03 的杠杆在"田块尺寸与 cell 划分、坡地可用性"，不在接入终点。** 下一轮应改
`plan_landcover` 的 cell 划分（现在小 cell 直接判 `source_too_small`）与坡地田块形状，
而不是继续扩接入规则。

**验证**：workspace exit 0；core lib **634 passed / 30 ignored**（+5 新测试）；
关闭态序列化 blake3 已锁定为 `57268eb9…`，旧输入逐字节不变。

---

## Round 12 · 2026-10-18 · 顺等高线长条田形状策略（opt-in）

实现：`style.agricultureProgram.scenes.<id>.fieldShapes = "contour-elongated"`
（缺省/其它值 = 关闭，关闭态序列化 blake3 仍是 `57268eb9…`，逐字节不变）。
12% / 8% / 160 m / 4 m 农道 / 2 m 留白 / 16–128 m / 4 m 地头**全部未动**；
新增审计 `verify_field_shapes`，`landcover::verify` 每次都从**已发布矩形 + 最终地形**
独立复算形状、等高线轴、尺寸域、长宽比、留白、地头内缩与逐米坡度，
不再只靠"重新生成一遍再比字节"。

算法：每个 cell 用整数毫米高差和定出主坡向，**长边取垂直于主坡向的那个轴**（平地把平局判给 z）；
阶梯从作者短边降到 16 m，长边 = `min(3×短边, 128, cell 沿等高线可用长度)` 取 4 m 格；
第一步仍是今天的正方形（先入队、先处理），长条只后续补位，所以旧田块**不会被改写**。

1 km 世界实测（`b8-conv/world`，同 terrain/plan/entities，只改策略）：

| | 关闭（=现状） | 开启 |
|---|---:|---:|
| 田块 | 103 | **134**（+31，全是长条；旧 103 块几何/接入逐字段不变） |
| rural realized | 143,585 m² | **167,382**（+23,797） |
| rural unresolved | 652,782 m² | **628,985**（−23,797） |
| cultivated / headland | 26,368 / 32,960 | 34,608 / 44,096 |
| farm track | 16,462 | 20,913 |
| `source_too_small…` | 599 | **563**（−36） |
| `field_grade_exceeds_12_percent` | 264 | 687（长条也是候选，拒绝数随之变大） |
| `existing_land_use_reserved` | 26 | 78 |
| 审计耗时 | 7.0 s | 12.0 s（约 1.7×） |

长条形状分布：3:1 ×14、2.8:1 ×1、2.5:1 ×2、1.25:1 ×3、1:1 ×11（16×16，
cell 沿等高线不足 3×短边时取最大合法矩形，已在代码注释里写明）。

**诊断**：杠杆真实但**上限有限**。653 个 rural cell 的静态上限（忽略接入）
也只有约 15 万 m² 田面（现状 12.1 万），因为 78% 的地面在 12% 以内却**不连续**；
另有 563 个 cell 短边 < 20 m（多为 2–6 m 的走廊碎片），长条也放不下。
跨 cell 合并实测只有 **6 对**（合计约 1.5 k m²），且要为它把 patch 按 source cell 拆开
（`landuse_ledger` 的 "surface outside source boundary"/"source overcredited" 两条硬检查），
收益不抵契约风险，故本轮只做 in-cell 阶梯并保留具名拒绝。

**验证**：workspace exit 0（44 / 101 / 26 / 1 / 2 / 1 / 2 / 4 / 1 / **642** / 0），
core lib **642 passed / 31 ignored**（+8 新测试，其中 1 个是 1 km 已发布世界实测）。
两条金标未改，既有期望值一个未改。

---

## 下一步（Round 13 计划，Round 12 之前的列表仍有效）

1. **垫层模型决策（需项目负责人）**：平板垫层 + 纯填方 + 2 m 引道在坡度 ≥3.6% 的地形上
   数学上不可建（0.5 m ÷ 14 m 窗口）。这解释了 b8 真实 1 km 世界只有 1 个 frontage site。
   三选一：(a) 只做平地；(b) 有界挖方台地（违反现行"只填高"合同，需授权）；
   (c) 垫层随坡 + 阶梯基础（改动最大）。未决前不动这项。
2. **生态缺 1 株**（A 例外 #4）：在 1 km 冻结输入上复现并定位该株的合法候选与拒绝原因。
3. **B-03 用途闭合**：先只做"可数值化"的部分——把 50.23 km² 未实现按 urban/rural/nature
   分项报出 realized/unresolved，不引入新地表类型。

---

## Round 57 · 2026-10-20 · A 例外 #4 关闭（6×10km A-closure）

**目的**：用当前源码重新烘焙 6×10km 最大图，关闭 A 例外 #4（生态缺 1 株），
并给 B-03/B-05 留一份同源码快照的最大图基线。

| 项 | 值 |
|---|---|
| HEAD | `74cdc1ad533c2200b72339c02110f46f10771ab7` |
| manifest | `examples/maximum-6x10km.manifest.json` |
| manifest SHA-256 | `26647877A2F3D18D8D243F0463CBFA3024D93433D3D41C4DE2E391028AC189A9` |
| release binary SHA-256 | `03DD4D35BCC234B30529BC9423013AA67F5F81F4A35010D34B088E3C1A24E338` |
| output | `bevy/work/rk-x/a-exception4-20261020/world/` |

**实测**：

- `generate-world` exit 0，**255.941s**，240 chunks，623 entities；
- `quality-report` status `warn`，rules 623/623，access 156 connected / 0 disconnected，
  scale 240/240 chunks；
- `landcover-audit.json`：**ecologyPlannedPlants 342,380 / ecologyRealizedPlants 342,380 /
  ecologyUnfilledPlants 0**；
- `world-quality-gate` exit 1，**4 条失败**：
  purpose-area、landcover、land-use planning、road-first；
  生态配额项已不在 failures 中。

**同口径账本**：realized 9,461,820m² / unresolved 50,538,180m²。
历史 r49 的 50,230,050m² 是旧源码快照，不能与本轮混用。

**未验证**：第二次确定性 6×10km 重跑、240 chunk 浏览器漫游、偶发 `0xC0000005` 修复。
完整证据见 `A-REBASELINE-20261020.zh-CN.md`。

## Round 58 · 2026-10-21 · V3 审核通过后的自然—聚落首版实烘焙

按项目负责人确认的 V3 八项口径，新增自然基线冻结/最终差分、自动自然节点
多路网、需求驱动建筑原型、分级生鲜市场和入口搜索排除自身建筑。

1km 候选 `examples/natural-evolution-core-city-1km.manifest.json`：

- 207 个自然节点，4 个自动锚点，3 条自动道路全部 pass；
- 25 个需求/地块计划建筑，1200 户、700 岗位和 8000m² 商业全部满足；
- 5 个分级市场计划，最终 4 个市场实体，覆盖约 97.5%；
- 自然基线人工痕 0；最终改造 63,896/1,000,000 格，平均 0.217m；
- scale-audit pass，quality-report pass；
- 仍有 11 个计划建筑入口失败，已按锚点边界/入口台阶/不可达路线具名记录。

完整文档：
`docs/collaboration/rk-x/NATURAL-EVOLUTION-W1-W5-20261021.zh-CN.md`。

后续提交：

- `18f4014`：入口锚点按真实矩形边界落点，平台从整个 footprint 取样；
- `0e7e60d`：入口拒绝迭代收敛，删除级联失效建筑及其孤立引道；
- `f7cc1e0`：旧城窄地块、更新带中等地块、新区宽地块及对应建筑原型；
- `c704ef0`：普通局部合同保持 6m，`feature-cut` 可通过宏观工程授权扩大填挖。

最后一轮 `glyphweave-core --lib`：**866 passed / 0 failed / 34 ignored，
481.42s**。最终控制 `final16`：23 个规划建筑、12 个最终建筑、3 个市场、
800 户需求满足，scale/quality 均 pass。

## Round 59 · 2026-10-21 · 分时代需求预留与全局回填

旧版按道路顺序消费全局需求，更新带会在新区之前耗尽预算。新增
`settlementEvolution.districtDemand`：首轮只使用各 epoch 的住宅、岗位、
商业和服务预算，未使用预算进入第二轮全局回填。阶段份额不产生固定建筑数，
地块合法性、原型和容量仍决定实际建筑。

1km 控制：

- 24 个需求/容量建筑计划；
- 旧城 2 个、更新带 22 个、新区 1 个候选地块；
- 旧城窄街宅、更新带公寓/街面、新区 `new-town-midrise` 均实际出现；
- 旧城未用预算进入全局回填，记录 15 个更新带 spill 地块；
- 住宅 800/800、岗位 700/700、商业 8000/8000m²、服务 20/20；
- 最终 10 栋普通建筑 + 3 个市场，入口连通 13、断点 0；
- `scale-audit pass`、`quality-report pass`。

仍未解决：坡地建筑入口台阶工程、市场与普通建筑同级的入口工程报告，
以及新区站城/街区级公共空间组合。证据见
`DISTRICT-DEMAND-ALLOCATION-20261021.zh-CN.md`。

## Round 60 · 2026-10-21 · 市场失败具名与入口实验撤回

为市场新增 `market-materialization-report.json`，记录计划数、最终保留数以及
每个被 AccessGraph 拒绝的市场位置、容量、负载和状态。1km 控制继续保留
3 个市场，`market.0` 因 `anchor_not_on_building_boundary`、`market.3`
因 `entrance_step_requires_engineering` 具名失败；最终 13 个建筑/市场
全部连通，断点 0。

尝试过给市场增加 frontage pad、边界锚点和短引道，但 5 个市场全部被入口
合同拒绝。坡地市场与道路常相差 5–8m，短引道超过 25% 步行坡；固定切深又
会把市场变成无依据深坑。因此该实验从生产路径撤回，避免以放宽入口阈值制造
假连通。后续必须使用 landing + stairs/switchback 或可审计台地，并纳入统一
工程账本。证据见 `MARKET-MATERIALIZATION-AUDIT-20261021.zh-CN.md`。

## Round 61 · 2026-10-21 · street 2.5D 黑屏修复

运行态复现 `?view=street` 的大面积黑屏。根因是焦点取离地图中心最近的单店，
该店位于东南边缘，固定相机偏移把视锥大半推向世界外侧；同时 resize 未同步
WebGLRenderer 和透视相机。

street 现以住宅/商业/市场/办公/公共空间的整体质心为焦点，使用稳定的斜俯视
相机，`resize()` 同步 renderer、aspect 和 postFX。r6 控制中战略图、street
均可见，13 个建筑/市场连通、断点 0，scale/quality/visual feedback 全部
pass。证据见 `PREVIEW-STREET-CAMERA-FIX-20261021.zh-CN.md`。

## Round 62 · 2026-10-21 · 坡地入口台地与半米量化

入口诊断发现两类问题：整数 `world_y` 把 12.5m 地面向上取整成 13m；聚落
建筑又把 footprint 最高点当作整栋底面。修复后 ground height 以
`worldY + bottomOffsetM` 精确保留，建筑 pad 以道路设计标高为目标，控制
切面声明 `maxPadCutM=12`。短引道不再额外抬高 0.5m 路床。

1km 控制最终普通建筑从 12 提升到 16，市场从 3 提升到 4，实体 3594，
AccessGraph 20 个连通/0 断点，scale/quality/visual feedback 均 pass。
剩余 8 个规划建筑有真实坡地入口高差，具名等待 stairs/switchback/landing
工程，不能靠继续放宽 0.3m 台阶合同解决。

## Round 63 · 2026-10-21 · 市场候选真实占地去重叠

市场计划原先只按路线节点去重，离路后的建筑仍可能重叠；`market.1` 与
`market.4` 曾仅相隔约 7m，40m 以上 footprint 大面积相交。选择器现在计算
最终旋转 footprint，并要求 2m 净距。`market.4` 改选到 `(378,506)`。

r12 控制最终 4 个市场，AccessGraph 20 连通/0 断点，scale、quality 与
visual feedback 全部 pass。市场入口锚点随后改为与旋转矩形求交，
`market.0` 的边界错误消失；其唯一剩余失败原因变为真实的
`entrance_step_requires_engineering`。证据见
`MARKET-FOOTPRINT-OCCUPANCY-20261021.zh-CN.md`。

## Round 64 · 2026-10-21 · 斜向道路 carve 缺口登记

审查确认 1km 道路中有 60 段 45°、25 段 135° 以及少量其他斜向段，但普通
道路 carve 仍只正确处理 0°/90°，其余按 X 向长条近似。这个缺口会影响
flat top、填方过渡和入口 support ownership，不能由当前可读截图或
scale-audit pass 证明为正确。已登记修复切片，下一步先做 oriented
footprint 与 0/45/90/135 门禁测试。

## Round 65 · 2026-10-21 · 香港中环自然—城市机制切面

新增 `examples/hong-kong-central-natural-evolution-1km.manifest.json`，
声明为合成 `feature-cut`，而非香港全域或地理配准重建。场景关系固定为
陡坡山体、旧市场核心、更新台地、新城区和东侧填海水岸。

实现与修复：

- 市场选择同时避让其他市场和规划建筑，并可在道路两侧换边；
- office tower、residential block、商场/夜市、公共广场和绿地进入 support
  carve 白名单；
- 建筑入口从矩形对角交点改为朝向道路的立面中点；
- 2m 门前 landing 使用独立 1m 切方上限，0.30m 入口台阶合同未变；
- 建筑基础取自然支撑与已物化道路面的较高者，交叉路口不再制造假台阶；
- 最终门区 3m 内自然细节持续清理；
- skyline 按视口比例展示完整世界，新增由内陆看海港的 waterfront 预设。

最终控制 `hk-central-natural-evolution-1km-r1w`：

- 2,973 entities / 4 chunks；
- 900 户、2,600 岗位、20,000m² 商业和 36 服务预算全部闭合；
- 35 个计划建筑，28 个通过严格入口物化；6 个分级市场；
- 最终 6 个办公塔、22 个店面、1 个住宅街区、2 districts；
- AccessGraph 35 连通 / 0 断点；
- market planning coverage ≈100%，但 7 个被拒建筑尚未从市场需求点剔除；
- `scale-audit pass`、`quality-report pass`、`visual-feedback pass`；
- 最大跨 chunk 高度差 0.847m；斜向道路 carve 缺口仍按 Round 64 保留。

7 个计划建筑仍被严格入口合同拒绝：4 个
`entrance_step_requires_engineering`，3 个 `entrance_clearance_blocked`。
这些实体带 ID、坐标、锚点、道路点和诊断写入
`settlement-materialization-report.json`，没有通过放宽门槛或隐藏障碍制造
假连通。完整记录见
`HONG-KONG-CENTRAL-MECHANISM-CUT-20261021.zh-CN.md`。

## Round 66 · 2026-10-21 · 入口后市场需求重算

修复市场报告的时序错误：市场计划原先以 35 栋规划建筑为需求，入口审计淘汰
7 栋后仍沿用旧需求。新增 `reconcile_after_access`，固定已物化市场，重新映射
最终 28 栋建筑到 route graph，重算市场负载、最近市场、步行时距、局部供给
密度、缺口和覆盖率。

r2a 控制：

- 市场需求点 35→28；
- 总需求 2,834.23→1,972.88；
- 覆盖率仍约 100%；
- 6 个市场位置不变；
- 原计划保存在 `market-catchment-plan-pre-access.json`；
- `market-materialization-report.json` 同时记录 35 规划需求/28 最终需求；
- scale/quality/visual feedback 均 pass，AccessGraph 35 连通/0 断点。

完整记录见 `POST-ACCESS-MARKET-RECONCILIATION-20261021.zh-CN.md`。

## Round 67 · 2026-10-21 · 门前静态楼梯与真实 AccessGraph 连接

对 0.30–2.40m 的门前局部高差新增静态直梯实体和
`GroundingSpec.stairConnection`。建筑记录门端/地面端、两端毫米标高、
踏步数和宽度；AccessGraph 要求被引用的 `stairs` 实体真实存在，从楼梯地面端
执行原有道路搜索，连通状态写为 `walkable_route_with_stairs`。超过 2.40m
仍拒绝，不使用放宽台阶阈值。

r2h 控制：

- 23 个短梯 + 3 个从 facade 到 frontage road 的长直梯，共 26 个；
- 最终聚落建筑 28→34；
- AccessGraph 35→41 连通，断点仍为 0；
- 15 个普通地面入口路线，26 个楼梯入口路线；
- 严格入口拒绝 7→1；
- 剩余 1 个长梯路径仍被实体/水体/廊道合同拒绝；
- 入口后市场重算需求为最终 34 栋，覆盖约 100%；
- scale/quality/visual feedback 均 pass。

新增回归测试 `linked_stairs_connect_a_stepped_threshold`，确认楼梯不是
AccessGraph 里的假跳过，而是显式路点。完整记录见
`STAIRS-THRESHOLD-CONNECTIONS-20261021.zh-CN.md`。
