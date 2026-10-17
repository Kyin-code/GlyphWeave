# B04-C1-R2 + B02-W1 结果：请求级院落推导与 16m 宽路施工

日期：2026-09-17（Asia/Shanghai）。两项**分别报告**：

> **勘误（2026-09-17，R2/W1 收口轮追加；历史正文与旧证据未删改）**
>
> 1. §2 声称“只对缺省轴推导”当时并不成立：`effective_policy.footprint()` 仍无条件同时计算两个轴，合法的显式尺寸会被另一个轴的默认失败否决。已在收口轮改为真正的逐轴 lazy 推导（见 `r2-w1-closeout-result.zh-CN.md` §1），effective revision 随之升到 `b04-c1-r2c-effective-programs-v4`。
> 2. §3.2 末句“八个宽度/朝向组合”应为**六组**（3 种宽度 × 2 个朝向）。
> 3. §4 “临时在 crate 内搜索测试，已删除”与当前文件状态不符：该诊断现已保留为 `#[ignore]` 测试 `w1_cli_control_corridor_search`（`worldgen/road_network.rs`，需要历史世界，日志 `w1-corridor-search.log`）；普通测试套件因此报告 12 passed / 1 ignored。
> 4. §3.1 表中“过渡带 18m”只是 carve 的初始 blend 参数：生产随后取 `max(blend, 3*(groundHeight+32))`，实际过渡范围不是固定 18m。当时的 transition 用例实际检查的是**施工足迹内、整平带之外**的 shoulder 环，不等于完整外侧 blend 覆盖证书。
> 5. §3.1 的“新版本标签 `GRADED_ROAD_GEOMETRY_REVISION`”当时只是定义、没有消费点，因此并未完成版本区分；收口轮已把它接入权威身份与发布字段（见 `r2-w1-closeout-result.zh-CN.md` §2）。

- **B04-C1-R2：ready_for_review**（T2 剩余语义已修）。
- **B02-W1：ready_for_review**（16m graded collector 的真实完整路面施工与审核成立）。

R1 已通过的 T1/T3/T4/T5（8/12m 误报修复）全部保留；R6 未重开；未提交 Git、未启动 agent、未改 A 冻结材料/F 盘；原自然 512m 案例未改、未重跑。

## 1. 身份、基线与白名单

- 仓库 `G:/difyllmwiki/GlyphWeave`，HEAD `fb36dc4f9fdd38e6aa9c5a89676a65b96e5001b1`；dirty 工作区与全部旧证据保留。
- 本轮改动（SHA256 见 `bevy/work/collaboration/ds-b04-c1-r2-b02-w1/receipt.json`）：

| 文件 | 角色 |
|------|------|
| `bevy/crates/core/src/settlement/morphology_compile.rs` | R2：请求级生效院落驱动缺省 footprint、来源措辞、revision v3、定向测试 |
| `bevy/crates/core/src/worldgen.rs` | W1：graded 整平带规则 `max(7, 路面/2+1)`、共享定义、`GRADED_ROAD_GEOMETRY_REVISION`、覆盖短欠纯函数 |
| `bevy/crates/core/src/worldgen/road_network.rs` | W1：结构性覆盖失败（每条路一次、不计入 realized）+ 宽度/朝向/接点定向测试 |
| `docs/collaboration/b04-c1-r1-result.zh-CN.md` | R1 结果追加勘误（未重写历史） |

未改：`settlement/morphology.rs`、`road_parcel_feasibility.rs`、`auto_roads.rs`、`road_extensions.rs`、`road_profile.rs`、`development.rs`、`landcover.rs`、`cli/src/main.rs`、`cli/tests/*`、两份 C1 manifest、资产/依赖/lock、渲染与自然生成算法。

## 2. B04-C1-R2：每个请求用实际生效院落推导 footprint

### 2.1 缺陷与修复

