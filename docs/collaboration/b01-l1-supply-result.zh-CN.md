# B01-L1-S1 结果：让道路供给服务于真实地块需求

> **勘误（2026-09-17，B01-SUPPLY-R1 追加；本文件其余内容保持当时原样，不改旧 receipt）**
>
> 1. **33/35 基线**：本文 §4 的 S2 求解把自动路阶段整体替换为 v3，因此实际是从 **33 边**（31 基础 + 2 显式）开始，
>    没有保留冻结的 `generated.road-auto.scene-0.0/.1`。正确基线应为 **35 边**；R1 已改为 `mode:"append"` 的“先跑 v1/v2 再追加”，
>    并断言 35 边/27 节点/21 义务 site 不被改动、新增 ID 用独立命名空间。
> 2. **122 状态的范围**：那是“33 边起点 + 仅 12m 宽度”的候选列表被走完，**不是**“冻结 35 边网络的完整候选空间”。
>    不得据此推断 24 不可达，也不得据此断言“改 carve 是唯一出口”。R1 的 128 状态评估同样未跑完空间（`sweepExhausted=false`）。
> 3. **恒 false 的防倒退条件**：旧 `total < saturating_sub(total, baseline)` 对非负整数永不成立（反例 baseline `[2,0]`→`[1,1]`），
>    因此本轮的“verified”并未真正防止需求回退。R1 已改为按 request ID 对照 + 稳定身份/几何/入口保护。
> 4. **计划信用 ≠ 实现信用**：旧 `verify_supply_commit` 只跑 frontage/development/landcover 计划并返回 `site_ids.len()`，
>    不是最终栅格上的实现链。R1 改为候选工程 final + AccessGraph + `frontage::audit(realized_ids)` + `development::verify`
>    + `landcover::realize/verify`（yard/area）+ 节点高程 + 逐路工程检查，并要求 `planned == realized_with_access`。
> 5. **CLI 测试数量与内部烘焙**：本文 §5 写作“2 + 4 + 1”，实际 CLI 套件为 **43 项 / 7 个 target**，且其中多项在自身 TempDir 内
>    烘焙测试 fixture，不能表述为“全部仅内存、未写任何世界目录”。
> 6. **carve 影响面**：“会改动每一个已烘焙世界”无证据；受影响的是**实际触发该覆盖组合**的世界，发布文件不会自行改变。
> 7. **几何纠正**：(168,220) 相对 auto.0（中心 x=160、宽 12m、yaw 90）横距 **8m**，整平平顶半宽 **7m**、运输足迹半宽 **9m**；
>    正确描述是“整平平顶外 1m、运输足迹内”，不是“9m 整平带外 1m”。
> 8. **诊断预算传递**：旧诊断对每条路都从完整 `count` 与完整 `maxCandidateAttempts` 起算；R1 已让生产与诊断共用累计 `RequestBudget`，
>    并把 attempts/rejections 汇总到**全部**请求（不再只取第一条 trace）。
> 9. 上述修正的完整交付见 `docs/collaboration/b01-supply-r1-result.zh-CN.md` 与 `bevy/work/collaboration/ds-b01-supply-r1/`。

- 任务：`docs/collaboration/ds-v41f-b01-l1-supply-task.zh-CN.md`
- 工作区 / HEAD：`G:/difyllmwiki/GlyphWeave` / `fb36dc4f9fdd38e6aa9c5a89676a65b96e5001b1`
- 状态：**partial（ready_for_review）**；B 未签收，C 未开始，Git 未提交。
- 结论一句话：S1 诊断与 S2“真实供给导向选路”已实现并自测；在**冻结 W1 输入上对既有候选状态空间做了完整（非预算截断）枚举，122 个候选状态中没有任何一条新路能带来一个合法地块**，因此不存在 24/24 见证，**未执行任何新的 512m generate-world**，按 partial 交付并把阻塞定位到具体坐标与合同。

