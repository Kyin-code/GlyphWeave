# B01-SUPPLY-R1 结果：修正供给算法的基线、回退与实现信用

> **勘误（2026-09-17，B01-SUPPLY-R2 追加；本文件其余内容保持当时原样）**
>
> 1. **超时语义**：本文 §4“墙钟到限只标记 incomplete、保留已提交前缀并返回 Ok”已被 R2 取代：现在墙钟中止会**丢弃本次追加的全部修改**、
>    保留进入追加阶段的可信 legacy 基线，并返回结构化 `Err("supply_phase_wall_clock_aborted:{…}")`；被发布的报告恒 `incomplete=false`。
> 2. **归属**：本文 §2/§3 的 `apply_realization` 用 `rsplit/contains` 猜测请求归属；检查员复现了同一地块 id 同时归入 `homes` 与 `scene-0`。
>    R2 改为只使用结构化 `realized_ids_by_request` + owner 校验，并原子替换会话承诺集合。
> 3. **跨轮身份**：本文 §2 的身份保护只覆盖会话起始集合；R2 改为**滚动承诺**（含本阶段此前各轮新增的 id/几何/入口）。
> 4. **道路审核**：本文 §2 的 `road_support_status` 是第二套窄检查且跳过 ungraded base 道路；R2 改为复用生产审核体
>    `road_network::audit_final_support_for_record`（同一 `verify_impl` 体、不重规划），并显式补 extension 设计源重建。
> 5. **候选 final**：本文 §6 采用平台“量化 natural 再 carve”约定；R2 起优先复用正式生成编码器（`finalSource=production-encoder`），
>    合成 fixture 才退回同 carve 组合并标注。
> 6. **yard 信用**：本文 §2 只比较 yard 总量；R2 按 owner parcel 分账（`unresolved_yard_by_parcel`），新增地块必须无缺额、旧缺额不得扩大。
> 7. 细节与证据见 `docs/collaboration/b01-supply-r2-result.zh-CN.md` 与 `bevy/work/collaboration/ds-b01-supply-r2/`。

- 任务：`docs/collaboration/ds-v41f-b01-supply-r1-task.zh-CN.md`；先读的独立审查：`docs/collaboration/b01-l1-supply-inspection-20260917.zh-CN.md`
- HEAD：`fb36dc4f9fdd38e6aa9c5a89676a65b96e5001b1`；工作区 `G:/difyllmwiki/GlyphWeave`
- 状态：**partial（ready_for_review）**；B 未签收、C 未开始、无 Git 写操作、无新 agent、无正式 bake。
- 本轮结论：审查指出的三处实质缺陷（33/35 基线、恒 false 的防倒退、计划信用冒充实现信用）已修复并用真实生产阶段复核；
  在**正确的 35 边冻结基线**上做了一次有界追加评估（128 状态、8.3s、0 接受、计划 0/24），**bestLegalCandidate = null**（没有任何候选获得“已实现且有入口”的信用），因此仍无 24 见证；carve 冲突只做最小诊断，未改任何规则。

## 1. 事务回退与累计账目（对应用户任务 §1）

### 1.1 恒 false 的防倒退条件已删除

旧代码 `if totals.zip(increments).any(|(total,gain)| total < gain)` 对非负整数恒不成立（这正是审查给出的反例 baseline `[2,0]` → `totals [1,1]`：`increments=[0,1]`，丢失检查与零收益检查都通过）。
现在改为：

- **按 request ID 对照**新实现总数与可信基线：`realization_regressed_request:{id}:{before}->{after}`，任一请求倒退即回退；
- **稳定身份/几何/固定入口**对照：基线中每个已实现 site 的 id + 归一化实体几何必须原样保留，否则 `realization_lost_existing_site` / `realization_changed_existing_site` / `realization_lost_existing_access`；
- baseline 不再硬编码为零：追加阶段的基线是“进入本阶段前的 plan + 生产 frontage 选择”经**同一实现链**审计后的真实集合（§2），`outstanding` 由该集合派生。

### 1.2 诊断与生产的累计预算统一

旧诊断对每条路都用固定的 `remaining=count` 和完整的 `maxCandidateAttempts`（同一请求可被多条路各自满足、预算也被各路重置）。
现在 `frontage::RequestBudget`（`frontage.rs`）是**生产与诊断共用**的累计账目：`claim_attempt/reject/accept/remaining/stop_reason`，
`frontage::append` 与 `frontage_supply::measure_road_for_request` 都只通过它记账，因此跨路累计、单请求预算、剩余需求在三处一致。

