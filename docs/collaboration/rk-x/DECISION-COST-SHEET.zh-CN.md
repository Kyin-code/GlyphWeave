# 决策代价表：6 项"需授权改动"的量化代价

> 只读分析，HEAD `a9744a5`。未改源码、未提交、未生成世界。行号按该提交。
> 任务书给的两处行号已漂移，本文按实际位置引用并注明差值。
> 口径：`Round N` = `ROUND-LOG.zh-CN.md`；`JSON` = 已发布世界字段（`bevy/work/rk-x/b8-conv`）；
> `grep` = 本次统计；`复算` = 由代码常量/区间直接算出；`上限推算` = 非实测。

## 0. 结论先行

| 判断 | 项 |
|---|---|
| 默认关闭 + 新开关即可安全实现 | **2（road-first 水体）**、**6（stationSpacingM 派生）**；3/4 的"只报告"形态同样安全 |
| 技术可 opt-in，但语义需负责人点头 | **1（有界挖方）**：旧世界字节不变，却把"只填不挖"改成"有界削填" |
| 必然改既有输出（默认开即不可逆） | 1、3（派生式）、4（gate 化）、5（校验式） |

## 1. 有界挖方（bounded cut）

**改动面（新增能力 + 改合同）**
- `worldgen/frontage.rs:911-946` `support_check`，闸门 `:924 if h > level` → `natural_above_design_level`（记入 `natural_support_or_fill_limit`，`:939`）。任务书写 `:807`，当前 HEAD `:807` 是 `in_purpose(core, …)`（漂移 117 行）。
- `worldgen.rs:4521-4571` `apply_terrain_carves` 是**纯填方**：`terrain.max(target.min(limit))`（`:4558`、`:4563`、`:4569`），`:4551-4556` 注释明写"pad 是 fill 不是 excavation，永不降低自然面"。
- `worldgen.rs:96-109` `TerrainCarve` 无挖深字段；`plan_terrain_carves`（`:4275-4389`）只产出 target/blend/priority。
- 第二条同义闸门：`worldgen/services.rs` 的 `:462-475`（`support_height_or_fill_envelope`，同款 fill-only 注释）；`frontage.rs:842-858` `sample()` 用 `.max(base)` 把支撑面钉在自然面之上。
- 发布物：`engineered-terrain.json`（`engineering.rs:17-31`；`:38` 按有无 profile 定 version 1/2；`cli/tests/baked_audit.rs:2331` 断言 `version==2`）→ 削方需 v3 与 status 串变更。

**会动多少既有世界/夹具**
- `apply_terrain_carves` 在 `src/worldgen/` 目录内引用 **48 处**（grep，含测试）；`worldgen.rs` 自身另有定义与调用；生产闸门 **2 处**（上列）。
- 默认开启须重冻结：`road_network.rs:4557` `c3_base_identity…`、`:4793` `w4_chain_digest…` 两条金标；`cli/tests/baked_audit.rs`；`cli/tests/b05_multiseed.rs`（14 次烘焙矩阵）；`frontage_supply.rs` 的 ignored 只读审计 `support_gate_audit_of_a_published_world` 的期望值。
- opt-in（默认关）则以上**全部不动**（Round 11/12 先例：关闭态序列化 blake3 `57268eb9…` 逐字节不变）。

**收益（量化）**
- Round 9：`natural_support_or_fill_limit` 233 例中 **211 需挖方**；C=1m 救 54、2m 救 94、4m 救 188、不限救 211（两请求合计）；另有 23 例填方 >6 m 不受益。
- 现状（JSON：`road-network-plan.json → frontage.outcomes`）：homes `count 120 / siteIds 43 / attempted 412 / unfilled 77`；shops `24 / 24 / attempted 176`；`maxRoadbedFillM=6.0`。
- 上限推算（非实测，且把两请求可救数全记在住宅侧）：C=1m → 43+54=97，C=2m → 43+94=137，触顶 120。