## 1. 输入、哈希与冻结条件

| 项 | 值 |
|---|---|
| 只读来源 manifest | `bevy/work/collaboration/ds-b04-c1-r2-b02-w1/w1-collector-local.manifest.json` sha256 `930bdc13c6712cbe0dff120c914b366654da5425170e2d35d75b104c2a474e80` |
| 本轮副本 | `bevy/work/collaboration/ds-b01-l1-supply/w1-collector-local.manifest.json`（与来源逐字节相同，已复核） |
| 来源 HEAD | `fb36dc4f9fdd38e6aa9c5a89676a65b96e5001b1` |
| 源码起始哈希 | `bevy/work/collaboration/ds-b01-l1-supply/source-hashes-start.txt` |
| 冻结世界（只读引用） | `bevy/work/collaboration/ds-b04-c1-r2-b02-w1/gen-w1-collector-local` |

冻结内容（本轮一字节未改）：seed 42、scene `scene-0` 512×512 origin(0,0)、terrainProfile/landUseProfile、
16 住宅（12×10×8、setback 12、spacing 24、insets 4/2/4）+ 8 店铺（12×10×6，同参数）、
显式 16m + 12m 走廊与坐标、固定节点、既有 obligation、fill（6000mm）/坡度（80mm/m）/保护合同。
本轮只允许改动**新增道路候选的位置/连接拓扑/合法内部高程与确定性候选顺序**，以及显式启用的需求导向选路。

## 2. S1：真实候选供给诊断（不改旧世界、不生成新图）

新增只读诊断：`bevy/crates/core/src/worldgen/frontage_supply.rs`。它**只调用生产函数**
（`frontage::profile_roads / stations / candidate_site / envelope_reason / PlacementState::assess_detailed`
与生产 carve/support/access 链路），不复制弱化版放置逻辑；默认生成不开大日志，仅在 `#[ignore]` 诊断测试中使用。
`frontage::append` 内部被抽成同一套 `stations/candidate_site/envelope_reason/PlacementState`，
因此诊断与生产不可能漂移；重构后 `worldgen::frontage` 10 项测试全绿。

重建可信性（先证明诊断对象就是冻结世界，而不是相似输入）：

| 检查 | 结果 |
|---|---|
| 自然高度场 hash | `ee43d098f6ffa6000a197c851cd5e7fa957f8d50da8c5dbcb6bf4ae998081a14` = 冻结 `road-network-plan.json.naturalHeightfieldHash` |
| 节点集合（27） | 与冻结一致 |
| 边集合（31 base + 2 显式 + 2 auto = 35） | 与冻结一致 |
| frontage 复现 | homes `attempted=24`、shops `attempted=24`，与冻结 `scene.json` 的 0/16、0/8 及拒绝直方图逐项一致 |

### 2.1 冻结 W1 的逐路逐请求供给

| 请求 | 道路 | 路宽 | profile 长度 | 站起点 | 站数 | 尝试 | 接受 | 独立容量（单路） |
|---|---|---|---|---|---|---|---|---|
| b04c1-homes | `generated.road-auto.scene-0.0` | 12 | 128 | 18 | 5 | 10 | 0 | 0 |
| b04c1-homes | `generated.road-auto.scene-0.1` | 12 | 128 | 18 | 5 | 10 | 0 | 0 |
| b04c1-homes | `generated.road-extension.scene-0.1` | 12 | 48 | 18 | 1 | 2 | 0 | 0 |
| b04c1-homes | `generated.road-extension.scene-0.0` | 16 | 48 | 22 | 1 | 2 | 0 | 0 |
| b04c1-shops | 同上四路 | — | — | — | — | 24 | 0 | 0 |