证据测试：`diagnosis_replays_production_and_carries_budgets_across_roads`（冻结 35 边上把 `count=4 / maxCandidateAttempts=10` 的请求交给 4 条 profile 路）：

- 诊断的接受 ID 序列**逐条等于** `frontage::append` 的 `outcome.site_ids`；
- 每个请求 `joint_attempted ≤ maxCandidateAttempts`、`joint_accepted ≤ count`（累计语义，跨路不重置）；
- `joint_station_attempts` 与 `joint_rejected` 覆盖**全部请求**的 trace 合计，不再只取第一条有拒绝详情的 trace。

### 1.3 事务与负例

- 候选一律只在 clone 上试算；只有“真实实现复核通过”才写回 `plan`（只写 road，frontage 由最终生产 pass 重新推导），
  失败/零收益/一增一减都不污染 plan、terrain、ID 与计数。
- 注入式守卫负例（`transaction_guards_reject_regression_identity_yard_and_unrealized_plans`）：
  审查的算术反例（住宅 2→1、店铺 0→1）**被拒**；同总数换身份**被拒**；`planned=3/realized=2` 报
  `realization_planned_but_not_realized`；`yard_unresolved > baseline` 报 `realization_yard_unresolved`；纯新增且院落已实现**通过**。
- 真实生产阶段负例：`real_stages_never_trade_existing_identities_for_new_ones` 在真实 `append_legacy → frontage::append → 追加阶段`
  序列上断言“已提交 plan 必须保留基线的每一个已实现身份”；本轮**未能构造出“测量通过但生产复核丢地块”的强制端到端复现**
  （见 §7 未完成项，附已尝试的构造思路与失败原因，不伪造证据）。

## 2. “verified”接到真实实现链（对应用户任务 §2）

`frontage_supply::realize_candidate` 只用已验收叶子函数构造候选证据（不重写 checker、不建第二套碰撞算法）：

| 步骤 | 复用的已验收函数 |
|---|---|
| 候选工程 final | `plan_terrain_carves` + `apply_terrain_carves`（`road_parcel_feasibility` 平台同一约定：量化自然面 + 候选 carve，毫米编码） |
| 候选实体 | `plan.entities() + plan.site_entities()` + `normalize_spatial_semantics` |
| AccessGraph | `worldgen::build_access_graph` |
| 实现并有入口 | `frontage::audit`（新增**附加**字段 `realized_ids`，计数与阈值不变；旧文档无该字段仍可反序列化） |
| 权属/建筑 | `development::plan` + `development::verify` |
| 地表/院落 | `development::plan_landcover_with_program` + `landcover::realize` + `landcover::verify`（`unresolved_yard_m2` / `unresolved_area_m2`） |
| 节点高程 | `road_parcel_feasibility::audit_node_heights` |
| 道路工程 | 生产规则同值复用的逐路检查：graded band 必须覆盖路面、整平带内设计高程、运输足迹内 fill≤上限且不挖方（见 §7 的接口说明：生产 `road_network::audit` 会重规划并要求 plan 相等，无法接受带候选超集的 plan） |

提交判据（`commit_decision`，纯函数、可注入结果测试）：
① 每个基线已实现身份/几何/入口保留；② 每请求实现数不倒退（按 ID 对照）；
③ 至少新增 1 个“已实现且有入口”的稳定 ID；④ `planned == realized_with_access`（计划不等于实现即拒）；
⑤ 院落未解决面积与地表未解决面积不高于基线；⑥ landcover/development 不新增 failure；⑦ 不存在新增的必需未连通入口；
⑧ 节点高程通过；⑨ 道路工程 failure 不高于基线。

报告逐路给出：`planned`、`realizedWithAccess`、`yardUnresolvedM2`、`roadStatus`、`developmentStatus`、`landcoverStatus`，
以及 `predictedGains`（仅供排序，命名即 predicted/planned）与 `verifiedGains`（真实实现增量）。

### 2.1 正确 35 边基线的真实实现证据（一次内存实现，1.03s）

`frozen_w1_joint_realization_baseline_is_measured_once`（只读冻结输入）：

| 项 | 值 |
|---|---|
| 基线形态 | edges 35、nodes 27、extensions 4、obligated sites 21 |
| frontage | planned 0 / realizedWithAccess 0（该世界本批未落任何 frontage 地块） |
| 权属/地表 | development `incomplete`、landcover `incomplete`、program-landcover `pass`、parcels 21、buildingsRealized 21 |
| 院落 | `unresolved_yard_m2 = 13`（**基线本来就有的部分未解决**，故判据是“不高于基线”而不是“必须为 0”） |
| AccessGraph | 必需未连通 0 |
| 可信基线来源 | plan 自身的未分级 base 域 **等于** 生产 `plan_with_zones` 重建：31 条 base 边、23 个固定节点、base level 8000mm |

