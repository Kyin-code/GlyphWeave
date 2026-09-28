# B01-SUPPLY-R2 结果：追加事务与生产实现信用收口

- 任务：`docs/collaboration/ds-v41f-b01-supply-r2-task.zh-CN.md`；先读的独立审查：`docs/collaboration/b01-supply-r1-inspection-20260917.zh-CN.md`
- HEAD：`fb36dc4f9fdd38e6aa9c5a89676a65b96e5001b1`；工作区 `G:/difyllmwiki/GlyphWeave`
- 状态：**partial（ready_for_review）**；B 未签收、C 未开始、无 Git 写操作、无其他 agent、无新 bake、未改全局 carve。
- 本轮只做三件事：A 真追加事务（超时不发布）、B 滚动承诺集合与结构化归属、C 非递归共享生产审核与按 parcel 信用。35 边 append、RequestBudget、结构化 realized IDs 全部保留，冻结 128 评估只复跑一次以确认 R1 数值未变（见 §5）。

## A. 追加事务：墙钟中止整轮回退，且不能 `Ok` 发布成功前缀

### A.1 结构

- 追加阶段整体在**自己的 working plan** 上运行（`run_supply_phase`）；只有 `SupplyPhase::Complete` 才会被包装函数写回。
- 墙钟到限 → 返回 `Err("supply_phase_wall_clock_aborted:{json}")`，JSON 含 `stage / candidateStatesExamined / acceptedRoadsDiscarded / creditedRoadsDiscarded / authoritative:false`；
  **`plan` 完全保持进入追加阶段的可信 legacy 基线**（本轮所有已提交内容一并丢弃），调用者拿不到 `Ok`，bake 不会从 partial 前缀复现。
- 截止检查覆盖重步骤前后：会话开始、baseline realize 前/后、每条候选 realize 前/后、rollback 之后、**写回之前**（共 7 个检查点）。
- 正常“确定性工作预算”触顶（候选状态上限、单请求站点预算、段数预算）仍返回可重现 partial 结果，与墙钟超时**语义分开**；`Report.incomplete/incompleteReason` 保留在 schema 中，但被发布的报告恒为 `false/None`（R1 曾用 `incomplete=true` 表达超时，见勘误）。

### A.2 三个注入点 + 扫描式证明

`resource_clock_expiry_never_publishes_a_success_prefix` 不再是只跑 `trip_at=0`，而是对 `trip_at=0..80` 全程扫描（fixture 需求 4 块，故确有第二轮）：

| 断言 | 结果 |
|---|---|
| `Ok` 分支 | 报告 `incomplete==false`，即发布的一定是完整确定性结果 |
| `Err` 分支 | 错误含 `supply_phase_wall_clock_aborted` 且 plan 与 legacy 基线**逐字节相同**（去报告字段） |
| 覆盖 | 扫描内出现 `acceptedRoadsDiscarded>0`（**已有真实成功之后**才超时）与 `candidateStatesExamined>0`（realize 期间/之后超时）各至少一次 |

## B. 滚动承诺集合与结构化归属

### B.1 会话持有真实几何，且每轮原子更新

- `SupplySession` 现在持有 `committed: Vec<RoadsideSite>`（当前已实现地块的真实几何）、`commitments: id→归一化实体几何`、以及最近一次已验证 frontage 报告。
- `adopt(verified_plan, realization)` 只使用**结构化** `realized_ids_by_request`：
  - 每个 id 必须被某个请求列出，且其 `site.program.request_id` 必须与该请求一致，否则 `realization id … is attributed to … but belongs to …`；
  - 全部校验通过后才一次性替换 `budgets/committed/commitments`（失败不改变任何部分）。
- `apply_realization` 里 `rsplit`/`contains` 的字符串归属猜测**已删除**；测量候选时占用地面改为 `session.occupied()`（上一轮真实几何），不再从只含 roads 的 plan 空取（R1 的第二轮“占用失效”缺陷）。

### B.2 反例与证据（对应检查员 `guard-probe.json`）

`attribution_uses_the_structured_mapping_not_id_strings`：两个都通过生产 parser 的请求 id `homes` 与 `scene-0`，而地块 id 形如
`generated.frontage.steal.homes.generated.road-auto.steal.0.94.negative`（含场景名）：

- 把该 homes 地块归属给 `scene-0` → `adopt` 拒绝（`belongs to homes`），且 commitments 不变；
- 正确归属 → 只落在 homes（1），`scene-0` 为 0；
- 同一 id 被两个请求同时列出 → 仍然只记一次，且落在真实 owner 上（不会双计）。