联合拒绝（homes；shops 相同）：`wrong_purpose_or_protected 8`（auto.1 的 4 个站的 8 次侧向）、
`natural_support_or_fill_limit 10`、`existing_support_changed 3`、`occupied_or_road_support 3`。
四路**独立评估容量全为 0**，所以“把各 road 独立容量相加”得不到任何全局见证——本轮没有这样做。

### 2.2 为什么 `maxCandidateAttempts=8192` 却只尝试 24 次

生产循环的枚举是 `margin = width/2 + 8 = 14`、`station_start = margin + (road_depth-8)`、
在 `length` 与 `margin` 之间以 `station_spacing=24` 步进。冻结世界里**只有 4 条带 profile 的道路**
（2 条显式走廊 + 2 条 v1/v2 自动路），可用路段本身很短：

| 道路 | profile 长度 | 可用站区间 | 站数 × 2 侧 |
|---|---|---|---|
| 12m/128m（auto.0） | 128 | 18..=114 | 5 × 2 = 10 |
| 12m/128m（auto.1） | 128 | 18..=114 | 5 × 2 = 10 |
| 12m/48m（ext.1） | 48 | 18..=34 | 1 × 2 = 2 |
| 16m/48m（ext.0） | 48 | 22..=34 | 1 × 2 = 2 |

合计 24 次/请求，随后 `stop_reason = no_feasible_frontage`（**不是** `candidate_budget_exhausted`）：
这是**几何候选穷尽**，不是预算耗尽；8192 对这批道路永远用不到。

### 2.3 拒绝样本（可复核坐标 / 自然 / 设计高程）

诊断保留结构化拒绝证据（`RejectDetail`）。`natural_support_or_fill_limit` 会给出首个越界样本的
坐标、自然 mm、设计 mm 与子类（`natural_above_design_level` / `fill_exceeds_roadbed_limit` /
`fill_exceeds_zoning_capacity`）：

| 道路 | 站/侧 | 样本坐标 | 设计 mm | 自然 mm | 子类 |
|---|---|---|---|---|---|
| auto.0 | st18 / -1 | (130,262) | 7200 | 7800 | natural_above_design_level |
| ext.1 | st18 / +1 | (165,122) | 10210 | 4181 | fill_exceeds_roadbed_limit |
| ext.0 | st22 / -1 | (194,64) | 8887 | 9695 | natural_above_design_level |
| auto.1 | st18 / -1 | (362,66) | 7321 | 8633 | natural_above_design_level |

`existing_support_changed` 给出冲突实体与首个变化样本（前后工程高程）；同一批证据还能定位该值的
来源实体：auto.0 东侧 (168,220) `6885 → 3360`、(168,196) `4410 → 1440`、(168,172) `4320 → 1261`，
冲突实体是 `generated.road-auto.scene-0.0`（路自身），而 `before` 值单独由已承诺住宅
`generated.roadside.scene-0.generated.road-network.scene-0.30-31.-1` 的**混合填方扇面**造成
（该实体单独在该点抬到 6885mm）。完整证据：`bevy/work/collaboration/ds-b01-l1-supply/s1-diagnosis.json`。

## 3. S2：有界的需求导向道路候选（显式版本化）

### 3.1 开关与格式

- `style.automaticRoadExtensions.version = 3`；旧 v1/v2 路径**字节级不变**（见 §5）。
- 新增 `scenes[].supply = { "version": 1, "maxCandidateStates": ≤128, "preflightBudgetMs": ≤180000 }`；
  v3 必须声明，v1/v2 声明即拒；v3 必须有显式 `frontageProgram`（**不引入第二需求源**）。
- `settlement/morphology_compile.rs` 只做严格读取/保留：接受并**保留**显式 v3（不再强制降为 2），
  已验收的显式轴算法未动。