**不可逆程度**
- 默认开：坡地世界地形/实体全变 → 须 bump `WORLD_GENERATOR_REVISION`（`worldgen.rs:54`），旧 manifest 重烘不再同身份。
- opt-in：旧输入逐字节不变，不必 bump；但合同语义变化 + 需补挖方台账与 gate 规则（Round 9 已列）→ **授权项，不是实现项**。

## 2. road-first 支持结构化水体

**改动面（1 个谓词 + 6 处一致调用）**
- `road_network.rs:257-273` `enabled()`，`:263` 的 `water.kind == WaterKind::None` 是唯一硬闸；调用点全走同一谓词：`auto_roads.rs:327`（报 `automatic roads require active road-first planning`）、`road_extensions.rs:53`、`frontage.rs:261`、`worldgen.rs:2192`（service program）、`:2199`（agriculture program）、`:2246`（是否规划路网）。
- 规划器本身已懂水：`rejected_water`（`road_network.rs:29`、`:611`）、候选水位判定 `:956`；既有测试 `:1964` 证明"整湖覆盖 → 0 边 + 全记 `rejected_water`"。

**会动多少既有世界/夹具**
- `examples/*.json` 34 个中 **7 个**带 `waterType`（6 river + 1 lake），**0 个**同时带 `automaticRoadExtensions`（grep）→ 默认开也无既有输入可观测。
- L2 夹具 `b05_multiseed.rs` 14 次烘焙全部 `waterAreaM2: 0.0`，并把该约束写成 finding `waterfront-context-unreachable-for-these-features`（`:1179-1182`）→ 开启后必须改这段文本。
- 无既有测试断言 `automatic roads require active road-first planning`（grep 仅错误串本身）→ 默认开不红测试，但也没有护栏。

**收益（量化）**
- 解锁 `b04-case-register` 48 条中的 **26 条**水体相关案例（地点/地形列命中水体词表，grep 计数）。
- 补上 L2 验收明文要求的"滨水阻断"正例（`milestone-b-acceptance.zh-CN.md:20,48`）；7 个滨水 example 目前只能烘出没有 road-first 路网的世界。
- 首个水陆路网世界的临街/路网数字：**需实测**（无既有样本）。

**不可逆程度**
- opt-in：无既有世界身份变化，不必 bump；但"以前必错、现在能过"是 fail-closed 语言扩容，必须新增水体审计（不得填水、岸线退距、水面高程），否则会"填湖造路却过 gate"。

## 3. coverageRatio 接线

**改动面**
- `settlement/morphology_compile.rs:903-923` 现把 `coverageRatio` 记 `unsupported`（"no consumer: 管线没有 footprint 覆盖目标"）。两条接法：**(a) 只报告**；(b) 派生（用覆盖率反推 `widthM`/`depthM`，改有效文档）。

**0.123 的来源（复算，与任务书一致）**
- `low-density-residential`：区间经 `even_midpoint` 向下取偶（`:142-155`）→ 地块 26×50、insets 8/8/26 → footprint 10×16=160 m² → **160/1300 = 0.1231**；该 profile 自带带 `[0.15,0.40]`（`morphology.rs:149`）。
- 同法 `village-field` = 14×8 / (22×34) = **0.150**，低于自带带 `[0.20,0.50]`（`morphology.rs:197`）→ 6 类 profile 有 **2 类**落在自己的带外。
- 派生值已被测试钉死：`morphology_compile.rs:2210-2216`（低密 `(10,16)`、村 `(14,8)`）与 `:2279-2288` 的 matrix 断言。

**会动多少既有世界/夹具**
- (a) 只加报告：0 世界输出变化，只暴露 2/6 profile 不达标。
- (b) 改派生：`examples/morphology/*.json` 3 个 + 18 组 profile×kind 矩阵 + 两条 digest 测试（`:1218-1232`）+ `cli/tests/morphology_compile.rs` + `cli/tests/graded_geometry_revision.rs`（身份）。