`rolling_commitments_protect_a_first_round_identity`：第一轮提交 A；第二轮 A 丢失但同请求新增两个 → 守卫拒绝并点名 A（`realization_lost_existing_access:A`）；正常 A+B → 通过。

`real_stage_realization_failure_rolls_back_atomically`：用 `#[cfg(test)]` 且限定独立 scene id 的开关，把候选**自己的工程 final** 在某个计划地块下埋高 1.5m，随后由**真实**审核链拒绝（landcover/yard 与逐路支撑审核），结果 accepted 0、拒绝类别含 `realization_*`、plan 与 legacy 基线一致。该开关不进入生产路径（`#[cfg(test)]` 编译掉、且只对专用 scene 生效）。

## C. 非递归共享生产审核 + 新地块不能花旧缺额

### C.1 同一条生产审核体

- 新增 `road_network::audit_final_support_for_record(...)`：进入**同一个** `verify_impl` 体，只是不再重新 plan、不再做 plan 相等比较（`trusted_record=true`）；规则、阈值、覆盖范围完全不变（graded 精确 band/5e-5、ungraded 旧 0.26 合同、fill 上限、cut、band shortfall、base 边与实体比对、site 支撑 mm 精确）。
  `verify_with_style`/`verify_with_program`/`verify` 现在都是同一实现体的薄包装。
- 新增 `road_network::audit_extension_provenance(...)`：显式重做 extension 设计源重建（生产路径原本靠“重规划 + 相等”隐式获得）。
- `frontage_supply` 里自建的第二套窄检查 `road_support_status` **已删除**。
- 冻结 35 边基线实测：生产入口与共享叶子结果**完全一致**（realized 35/35、failures 0），且：

| 小验收 | 结果 |
|---|---|
| graded 平顶边缘 +1mm | 拒绝（realized<35，点名该路；`flat_top_edge`） |
| graded 运输足迹内 +1mm | 拒绝（同上；`transport_footprint`） |
| 删除固定 base 边 | `audit_node_heights` 拒绝 |
| 伪造 extension 设计源 | `audit_extension_provenance` 拒绝 |
| 非中心/整段支撑 | 由同一叶子覆盖全部 graded + ungraded base 道路与 site 支撑 |

（ungraded base 道路的 ±0.26m 是已验收的 legacy 合同，未放宽也未收紧；精确到毫米的 roadside site 支撑仍在 1mm 级别拒绝。）

### C.2 canonical final 与量化顺序

- `candidate_final_terrain` 现在复用**正式生成路径**：`generate_chunk_encoded(seed, scene origin, mm precision, zoning mask, 候选 carves)`；
  仅当传入的 natural 不是作者 seed 场（合成 fixture）时才退回“对同一 natural 施加同一 carve 组合”，并在证据中标注 `finalSource`。
  冻结基线报告 `finalSource=production-encoder`。
- R1 报告的 `actual=7.998, expected=7.9975` 分歧来自 **R4/R6 平台**（`road_parcel_feasibility` 的 0.0005 容差 + 其“量化 natural 后再 carve”基面约定），与生产审核无关；本轮改为共享生产审核后，生产路径在候选工程 final 上通过，**未改任何容差**。

### C.3 新地块不能消费旧缺额

- 只读复用已验收 landcover 的拒绝记录，给 `SurfaceAudit` 增加**附加**字段 `unresolved_yard_by_parcel`（同一批 yard 拒绝按 owner parcel 归集；总量、阈值、所有权与总账语义未改，旧文档仍可反序列化）。
- 守卫改为：**新增地块**（不在滚动承诺集合中的实体）其 yard 缺额必须为 0；**既有地块**的 yard 缺额不得高于基线；landcover/development/road 失败按**身份集合**比较（不是只比条数），必需未连通入口按身份集合比较。
- 反例（注入式，走真实守卫）：新地块缺 13m²（即使旧 13m² 被修复）→ `realization_new_parcel_yard_unresolved` 拒绝；正常新增（新地块 yard=0）→ 通过。
- 未完成口径：`unresolved_area_m2`（非 yard 的地表缺额）仍是“不高于基线”的总量对比，尚未按 source 拆分，已如实列出。

## 5. 命令、预算与未完成

