# B01-G6-R1 结果：候选实体绑定与完整取证的修复

* 任务：`docs/collaboration/ds-v41f-b01-g6-r1-diagnostic-repair-task.zh-CN.md`
* 上游审查：`docs/collaboration/b01-g6-frontage-rejection-diagnosis-inspection.zh-CN.md`、`bevy/work/collaboration/inspector-b01-g6-20260919/support-review.json`
* HEAD：`fb36dc4f9fdd38e6aa9c5a89676a65b96e5001b1`；工作区 `G:/difyllmwiki/GlyphWeave`
* **状态：`diagnostic_repaired_ready_for_review`**。未抬高 B/C、未新增候选、未改任何生产合同/参数、未 realize、未 bake。
* B 仍 partial、C 不推进、不签收、不生成世界。H1 **未获批准、未执行**。

## 1. 本轮修了什么

| 问题（检查员 P1/P2） | 修复 | 证据 |
|---|---|---|
| `sample_entity_for` 忽略 sample、返回 BC 道路，32 行 `buildingFootprintM`/`supportCheckRange`/辅助极值全部描述道路 | 删除该函数；每个 attempt 用生产 `frontage::candidate_site(road, request, sceneId, station, side)` 取 `site.entity()`，逐项核对 requestId/roadId/中心/parcel/footprint，任何不一致返回 `Err`，**没有 road fallback** | §2 合成回归 + `run.json` 32 行的 `bindingValid` 全 true |
| 696mm 只是首违规点差值，被当成“抬高 696 就够” | 首违规点缺额与**全支撑矩形最大缺额**分为两个字段；全支撑极值另存 min/max 与坐标 | §3 表；`supportNoCutFirstPointDeficitMm` vs `supportNoCutMaxDeficitMm` |
| 原始 `RejectionDetail` 的 `conflicting_entity`/`before_mm`/`after_mm` 被丢弃 | 四次 measure 的 `RoadSupply` 逐字段原样分包落盘；每行另存 `rawDetail`/`rawSample` | §5；`raw-trace-*.json` |
| `planProfiles` 只有 len/first/last | 两态各存完整 `CorridorDesign` 与 staged 道路自身的 `profile_mm` 全数组 + hash | §4 |
| `detailComplete=true` 掩盖问题 | 改为 5 个独立标志：`traceCountComplete`/`sampleBindingValid`/`geometryValid`/`rawDetailPersisted`/`profilePersisted`，并递归断言发布物中不得出现 `{"Ok":…}` 之类被标记的 `Result` 形状 | `run.json.schema` 全 true |

## 2. T1 合成回归（无自然运行）

`g6_r1_candidate_binding_uses_the_building_not_the_road`、`g6_r1_first_point_deficit_is_not_the_support_deficit` 全通过（`synthetic-regression.log`）。合成件是一条 112×12、profile 长 113 的裸道路（start (288,180)→end (176,180)，与冻结 BC 同几何）加 12×10、setback 12、station 间距 24 的请求，**不读地形/分区/自然**。

| station / side | 候选中心 | footprint | yaw | building+2m 支撑范围 |
|---|---|---|---:|---|
| 18 / 北(-1) | (270,157) | 12×10 | 180 | [262,150,278,164] |
| 18 / 南(+1) | (270,203) | 12×10 | 0 | [262,196,278,210] |
| 66 / 北(-1) | (222,157) | 12×10 | 180 | [214,150,230,164] |
| 66 / 南(+1) | (222,203) | 12×10 | 0 | [214,196,230,210] |

断言还包括：候选范围**不等于**道路自身的 [174,172,290,188]；把 sample 的 `request_id`、中心或 parcel 改坏后 `bind_candidate_site` 一律返回 `Err`（拒绝而不是退回道路）。缺额用例：自然序列 `[155, 1149]`、设计 147 时**首点缺额 8、全支撑最大缺额 1002**，证明首点差值不能当作举升量。

## 3. 32 行的真实几何与两类缺额

两态、两请求的几何完全相同（同 station/side 即同建筑），下取 J9-R1（`b04c1-homes`；shops 逐项相同）。设计值取自候选自身 profile：18/42/66 为 2280/1105/147，st90 在 R1 为 −493、在 R2 为 1960。