原实现先在 requests 循环**外**用 profile 院落算好 footprint，再逐请求保留显式 `parcelInsets`：字段没有被覆盖，但**依赖这些字段的推导仍用旧默认院落**。

修复：每个请求先严格解析出**生效院落**（显式对象必须含 frontM/sideM/rearM 非负整数；否则用 profile 默认），再逐轴推导：

- 缺省 `widthM` = profile 完整地块宽 − 2×**生效** side；
- 缺省 `depthM` = profile 完整地块深 − **生效** front − **生效** rear；
- 显式 width/depth 各自保留，只为缺省轴推导；推导 checked、正偶数，之后仍过 consumer/asset 合同；
- 来源表注明实际用的是 `authored` 还是 `profile` 院落；显式 footprint 造成的偏离只在 `notes` 记录，措辞不再把用户院落说成 profile 院落。

### 2.2 检查员三个输入的复验（`inset-probe-r2.rs`/`.json`，只读复用其逻辑，未覆盖其文件）

| 输入院落 front/side/rear | R1 结果 | R2 结果 |
|--------------------------|---------|---------|
| 缺省（8/8/26） | 建筑 10×16，地块 26×50 | **10×16，26×50（不变）** |
| 显式 8/2/26 | 建筑 10×16，地块 14×50（错误） | **建筑 22×16，地块 26×50**（真实 `boundary` 确认） |
| 显式 2/2/2 | 建筑 10×16，地块 14×20（静默接受） | **推导 22×46 → storefront 深度合同拒绝**（错误信息列出派生轴） |

来源表示例（第二例）：`widthM ← profile-default :: complete parcel 26m - 2x2m authored side inset = 22m footprint`。

### 2.3 其它验收

- 逐轴优先级：仅显式 width → depth 仍按生效院落推导（`original-input` / `profile-default` 分别记录）；仅显式 depth 同理；两者都显式 → 不做推导，显式院落原样保留，越出 profile 区间时 notes 如实说明。
- 四朝向：显式 8/2/26 推导出的 22×16 建筑在 0/90/180/270 下完整地块均为 26×50（世界轴交换）✓。
- 非法显式院落（缺字段、字符串、数组、null）在推导前拒绝 ✓；未知键/错误类型/合法省略仍按 R1 语义 ✓；发布文档重编译篡改仍拒绝 ✓。
- effective revision 升到 **`b04-c1-r2-effective-programs-v3`**；v1/v2 文档与两份 C1/R1 世界全部保留、未覆盖。

## 3. B02-W1：16m graded collector 的完整路面施工

### 3.1 四个范围与各自合同（不再混成同一矩形）

| 范围 | 来源 | 合同 |
|------|------|------|
| 实际路面（pavement） | 走廊合同 `roadWidthM`（8/12/16） | 必须精确等于纵向设计高程 |
| 整平带（graded band） | 生产 carve 的整平范围，`graded_flat_top_half_extents` | 精确等于设计高程，且**必须覆盖路面** |
| 工程支撑/施工足迹（transport） | 路面 + 走廊固定 3m shoulder + 两端 junction 平台 | 只限 fill ≤ 该路 `maxFillMm`、cut ≈ 0 |
| 过渡带（transition） | carve 的 18m 平滑过渡 | 只受 fill 上限约束，**不要求**等于设计高程 |

宽度规则：**`max(7, 路面/2 + 1)`**。8m → 7、12m → 7（与 R1/旧值逐位相同），16m → 9（覆盖 8m 半路面 + 与 12m 同样的 1m 整平余量）。carve 与审核共用同一定义，杜绝再次漂移；新版本标签 `GRADED_ROAD_GEOMETRY_REVISION = "b02-w1-graded-band-v2"`。

结构性覆盖：新增纯函数 `graded_band_shortfall_m(pavement_half, band_half)`；审核在采样**之前**判一次，不足则**每条路只记一条**失败并 `continue`，该路不计入 `realized_edges`。窄带反例即 **pre-W1 的 (8, 7) → shortfall 1**（16m 路在旧常量 7m 下正是这个状态），已被规则测试固定；W1 之后所有合同宽度（8/12/16）都被覆盖，生产规划不再可能产出窄带路。