| 项 | 值 |
|---|---|
| 定向测试 | `worldgen::frontage` 10 passed；`worldgen::auto_roads` **18 passed**（新增/改写 4：时钟扫描、事务守卫、归属、滚动两轮、真实回退）；`worldgen::road_network` 14 passed（新增共享叶子=生产审核体等价）；`settlement::landcover` 12 passed；`glyphweave-cli` 仅编译（exit 0） |
| 只读 ignored 探针 | 5 项全通过（S1 诊断、诊断等价、基线实现+生产等价+扰动、carve 诊断、冻结追加评估），合计 15s。**任务要求不再跑冻结 128 扫描/carve 诊断**，本轮为确认 R2 改动没有改变 R1 数值而各复跑一次，随后未再重复；结果与 R1 一致（35 边、128 状态、0 接受、0/24，`incomplete=false`） |
| 基线与实现证据 | 35 边/27 节点/21 site；planned 0 / realizedWithAccess 0（本批无 frontage 地块）；yard 13m²（旧的既有缺额，保留为历史事实）；roadStatus pass；`finalSource=production-encoder`；生产 vs 叶子 `equivalent=true` |
| 未运行 | 无新 generate-world/validate/quality/gate、无浏览器、无最大图、无全 CLI/worldgen/settlement、无 release |
| Git | 未提交/暂存/推送；`git diff --check` 干净；`doc-tree:check` PASS |
| 冻结完整性 | 旧 W1 世界登记 7 文件哈希不变、旧 receipt 不变、A 冻结材料/F 盘未动 |
| 未完成 | (1) 非 yard 地表缺额未按 parcel 拆分；(2) 墙钟只保证“不发布部分成功”，不保证跨机器完全相同的 wall-clock 中止点（成功计划由确定性计数决定，已由测试覆盖）；(3) 仍未追求 24 见证，`bestLegalCandidate=null`，B 未签收 |

## 6. 与旧结果的差异（勘误要点，已同步追加到 `b01-supply-r1-result.zh-CN.md`）

1. R1 的“超时只标记 incomplete 但保留已提交前缀并返回 Ok”已被取代：现在墙钟中止**整轮回退并返回 Err**。
2. R1 的归属用 `rsplit/contains` 字符串猜测（检查员复现了同一 id 同时归入 `homes` 与 `scene-0`）；现在只用结构化映射 + owner 校验。
3. R1 的身份保护只覆盖会话起始集合；现在覆盖**滚动承诺**（含本阶段此前各轮）。
4. R1 的 `road_support_status` 是第二套窄检查，且跳过 ungraded base 道路；现在复用生产审核体本体。
5. R1 的候选 final 用“量化 natural 再 carve”的平台约定；现在优先复用正式生成编码器，并在证据里标注实际来源。
6. R1 的 yard 判定只比总量；现在按 parcel 分账，新增地块必须自证无缺额。

## 7. 勘误（B01-SUPPLY-R3 轮追加，2026-09-18）

独立审查 `docs/collaboration/b01-supply-r2-inspection-20260917.zh-CN.md` 指出本文与 receipt 的三处口径问题。本节只纠正描述与计数，不重写已验收结论，也不修改任何旧 receipt 字节（旧 receipt 仍为 `b3c39e92…`，其字节未变）。

1. **预算口径自相矛盾。** §5 已如实写明“为确认 R2 改动未改变 R1 数值而各复跑一次”，但同段又出现 `frozen128RescanPerformed=false` 的表述，receipt 里也留着 `frozen128RescanPerformed: false`。正确口径是：本轮**冻结 128 追加评估确为 1 次**、**carve 诊断确为 1 次**；两者都**不能**写作 0 或 false。审查方在 R2 之后未再扫描。相关预算字段以 R3 receipt 的 `budget` 为准（`frozenAppendEvaluations: 1`、`frozen128RescanPerformed: true`、`carveDiagnosisRerun: 1`）。
2. **共享叶子测试是重复注册，不是两种用例。** `worldgen::road_network::tests::shared_final_support_leaf_is_the_production_audit_body` 曾被连续两个 `#[test]` 注册两遍，harness 因此报 “2 passed”，但那只是同一函数跑两次，**不是两个独立场景**，也不能当作两种覆盖证据。R3 已删除多余的 `#[test]`；该筛选现在如实报 1 个用例（本轮实测该模块 13 passed / 1 ignored）。
3. **同坐标扰动不是两类独立位置。** §C.1 小验收里名字不同的两个扰动 `flat_top_edge` 与 `transport_footprint` 实际都作用在**同一坐标 (228,96)**，失败文本也相同（`road generated.road-extension.scene-0.0 changed after planning: design error 0.0010004044m at (228,96) …`）。因此它只能算**同一采样点的重复验证**，不能称为已分别证明“平顶”与“过渡足迹”两类独立位置。共享生产审核体本身不受影响（其等价性由生产入口与叶子逐项相等证明），R3 未新增大型扰动矩阵。
4. 上述第 1 条的 hash/字节层面事实不变：R2 receipt 与证据文件均未被改写；本节只要求在后续文档与回执中按“各 1 次”如实表述。