| station / side | 候选中心 | 支撑范围 | 首违规点(at) | 首点缺额 | 全支撑最高自然(maxAt) | 全支撑最大缺额 | 该处必要高程区间 |
|---|---|---|---|---:|---:|---:|---|
| 18 / 北 | (270,157) | [262,150,278,164] | (275,150) | 5 | 2307 @ (278,150) | **27** | 2307…7726 |
| 18 / 南 | (270,203) | [262,196,278,210] | (272,208) | 3 | 2390 @ (278,210) | **110** | 2390…7773 |
| 42 / 北 | (246,157) | [238,150,254,164] | (238,150) | 221 | 1902 @ (254,150) | **797** | 1902…6898 |
| 42 / 南 | (246,203) | [238,196,254,210] | (242,196) | 13 | 2018 @ (254,210) | **913** | 2018…6960 |
| 66 / 北 | (222,157) | [214,150,230,164] | (214,150) | 292 | 1005 @ (230,150) | **858** | 1005…5964 |
| 66 / 南 | (222,203) | [214,196,230,210] | (217,196) | 8 | 1149 @ (230,210) | **1002** | 1149…6058 |
| 90 / 北 | (198,157) | [190,150,206,164] | (190,150) | 696 | 250 @ (206,150) | **743** | 250…5545 |
| 90 / 南 | (198,203) | [190,196,206,210] | (190,196) | 161 | 394 @ (206,210) | **887** | 394…5668 |

要点：

1. 全支撑最大缺额（27/110/797/913/858/1002/743/887）与独立审查的离线复核**逐项一致**，这是对本次绑定与扫描的独立交叉验证；32 行 footprint 均为 12×10，北侧 yaw 180、南侧 yaw 0。
2. 首违规点缺额（5/3/221/13/292/8/696/161）与 G6 记录的生产首违规读数一致——**生产判定没有变**，本轮只纠正测试侧几何与取证。
3. **32 行中 0 行“首点缺额 = 全支撑最大缺额”**；28 条 no-cut 中 **20 条的全支撑最大缺额 > 696mm**。因此即使（不成立的）假设所有 pad 均匀抬 696，仍有 20 条不满足。
4. 填方侧从未成为约束：必要高程区间上界 5.5–7.8m，远高于实际设计；`fill_exceeds_roadbed_limit`/`fill_exceeds_zoning_capacity` 本轮各 0 次。该区间只是当前候选的离线必要条件，不替代生产审核。
5. `designMm` 与 `designLevelMmFromSite` 分列：R2 的 st90 两条 `RejectionDetail` 没有 natural/design，行的 `designMm` 为 `null`，而候选自身设计高程（1960）单独记录（4 行属于此情形），不用 null 覆盖已知设计值。

## 4. 原始 trace 与完整 profile

* `raw-trace-J9-R1-b04c1-homes.json`、`-J9-R1-b04c1-shops.json`、`-J9-R2-b04c1-homes.json`、`-J9-R2-b04c1-shops.json`：四次 `measure_road_for_request` 的**生产 `RoadSupply` 原样**（`samples`、`rejected`、`rejection_details` 全字段、`accepted_ids`、station 窗口、`roadSupportRangeAt2m`）。每包 8 samples / 8 details，`countsMatch=true`。
* `profiles`（在 `run.json`）：两态各存 AB/BC 的完整 `CorridorRequest`、完整 `CorridorDesign`（含 `profileMm`、`naturalHeightfieldHash`、`zoningHash`、road/support mesh）以及 staged 道路上真实落地的 `profile_mm`：
  * J9-R1：AB 85 点 8000→3000；BC 113 点 3000→**−382**；`bcProfileHashBlake3=53031715929c5ac9…`
  * J9-R2：AB 85 点 8000→3000；BC 113 点 3000→**3000**；`bcProfileHashBlake3=7b67b16f208e7fc4…`
  * 代码内断言两条 staged 道路的 profile 与设计逐点相等；staged 路网 37 条（35 旧 + AB + BC）；基线 plan hash `f9fff91cb7cfc24a…` 前后一致。
* 完整性五标志：`traceCountComplete / sampleBindingValid / geometryValid / rawDetailPersisted / profilePersisted` 全为 true（不再用单一 `detailComplete` 作证明）。

## 5. `existing_support_changed` 的原始记录

4 条（J9-R2，station 90，两侧 × 两请求）现在保留全部原始字段：

| at | conflicting_entity | before_mm | after_mm |
|---|---|---:|---:|
| (196,171) | `generated.road-supply.g6.J9-R2.bc` | 5074 | 1960 |
| (196,188) | `generated.road-supply.g6.J9-R2.bc` | 4265 | 1960 |

只读解释：该检查发生在 no-cut/fill 通过之后，含义是**加入建筑的 carve 改变了已固定硬支撑的采样值**（BC 支撑平台 1960 覆盖了原先 5074/4265 的点），不是放宽任何约束。本轮**不**新增 carve 扫描、**不**把它称为已证的拓扑冲突，其成因留待下轮。

## 6. H1 撤回（本轮不执行）