## 3. 在正确冻结基线上做追加求解（对应用户任务 §3）

- 显式 `scenes[].supply.mode`：`"append"` = 先跑作者声明的 v1/v2 自动路阶段（成为可信基线），**再**追加需求导向的路；
  `"replace"` = 保留旧的“v3 取代 v2”语义（另一种输入语义，单独报告，不得充当本轮 35 边实验）。
- 新增 ID 使用独立命名空间 `generated.road-supply.{scene}.{n}`，不与已占用的 `generated.road-auto.scene-0.0/.1` 冲突。
- 基线在报告中原样保留：`baseRoadProgram`（v1/v2 阶段报告逐字段不变），本阶段只列自己的 `creditedRoadIds`。
- 评估前后的精确比较（`frozen_w1_demand_led_solve_reports_real_verified_parcels` 内断言）：
  节点集合与冻结 27 节点逐一相等、21 个义务 site 不变、冻结 35 条边全部仍存在且端点/长度/朝向不变、只允许新增。

### 3.1 一次有界冻结追加评估（本轮唯一一次）

| 项 | 值 |
|---|---|
| 阶段输入 | edges 35 / nodes 27 / sites 21（= 冻结基线，未替换） |
| 模式 | `append`（基线 road ids：`generated.road-auto.scene-0.0`、`.1`） |
| 评估状态 | 128（`maxCandidateStates` 用满；`sweepExhausted=false`：**未**跑完候选空间） |
| 接受 / 计入 | 0 / `creditedRoadIds` 为空 |
| 拒绝 | `existing_road_crossing 81`、`extension overlaps road outside a matching endpoint junction 7`、`no_real_parcel_supply 16`、`profile_infeasible 15`、`reserved_or_outside_zoning 9` |
| frontage 结果 | 计划 0/24（请求 16 + 8 保持不变） |
| 耗时 | 规划 8.3s（其中每条候选需要真实实现时才 1.03s/次，本次无候选进入实现阶段）；frontage 0.14s |
| 见证 | 无 → **未烘焙**、未跑 validate/quality/gate/browser/最大图 |

## 4. 候选范围、预算与可重复性（对应用户任务 §4）

**候选集（准确描述，不再夸大）**：现有 27 个图节点 × 4 个基方向 × 声明段长 {64,128}m × **仅 `tier_for(purpose,false)` 的 local 12m**
（本轮扫描到的宽度集合就是 `{12}`；collector/其它授权 tier 未扫描）。按文档化顺序（先仍缺口的用途到用途用地的距离，再节点/端点/长度/宽度）
枚举，最多评估 128 个状态；角度/长度/宽度的集合随 `append-solve.json` 的 `sweepWidthsM/sweepLengthsM/sweepNodes` 一并披露。
`sweepExhausted=true` 只表示“当前定义的候选列表被走完”。

**可重复性/超时**：确定性计数器（累计状态数、单请求站点预算、每轮批量、接受上限）决定成功计划；墙钟只做资源中止：
到限时**不提交**该候选、报告 `incomplete=true` + `incompleteReason`，plan 保持上一个已复核状态（`resource_clock_expiry_never_publishes_a_success_prefix`
用可注入 `StepClock` 精确验证）。最长不可中断单步 = 一次候选完整实现（冻结世界实测 1.03s），已如实披露。

## 5. carve 冲突：最小诊断（本轮不改规则）

`frozen_w1_carve_conflict_is_reproduced_and_located`（只读，35 边上下文，两次采样结果相同 ⇒ 确定性）：

| 项 | 值（已按审查纠正） |
|---|---|
| 道路 | `generated.road-auto.scene-0.0`，中心 x=160、yaw 90、**宽 12m** |
| 半宽 | 路面 6m、**整平平顶 7m**、运输足迹 9m |
| 冲突点 | (168,220)，横距 **8m** |
| 所属层 | `transport_shield_outside_flat_top` —— **整平平顶外 1m、运输足迹内**（不是“9m 整平带外 1m”） |
| 采样 | 自然 1274mm → 无候选 final **6885mm** → 有候选 final **3360mm**；该点道路设计高程 3200mm |
| 决定性 carve | ① 已承诺住宅 `...30-31.-1`（priority 1、flat 13×13、blend 120m、距点 31m、target 8000mm）：单独把该点抬到 6885mm；② 候选入口走廊（priority 2、flat 10.5×2、blend 2、**outside=0 完整覆盖**、target 3360mm）：触发 `graded_influence && full_priority != MIN → terrain = full_target`，把 6885mm 覆盖为 `max(自然 1274, 3360) = 3360mm` |
| 证据 | `carve-conflict.json`（20 个进入各自 blend 的 carve，含 owner/kind/priority/flat/blend/outside/target/profile/是否候选） |

