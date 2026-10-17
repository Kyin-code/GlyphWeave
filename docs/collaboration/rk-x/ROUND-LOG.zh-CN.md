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

## 下一步（Round 10 计划）

1. **垫层模型决策（需项目负责人）**：平板垫层 + 纯填方 + 2 m 引道在坡度 ≥3.6% 的地形上
   数学上不可建（0.5 m ÷ 14 m 窗口）。这解释了 b8 真实 1 km 世界只有 1 个 frontage site。
   三选一：(a) 只做平地；(b) 有界挖方台地（违反现行"只填高"合同，需授权）；
   (c) 垫层随坡 + 阶梯基础（改动最大）。未决前不动这项。
2. **生态缺 1 株**（A 例外 #4）：在 1 km 冻结输入上复现并定位该株的合法候选与拒绝原因。
3. **B-03 用途闭合**：先只做"可数值化"的部分——把 50.23 km² 未实现按 urban/rural/nature
   分项报出 realized/unresolved，不引入新地表类型。