### 3.2 定向验证（`bevy/crates/core/src/worldgen/road_network.rs`，12 passed）

1. **宽度 × 朝向矩阵**：8/12/16m × 0°/90°（六个组合）在**非平自然地形**（(160,160) 为中心、0.05 m/m 的锥形起伏）上，用生产 `plan_with_style` → 生产 `plan_terrain_carves`/`apply_terrain_carves` → 生产 `verify_with_style`：全部 `status=pass`、realized=planned，整平带内误差 ≤0.00005m，band = {7,7,9,7,7,9}，且 band ≥ 路面半宽。
2. **collector + local 生产见证**：16m collector 48m + 12m local 48m，共享接点、两端固定高程一致，两段都被生产规划建成；carve 后 final 再生产 verify → `pass`、realized=planned；16m 路整平带 = 9m、整平带误差 0；共享接点两侧设计高程相同。
3. **单变量负例（各自独立坐标与判据）**：
   - 路面边缘 +1mm → `changed after planning`，**failures 恰 1 条**，`realized_edges = planned − 1`（失败路不计入）；
   - 整平带外缘（12m 案为横向 7m）+1mm → 设计误差判据失败；
   - 把路面下挖到自然面以下 0.2m → **cut 判据**失败（0.2m）；
   - 传动足迹内抬高到自然面 +6.5m → **fill 判据**失败（> 6m 上限）；
   - 过渡带：仅允许 fill、不得下挖，且实测确有 >0.001m 的偏离而审核仍通过（证明过渡带不按设计高程判定）。
4. **固定接点负例**：corridor 起始高程与父节点不一致 → 生产规划以 `start junction height mismatch`（或 corridor infeasible）拒绝；同几何在正确高程下可接受。
5. **8/12m 回归**：`graded_extensions_are_applied_and_independently_checked_not_just_designs`（8m，含 shoulder +1mm 篡改）独立通过；两份 C1 与两份 R1 世界用只读探针按 W1 规则重算：8/12m 的 band 仍是 7，整平带误差 0、apron fill ≤6、cut ≤0，与 R1 结论一致（旧证据未被改动）。

### 3.3 一次新的 512m CLI 控制（本轮唯一一次）

输入增量：沿用 C1 compact 控制的自然参数/seed/需求（512m、seed 42、同一 landUseProfile、16 住宅 + 8 店铺、同一 `automaticRoadExtensions`），**新增显式 `roadExtensions`** 表达 collector/local（该文件在轮次工作目录，不改 `examples/`）：

- 16m collector：(224,96) → (176,96)，8000 → 10327 mm；
- 12m local：(176,96) → (128,96)，10327 → 11970 mm（共享接点同一高程）；
- `rulesDir` 改为绝对路径（因清单不在 `examples/morphology/` 下）。

这些走廊由 `w1_cli_control_corridor_search`（`#[ignore]` 手工诊断，需历史世界，`--ignored --nocapture`，日志 `w1-corridor-search.log`）在**同一真实地形**上用生产规划搜索得到；外部独立探针无法复用该搜索，因为 `AlignmentContext`/`ZoningMask` 是私有结构（编译日志 `w1-external-probe-blocked.log`，见 §5）。

结果：

| 步骤 | 结果 |
|------|------|
| `generate-world` | exit 0，10.6s，revision `3f21db9d8d15a1c5…` |
| `validate-world` | exit 0 |
| `quality-report` | exit 0，status=warn（4 warnings） |
| `roadNetworkAudit` | **pass，plannedEdges 35 / realizedEdges 35，maxDesignHeightErrorM 0.0** |
| 生成物中的 graded 路 | `road-extension.scene-0.0` **16m**（48m）、`road-extension.scene-0.1` **12m**、另 2 条 12m auto-road；宽度集合 {12,16} |
| `world-quality-gate` | exit 1，**6 项内容缺额**（frontage 0/24、auto-road 目标未达、purpose-area、landcover、land-use、development）；无道路工程失败 |