对冻结 manifest，本改动是**无操作**（证据 `effective-report-diff.json`）：重编译后的
`effectiveStyle` 与发布世界逐字段相同、`manifestStyleHash`/`effectiveStyleHash` 相同，
唯一差异是 `revision` 标签——由**上一轮收口**从 `b04-c1-r2-effective-programs-v3` 提升为
`b04-c1-r2c-effective-programs-v4`，与 L1-S1 无关。因此对“收口前”烘焙的旧世界执行
`validate-world` 会因该标签（而非本轮改动）失败：旧 W1 世界 `exit=1`
`effective-programs.json differs from the recompiled manifest+profile`
（日志 `cli-validate-old-w1-readonly.log`，只读检查，未改任何世界）。

### 3.2 候选与信用

- 候选状态 = 现有图节点 × 4 方向 × 声明段长 × 该用途被层级授权的 tier；
  **端点与既有边完全重合的段不是候选**（那条路已经存在），避免把冻结预算花在重复上。
- 信用**只来自真实地块**：候选先过生产 `clear` + 固定节点 + `append_one`（corridor 工程），
  再在临时副本上用生产 frontage 候选/完整 parcel/工程 preflight 计数“新增可落地块”。
  到 zone 中心距离**只用于确定性排序**（`supply_order`），不产生任何信用。
- 排序键（确定、稳定）：先服务仍有缺口的请求（program 顺序）→ 该请求真实增量 → 总增量 →
  授权 tier 成本 → 段长 → 节点 → 端点；避免一种请求把另一种永久挤掉。
- 边界（冻结并披露）：候选状态上限 128（跨整轮累计试算，不是每轮各 128）；每轮最多收集 32 个
  “有真实供给”的候选再择优验证；单条候选站点尝试仍受原 request 预算约束；总预检 180s；
  命中任一上限立即停止并记录 `stopReason`。**同一个 (start,end,width) 状态在整个求解中只试算一次**
  （已在 `supply_states` 中按累计 `attempted` 过滤，并有“sweep 无重复状态”的测试断言），
  因此跨轮不会重复消耗预算、也不会放大搜索。
- 事务性：候选只在 clone 上试算；**只有**通过真实复核才写回 plan，被拒候选不污染 plan/terrain/计数/ID。

### 3.3 接受前的真实复核（不是“只看 planned”）

接受一条候选前，对“已承诺 plan + 候选路”的副本按生产顺序执行：

1. `frontage::append`（真实 frontage 选择：完整 parcel 冲突、用途、fill/capacity、平台、
   `access_for_entity` 入口连通）；
2. `settlement::development::plan`；
3. `development::plan_landcover_with_program`。

任一失败、或任一请求出现**回退**（已承诺地块被顶掉）、或新增为 0 → 丢弃该候选（回退），继续下一个；
报告同时记录 `predictedGains` 与 `verifiedGains`（生产复核少于预测时标注
`verified_and_committed_with_shortfall`，不隐藏）。

## 4. 冻结 W1 输入上的一次有界联合求解（本轮真实测量）

`GLYPHWEAVE_W1_SUPPLY_STATES=128`，同一冻结 manifest（只把 `automaticRoadExtensions.version` 提到 3
并加显式策略；需求、尺寸、seed、地形、用途、固定节点、既有道路、fill/坡度合同全部不变）：

| 项 | 值 |
|---|---|
| stopReason | `no_real_supply_in_bounded_sweep` |
| 候选状态评估 | 122（**完整枚举**：`sweepExhausted=true`，不是在第 128 个状态处被截断） |
| 其中 | `road_refused 96`、`no_real_parcel_supply 26`、有供给 0 |
| 走廊拒绝细分 | `existing_road_crossing 73`、`profile_infeasible 14`、`reserved_or_outside_zoning 9` |
| 接受道路数 | 0（`edges 33 = 31 base + 2 显式`，与冻结一致） |
| 请求 / 计划 / 有入口 | 24 / 0 / 0 |
| 规划耗时 | 6.9–7.0s（180s 预算未触顶） |
| 26 条“可建但无地块”的站级拒绝合计 | `occupied_or_road_support 53`、`existing_support_changed 48`、`wrong_purpose_or_protected 38`、`natural_support_or_fill_limit 25` |