**最小后续设计提议（未实现，需另开合同轮）**：

1. 把“硬支撑覆盖”从“重置到自然面”改成**与既有承诺的兼容合成**：`full_target` 至少保留已承诺实体的既有混合支撑与自然面，
   只允许新增工程**抬高**或在其自有整平带内取精确值；不得降低支撑要求、不得删除既有义务、不得对合法新增做全局 max 合并。
2. 以版本化工程策略承载该变化（例如新的 `engineeringPolicy` revision），使已发布世界的 final 不会被静默改写；
   受影响的只是**实际触发该覆盖组合**的世界，发布文件本身不会自行改变（纠正旧报告“会影响每一个已烘焙世界”的说法）。
3. 若最小修复后仍无可行追加，保持“拒绝”是合法结果；本诊断不构成“改 carve 即可达成 24”的结论。

## 6. 接口边界（如实记录，未放宽任何审核）

候选最终栅格与实现审计采用**已验收平台同一约定**（量化自然面 + carve、毫米编码）。原因：已验收的 `road_parcel_feasibility::audit_joint_plan`
在本轮真实世界上不可用——① 它用**解析自然面**的 bake 栅格比对 `road_parcel_feasibility::engineered_terrain` 约定时会超差
（实测 `road generated.road-network.scene-0.44-45 final support differs at 178,423: actual=7.998, expected=7.9975`）；
② 其 5e-4 的 f32 容差正好落在毫米量化的半格上，对恰好 0.5mm 的样本无法满足（既不能通过也不能靠另一种量化通过）。
因此改为**复用同一批验收叶子**（§2 表），并额外在基线探针中证明 base 域与生产重建一致。该边界只作为待裁决项记录，本轮不放宽容差、不改审核代码。

## 7. 交付与预算

| 项 | 值 |
|---|---|
| 定向测试 | `worldgen::frontage` 10 passed；`worldgen::auto_roads` **15 passed**（新增 3：`resource_clock_expiry_...`、`transaction_guards_...`、`real_stages_never_trade_...`）；`worldgen::frontage_supply` 5 个 ignored 诊断全通过（S1 诊断、诊断等价、基线实现、carve 诊断、追加求解） |
| 旧 S1 诊断 | 保留并重跑，结论不变：hash/节点/边一致、24 次/请求、0 接受、`no_feasible_frontage`（几何穷尽） |
| 一次有界追加评估 | 128 状态 / 8.3s / 180s 预算未触顶 / 0 接受 / 0/24 |
| 未运行 | 无正式 generate-world、无 validate/quality/gate、无浏览器、无最大图、无释放构建、无全 workspace |
| 早期误跑披露 | 本轮开始阶段曾跑过 `worldgen::` / `settlement::` / `glyphweave-cli` 全量（日志见 `ds-b01-supply-r1` 目录外的旧证据），读到本任务预算约束后未再重复；CLI 套件 43 项、其中多项在自身 TempDir 内烘焙**测试 fixture**，不得称“全部仅内存且未写任何世界目录” |
| Git | 未提交/未暂存/未推送；`git diff --check` 干净；`doc-tree:check` PASS |
| 冻结完整性 | 旧 W1 世界登记 7 文件哈希不变、manifest 副本相同、A 冻结材料/F 盘未动 |

## 8. 未完成项（如实列出）

1. **强制端到端回退负例未构造**：要在有界 fixture 内让“测量通过、生产复核丢地块”，需要“高位义务 + 低位新路”的坡地组合
   （W1 世界正是这种地形）。本轮尝试了平坦 fixture 的“16m 基线 + 12m 候选抢占前序站点”思路，候选的 parcel 与基线重叠，
   测量阶段即被 `occupied_or_road_support` 拒绝，无法进入验证阶段；因此只交付注入式守卫负例 + 真实阶段的“不丢身份”不变量测试，
   未伪造端到端复现。
2. **候选空间未跑完**：128 状态预算内只扫描 local 12m；collector/16m、其它段长、非图对齐几何都未评估，故不得推断 24 全局不可达。
3. **`audit_joint_plan` 接口边界**（§6）尚未裁决：需要独立合同轮决定是修其容差/基面约定，还是继续按本轮的叶子组合审计。
4. **bestLegalCandidate = null**：本轮没有候选取到“已实现且有入口”的信用；没有 24 见证，因此不做正式 bake；B 未签收、C 未开始。