即：16m collector 与 12m local 在真实 CLI 生成中成立，完整路面受支撑并通过独立设计审核；内容缺额与本轮无关，照旧如实列出。

## 4. 测试与预算

- core 定向：`morphology` **27 passed**（含 R2 新增 3 条）、`worldgen::road_network` **12 passed**（含 W1 新增 3 条）、`worldgen::frontage` 10、`worldgen::auto_roads` 9、`worldgen::road_extensions` 6、`worldgen::road_parcel_feasibility` 8/2 ignored（模块未改）、`settlement::farm_program` 3、`settlement::residual_land` 2。
- CLI 定向（未跑全量）：`--test morphology_compile` 2 passed；`--test baked_audit graded_extensions_are_applied_and_independently_checked_not_just_designs` 1 passed。
- 新的 512m **CLI 生成：1 次**（§3.3，10.6s）。测试内部 bake 次数披露：`worldgen::road_network` 的矩阵/collector/负例都在**内存**里构造 natural/final（无世界落盘）；`cli/tests/morphology_compile.rs::the_real_binary_binds_and_rejects_a_tampered_effective_document` 会用 `bake_world` 在自己的 TempDir 里烘焙 1 个 512m 世界（测试结束时由 `TempDir::drop` 删除，不影响任何交付世界）。未跑 release、最大图、浏览器巡检、全 core/workspace/CLI 全量、原自然四档扫描；R6 ignored 测试未执行。
- 只读探针：`probe-*.json`（C1/R1/W1 五个世界），用于证明 8/12 数值不变与 16m 整平带成立，全程未写回任何旧世界。

## 5. 未完成 / 阻断 / 最小提议

- **frontage 仍 0/24**（`wrong_purpose_or_protected 12`、`natural_support_or_fill_limit 12`、`existing_support_changed 3`、`occupied_or_road_support 3`），strict gate 仍 6 项内容失败；原自然 512m 案例仍 unresolved。三者本轮均未试图“解决”，只保证道路工程不再混入。
- **外部可复用的走廊预检接口仍缺**：`road_alignment::AlignmentContext` 与 `zoning::ZoningMask` 是私有结构（仅其方法 pub），外部工具（包括检查员与生产者）无法在生成前用生产函数预检 corridor。最小提议：给这两个类型加 `pub` 可见性（或提供一个 `pub fn preflight_corridor(scene, terrain, water, zones, request) -> Result<..>` 门面）。本轮未做，属白名单外变更，仅记录阻断；因此 W1 的走廊搜索用了轮内临时测试（已删除并留存日志）。
- 16m 的整平余量取 1m（与 12m 相同），而非走廊 shoulder 合同的 3m：这是为**保留 8/12m 数值/行为**所做的最小选择。若将来要求 shoulder 也精确整平，应把规则改成 `路面/2 + 3` 并单独一轮验证其对既有 graded 世界地形的影响（会改变 12m/16m 的 final）。
- R2 之后，缺省 footprint 可用性依旧只有少数 profile/kind 组合（perimeter/clustered×block、low-density/village×storefront）；这是修正后的真实覆盖，不因本轮变绿。

## 6. 结论

- **B04-C1-R2：ready_for_review** —— 显式院落已参与缺省 footprint 推导，检查员三例行为全部符合预期，来源表与偏离说明如实，revision 已升级且旧文档保留。
- **B02-W1：ready_for_review** —— 16m graded collector 在真实 CLI 生成中成立（35/35 realized、road audit pass），八个宽度/朝向组合与 collector+local 生产见证通过，单变量负例与窄带规则回归齐备，8/12m 数值与旧证据未变。

不自行签收 B，不推进 C，交付后停止等检查员复验。