预算的 180s 只在耗时超限时截断；本次实测 7s（<4%），未触发，因此结果与机器速度无关；
“同输入同候选顺序同结果”另有测试断言。需求保持：v3 求解后有效 `frontageProgram`
仍是 homes 16/12×10/spacing24 与 shops 8/12×10/spacing24，落位实体 12×10、院落 16×18（测试断言）。

### 4.1 最好候选（bestLegalCandidate）与它为什么仍然失败

“最好”= 除一个机制外完全可行，且该机制的变化量被逐毫米记录：

| 候选 | 站数 | 站级拒绝 | 首个冲突实体 | 变化样本 |
|---|---|---|---|---|
| `(288,224)→(288,160)` 12m | 4 | 全部 `existing_support_changed` | `generated.road-auto.scene-0.0` | 6456 → 6522mm |
| `(160,288)→(96,288)` 12m | 4 | 全部 `existing_support_changed` | `generated.road-network.scene-0.30-31` | 8000 → 8550mm |
| `(224,288)→(224,160)` 12m | 10 | 4×occupied + 4×support_changed + 2×fill | `...roadside.scene-0...30-31.-1` | — |

### 4.2 已定位的最小接口边界（本轮不改，需另开合同轮）

**A. 既有 obligation 占用了地块带。** 冻结 plan 有 21 个 morphology 住宅（24×24，位于既有网格路
两侧约 30m 偏移）。frontage 地块从路中心 14m 到 32m，与这些 obligation 直接交叠；26 条可建道路里
53 次站级拒绝是 `occupied_or_road_support`。它们是已验收的既有设施，不能靠“绕开”解决。

**B. `existing_support_changed` 由 `apply_terrain_carves` 的优先级覆盖与邻地混合扇面相互作用触发（最小复现）。**
`frontage::preflight` 要求“新建筑不得改变任何已固定支撑样本”，比较 `sample(base_carves)` 与
`sample(base_carves + 拟建 carve)`。而 `worldgen.rs::apply_terrain_carves` 末段在
`graded_influence && full_priority != MIN` 时执行 `terrain = full_target`，`full_target` 以**自然面**为下限
重新累计，会**丢掉邻地的混合填方**：

```
坐标 (168,220)：自然 1274mm；无拟建时为 6885mm（相邻已承诺住宅的混合扇面）；
加拟建入口 carve（priority 2，目标 = 该站道路高程 3360）后 full_target 重置为 max(自然,3360)=3360mm
→ 6885 → 3360mm；该点落在既有 auto 路 bounds(margin=3) 内（x=168 只在 9m graded band 外 1m）
→ preflight 判 existing_support_changed，拒绝该建筑。
```

同机理在 auto.0 东侧给出 3 次拒绝（(168,220)/(168,196)/(168,172)），在 v3 扫描里给出
`(288,224)→(288,160)`、`(160,288)→(96,288)` 的全站拒绝；变化量本身只有 33–550mm。
即：可行地块被 1m 带外的混合/覆盖顺序效应挡住。修它必须重新规定 carve 合成语义
（`apply_terrain_carves` 优先级覆盖的下限），会改动所有已烘焙世界，超出本轮白名单，
因此本轮**只定位、不改**。

**C. 自然面本身也挡住一部分。** 25 次站级拒绝是 `natural_support_or_fill_limit`
（上坡侧自然面高于道路设计面 → 不允许挖方；或填方 >6000mm），例如 auto.1 需要 `over=+2021mm`、
ext.1 需要 `fill=6655mm`。

结论：在**输入授权的候选空间**（既有节点 × 4 方向 × 64/128m × 授权 tier）内 24/24 不可达；
这不是全局无解证明，但它是该候选空间内的**完整**结论，而不是“预算不够”。

## 5. 测试与证据