**收益（量化）**：(a) 1 个 `unsupported` 字段转正，并首次暴露 profile 数据自相矛盾（2/6）；(b) 几何会变但覆盖率不是需求 → 收益 0，不建议。

**不可逆程度**
- (a) 不动 `effectiveStyleHash`/`field_sources` → 世界身份不变（payload 只含这两项 + revision 串，`worldgen.rs:2903-2910`）。
- (b) 改身份：profile 世界须 bump `EFFECTIVE_PROGRAMS_REVISION`（`morphology_compile.rs:58`，现 `…-v5`）→ v6。

## 4. serviceRadiusM 接线

**改动面（改既有合同，不新增几何）**
- 唯一读者 `settlement/service_relations.rs:14-39 service_relations::defaults`，除自身测试外**无调用者**（grep：模块声明 `settlement.rs:19` + 本文件两处）。
- 接线 = 在最终 AccessGraph 上实现 `max_path_distance_m`（600/700/800/900 来自 profile `service_radius_m`），写进 `services::audit`（现字段仅 status/requested/planned/realizedWithAccess/unfilled/failures，无距离维度）。

**会动多少既有世界/夹具**
- `examples/*.json` 34 个中 **20 个**带 `settlementProgram`（grep）→ gate 化后这 20 个输入的审核结论要重判。
- 已发布 b8-conv 的 `service-program-audit.json` 现为 `pass 4/4`，但**无任何半径关系**；重判结果 **需实测**。
- 须改/新增：`service_relations.rs:44` 的测试、`services::audit` 产物结构与读取方、`cli/tests/baked_audit.rs` 服务审计段。

**收益（量化）**：把死字段变成"设施可达性合同"；新增建筑/临街/路网 **= 0**。可量化收益仅审核面：4/4 → 4/4 + 3 条路径距离关系。

**不可逆程度**
- 不改几何、不改世界身份；改的是**已发布世界的审核结论**（同 manifest 同字节，pass/fail 可能翻转）。
- 不必 bump generator 或 `EFFECTIVE_PROGRAMS_REVISION`；需给 audit 产物升结构。建议先 report-only 再定是否 gate。

## 5. terrain 接线

**改动面（改既有合同）**
- 现状 `morphology_compile.rs:913-915` 明文"descriptive profile metadata; the manifest terrain profile stays authoritative"。
- manifest 侧权威有三套：`style.terrain`（只接受 `continuous-heightfield`，`worldgen.rs:2079-2090`）、`style.terrainProfile`（定 `smooth_rolling`，`:1441-1445`）、`landUseProfile.theme`（CityForm，`:586-608`）。profile 的 `terrain: Vec<String>`（`morphology.rs:12`）是**第三套词表**。

**词表冲突（grep 比对）**：profile 5 个 token（plain/river-delta/coast/mountain-foot/mountain-valley），与 CityForm id 精确同名仅 **2/5**（river-delta、mountain-valley）；`plain`≠`temperate-plain`、`coast`≠`coastal-bay`、`mountain-foot` 无对应。

**会动多少既有世界/夹具**：3/3 `examples/morphology/*.json` 都是 `terrainProfile=plains`、`theme=temperate-plain`，而 `village-field`/`compact-mixed-block` 的 profile terrain 均为 `[plain, river-delta]` → 无映射表即 **3/3 被拒**；另需改 `morphology_compile.rs:1161` 的 `unsupported` 断言。

**收益（量化）**：证据诚实（适用地形从"描述"变"可判定"）；解锁场景 0、几何收益 0 → **六项最低**。

**不可逆程度**：校验式不改身份但**收窄输入语言**（旧 manifest 可能被拒）；覆盖式改地形身份，必须 bump。两者都不划算。

## 6. stationSpacingM 派生