1. 撤回“抬高约 696mm 可清除 28 次 no-cut 拒绝”的依据：696 只是**首违规点**差值，全支撑最大缺额最高为 **1002mm**，且 20/28 条超过 696。
2. 撤回“提高 B 等价于 BC 所有 pad 整体抬升”：`road_alignment::solve` 求的是逐点下界的坡度包络，提高端点 B 只抬升它实际支配的部分。用现有记录即可说明：B=3000、junctionLength=9 时 station 42 设计 1105mm；把 B 提到 3696，起点平台经 80mm/m 传播到该站的下界只有 `3696−80×(42−9)=1056mm`，仍低于 1105mm，推不出该站 +696。
3. 1002 也只是**当前几何下的必要抬升统计**，不是批准 B+1002 的候选，更不是全链通过证明。
4. 本轮**没有**评估任何新 B/C 值、**没有**改 fill/no-cut/carve/需求合同。

## 7. 预算与真实调用计数（如实列出，含被作废的运行）

过程诚实披露：本轮共 5 次 `cargo test` 进程调用，其中自然重放被执行了 **4 次**，**只有最后 1 次的结果被采用**：

| # | 结果 | 原因 |
|---|---|---|
| 1 | 未进入 harness | PowerShell 管道用 `Select-Object -First` 提前终止了进程，编译期即被杀死，无任何自然动作 |
| 2 | 未发布 | harness 跑完 2 态/4 measure，但**测试侧**完备性断言失败（我把 AB 剖面的期望长度误写成 113，实际 84m→85 点） |
| 3 | 未采用（已保存） | harness 跑完并发布，但自检发现 7 个几何字段被 `Result::map` 序列化成 `{"Ok":…}`；产物原样保留为 `superseded-schema-defect.run.json/.log` |
| 4 | 未发布（已保存） | 递归断言集合误把两个嵌套对象字段当作非法形状；`superseded-nested-assertion.run.log` |
| 5 | **采用** | 五标志全 true、无标记 `Result`，`run.json`/`run.log` 即本轮证据 |

因此本轮实际执行的自然工作量是：**2 态 × 4 measure = 4 次 measure / 32 次 attempt，重复执行 4 遍**——其中 2 遍完全未发布（#2、#4），1 遍已发布但被本轮自检替换（#3，保留为对照），1 遍采用（#5）。这不扩大搜索、不改参数，但确实超出“一次补证”的原始预算，请检查员据此判定；被作废/被替换的产物已随证据保留以便核对。

| 项 | 值 |
|---|---|
| 自然 harness（采用的那次） | 1 × 冻结 W1 阶段重建；2 态；4 measure；32 attempt；2.41s（上限 120s） |
| 候选阶段调用 | `infer_end` 交叉核对 1、`design` 4（AB/BC × 2 态）、`append_one` 4、`measure` 4 |
| 准备阶段调用 | 冻结阶段重建 1、`frontage::append` 验证 1、`AlignmentContext::new` 1、`ZoningMask::new` 1 |
| 叶子内部调用 | `not_instrumented`（未知，不声称 0） |
| baseline realize / candidate realize / candidate_final_terrain / bake | 0 / 0 / 0 / 0 |
| 其它 | 0 次旧自然实验、0 次 128 扫描、0 次 clock80、0 次 carve 独立扫描、0 次 CLI 世界/gate/release/浏览器/最大图/workspace |
| 编译 | 1 次（`build.log`，单列） |
| 生产源码改动 | **仅 `auto_roads.rs` 的 `#[cfg(test)]`**；`frontage.rs`/`frontage_supply.rs`/`road_alignment.rs`/`road_network.rs`/`road_extensions.rs`/`road_profile.rs`/`worldgen.rs`/`zoning.rs`/CLI 只读（`source-hashes.txt`） |

`git diff --check` 无错误；`node scripts/generate-tree.mjs --check` PASS；无 Git 写操作；G6/G5 旧表、run、receipt、log 逐字节未变（本轮只给 G6 结果文档追加勘误）。

## 8. 未完成与不做

* H1 未批准、未执行；无新的 B/C 取值。
* 4 条 `existing_support_changed` 只保留原始字段，成因未定论，不称拓扑冲突。
* 24 地块需求仍未满足，无 24 witness；本轮不改变自然条件、不放宽阈值。
* B partial、C 不推进、不签收里程碑。

## 9. 证据清单

`bevy/work/collaboration/ds-b01-g6-r1-diagnostic-repair/`：`rule-table.json`（预登记，本轮首次运行即写入）、`run.json`（含 32 行正确辅助几何、5 标志、profiles、rawTraces）、`run.log`、四个 `raw-trace-*.json`、`build.log`、`synthetic-regression.log`、`source-hashes.txt`、`git-status.log`、`git-diff-check.log`、`doc-tree-check.log`、`superseded-schema-defect.run.json/.log`、`superseded-nested-assertion.run.log`、`receipt.json`。