新增/相关测试（全部内存 fixture，不写世界）：

| 测试 | 覆盖 |
|---|---|
| `auto_roads::tests::demand_led_choice_prefers_real_parcels_over_distance_alone` | 同一 fixture：v2 选“只改善中心距离但 0 地块”的路；v3 明确拒绝它（记 `no_real_parcel_supply`）并选真实供给路；需求 count/尺寸不变；每栋实体/院落（16×18）/入口合同逐项校验 |
| `auto_roads::tests::blocked_parcels_and_unmeetable_fixed_roots_earn_no_supply` | 用途空间不足 → 无信用；地块带深坑 → fill 拒绝；固定根远低于地形 → 拒绝且根节点不动；缺策略/未授权用途/超限策略/旧版本带 supply → 格式拒绝 |
| `auto_roads::tests::supply_sweep_is_bounded_deterministic_and_leaves_refused_states_out` | 同输入同顺序确定；`attempted == sweep.len()`；只有被接受的路进入网络；`maxCandidateStates=1` 精确停在预算；需求不被改写 |
| `worldgen::auto_roads` | 12 passed（含旧 v1/v2 全部行为） |
| `worldgen::frontage` | 10 passed（重构后行为不变） |
| `worldgen::` / `settlement::` | 177 passed / 6 ignored；99 passed / 2 ignored |
| `glyphweave-cli` | 2 + 4 + 1 passed（CLI 自身 fixture，含其内部烘焙） |

冻结输入诊断（`#[ignore]`，只读 manifest 与已发布世界，不写任何东西）：
`worldgen::frontage_supply::tests::frozen_w1_supply_diagnosis_is_bounded_and_reproduces_the_published_plan`、
`...::frozen_w1_demand_led_solve_reports_real_verified_parcels`。

## 6. 端到端准入与预算

| 项 | 值 |
|---|---|
| 是否出现 24/24 见证 | **否**（122/122 候选状态完整枚举后无任何合法地块） |
| 新 512m `generate-world` | **未执行**（无见证不烘焙） |
| validate / quality / gate / road audit | 未执行（同上）；旧 W1 证据只引用不覆盖 |
| 候选状态数 / 接受道路数 / frontage 站点尝试 | 122 / 0 / 26 条测量状态内累计 164 次站级尝试（三者分开披露） |
| 内存 fixture 烘焙 | 仅测试内部 fixture；未写任何世界目录 |
| 旧世界 / A 冻结材料 / F 盘 | 未改、未动 |
| Git | 未提交、未暂存、未推送（`git status`/`git diff --check` 见证据目录） |

冻结世界完整性：按上一轮回执登记的 7 个关键文件哈希逐一复核，
`bevy/work/collaboration/ds-b04-c1-r2-b02-w1/gen-w1-collector-local` 全部 `unchanged=true`
（`frozen-world-integrity.json`）；本轮所有诊断都只读该目录与冻结 manifest。

## 7. 未完成项（如实列出）

1. **24/24 未达成**：本候选空间内 0 条新路带来合法地块；需要先解决 §4.2 B（carve 合成语义）或先
   重新定义需求/obligation 与地块带的关系（都属另开轮次的合同变更），本轮不做。
2. **`existing_support_changed` 的端到端“回退”负例未构造**：现有 fixture 无法在有界规模内构造
   “测量有供给但生产复核回退”的场景（阻断在测量阶段就已生效）；该路径的保证来自代码结构
   （只在 clone 试算、仅复核通过后写回）与 §4.2 的真实拒绝证据，未伪造测试。
3. **未探索的合法手段**（本轮白名单外）：改 `segmentLengthsM`/tier 集合扩大候选空间、
   修改 `apply_terrain_carves` 的优先级覆盖语义、调整 morphology obligation 与 frontage 地块带关系。
4. B 未签收；C 未开始；本回执不宣布里程碑 A/B 通过。