**改动面**
- `morphology_compile.rs:434-448`：`stationSpacingM`/`maxCandidateAttempts` 缺失即 `Err("… must be stated explicitly; the morphology profile has no consumer mapping for it")`；`setbackM` 是唯一已派生项（`:449-474`）。
- 钉死测试 `:1887-1897 a_missing_search_budget_is_not_invented`。任务书写 `:1795`，当前 HEAD `:1795` 是 `derived_parcel_box_holds_for_all_four_cardinal_fronts`（漂移 92 行）。
- 取值域 `frontage.rs:177-178`（8..=128 且偶数）；`MorphologyProfile`（`morphology.rs:7-24`）**无** station spacing 字段 → 派生需新增 profile 字段（`valid()` 要求 `version==1`）或从 road_hierarchy/parcel 宽推算。

**会动多少既有世界/夹具**：只影响今天**会报错**的输入，全仓 **0 个**（3/3 `examples/morphology`、b8 manifest、全部测试夹具都显式写了 `stationSpacingM`）。只需改上述 1 条测试断言；`maxCandidateAttempts` 仍必须写，故"零工程数 manifest"尚不成立。

**收益（量化）**：profile manifest 少写 1 个工程数；**0 个既有输入受益**。收益小，代价全表最小（1 函数 + 1 测试 + 1 条 provenance）。

**不可逆程度**：无。默认开也不必 bump——`field_sources` 只在新输入上多一条，既有成功输入逐字节不变。

## 7. 排序表（收益/代价）

| 序 | 项 | 收益 | 代价 | 收益/代价 | 可 opt-in 零风险 | 必然改既有输出 |
|---|---|---|---|---|---|---|
| 1 | 6 stationSpacingM 派生 | 小（少写 1 数） | 极小（1 函数 + 1 测试） | **最高** | ✅ 只碰报错输入 | 否 |
| 2 | 3a coverageRatio 只报告 | 小（1 字段转正 + 暴露 2/6） | 小（只加报告） | 高 | ✅ | 否 |
| 3 | 2 road-first 水体 | **大（26/48 案例 + L2 正例）** | 中（1 谓词 + 水体审计 + 1 条 finding） | 高 | ✅ 新开关默认关 | 默认关则否 |
| 4 | 1 有界挖方 | 大（+54/94/188） | 大（2 闸门 + carve 语义 + 台账 + v3 产物） | 中 | ⚠️ 技术可，合同变更需点头 | 默认开则必然 |
| 5 | 4 serviceRadiusM | 中（3 条关系） | 大（20/34 输入重判） | 低 | 只报告可 ✅，gate 不可 | gate 化会改审核结论 |
| 6 | 5 terrain | 极小（仅诚实性） | 中（需映射表，3/3 被拒） | **最低** | ❌ 校验即改输入语言 | 校验式必然 |

## 8. 下一轮能不能直接做

- **能，建议先做 2**：`enabled()` 加一个 `style` 开关（默认关），6 个调用点自动一致；34 个 example 中 0 个 `water + automaticRoadExtensions`，旧世界逐字节不变。代价补水体审计，收益 26/48 案例 + L2 滨水正例；需 1 次 1 km 实测（额度记账）。
- **能，零风险顺带做 6 与 3a**：只增加"原本报错/原本不报"的信息面，不改任何既有成功输出字节。
- **不能直接做 1**：opt-in 能让字节不变，但"只填不挖"是合同不是实现，需先裁定削方上限 C 与挖方台账/gate 规则。
- **建议不做 5**；**4 先 report-only**，等 20/34 输入重判结果再决定是否 gate。

---
计数汇总（grep）：34 个 example manifest；7 个带 `waterType`；16 个带 `automaticRoadExtensions`；20 个带 `settlementProgram`；13 个带 `frontageProgram`；3 个带 `morphologyProfile`（3/3 都显式写了 `stationSpacingM` 与 `setbackM`）→ **含 profile 且缺 `stationSpacingM`/`setbackM` 的磁盘输入为 0**（仅测试内构造）。
备注：`34` = `examples/*.json` 31 + `examples/morphology/*.json` 3；"行号漂移"是相对任务书所给行号。
