# B04-C1-R1 结果：修正编译语义，定位并最小修复 CLI 道路审核失败

日期：2026-09-16（Asia/Shanghai）。状态：**ready_for_review**（不签收 B、不推进 C、不提交 Git、不启动其它 agent）。

> **勘误（2026-09-17，B04-C1-R2 / B02-W1 追加；历史正文与旧证据未删改）**
>
> 1. §6.3 “任何合法的 graded road 在非平地上都无法通过”过于绝对。准确表述是：**两份 12m 控制世界**在该范围不一致机制下必然误报；不能排除其它地形/宽度本来能通过。T1/T3/T4 与 T5 的 8/12m 误报修复本身仍然成立。
> 2. §6.5 的反例清单与实际注入不符：R1 测试里的 +1mm 落在**横向 6m（12m 路的路面边缘）**，不是整平带外缘 7m；当时也没有单独注入“超填”变体（0.2m 下挖那一例是设计误差判据，不是 cut 判据）。准确的“路面边缘/整平外缘/超填/挖方”四类单变量反例已在本轮 W1 按真实坐标与判据补齐（见 `b04-c1-r2-b02-w1-result.zh-CN.md` §3）。
> 3. §8 的“16m graded road 的整平带仍窄于其路面”描述正确，但当时的**审核**也只检查 7m，因此 16m 路在 R1 下是“自洽地缺 1m 路面支撑”而不是被判失败；本轮 W1 把整平带改为 `max(7, 路宽/2+1)`（8/12m 数值逐位不变，16m 得到 9m），并新增“整平带必须覆盖路面”的结构性拒绝规则。
> 4. T2 在 R1 仍不完整：缺省 footprint 使用了 profile 院落而非**该请求实际生效的院落**；已在本轮 R2 修正（显式 8/2/26 现在得到 22×16 建筑 / 26×50 完整地块；显式 2/2/2 推导 22×46 并被 storefront 合同拒绝）。

本轮按 `ds-v41f-b04-c1-r1-task.zh-CN.md` 完成 T1–T5。C1 的 effective 编译/原 manifest/独立重编译接线全部保留；C1 旧证据（18 份世界文件）本轮逐字节未改（见 §8）。R6 `road_parcel_feasibility.rs` 未改（8 passed / 2 ignored）。

## 1. 身份与白名单

- 仓库 `G:/difyllmwiki/GlyphWeave`，HEAD `fb36dc4f9fdd38e6aa9c5a89676a65b96e5001b1`，dirty 工作区保留。
- 本轮改动（SHA256 全量见 `bevy/work/collaboration/ds-b04-c1-r1/receipt.json`）：

| 文件 | 角色 | bytes | SHA256（前 16） |
|------|------|-------|------------------|
| `bevy/crates/core/src/settlement/morphology_compile.rs` | T1–T3 语义/严格解析/来源/revision | 73 235 | `3b59ad7f22cf48b2` |
| `bevy/crates/core/src/worldgen.rs` | 共享 graded 整平带定义 + carve 读同一定义（值逐位不变） | 416 239 | `8136765baa6d9dfb` |
| `bevy/crates/core/src/worldgen/road_network.rs` | 审核带 = earthwork 实际整平带 + 覆盖规则 + 定向测试 | 60 709 | `2014ddb2d9c6a434` |
| `bevy/crates/cli/tests/morphology_compile.rs` | T4 自给可移植测试（重写） | 8 240 | `024b4b6b0cf4d4b7` |
| `bevy/crates/cli/tests/baked_audit.rs` | 仅旧断言最小修正 | 108 401 | `c638020971cb7484` |
| `docs/collaboration/b04-c1-result.zh-CN.md` | 追加醒目勘误（未重写历史） | 见回执 | 见回执 |

未改：`settlement/morphology.rs`（六 profile 定义）、`road_parcel_feasibility.rs`、`development.rs`、`landcover.rs`、`landuse_ledger.rs`、`parcel_geometry.rs`、`auto_roads.rs`、`road_extensions.rs`、`road_profile.rs`、`frontage.rs`、`services.rs`、CLI `main.rs`、两份 C1 manifest、资产/依赖/lock、A 冻结材料与 F 盘。

## 2. T1 严格解析：编译器不再吞未知输入

复现检查员 `compiler-probe.json` 的 `frontageProgram.unexpectedField`：原 `frontage::from_style` 拒绝，而 `compile` 成功且未知字段消失。

根因：`compile_frontage` 用 `json!({"version","requests"})` **重建**了容器，丢掉其它键。

修复：容器改为“原对象克隆 + 只替换 `requests`”，未知键保留并交给各自的严格读取器。核对其余重建路径：`automaticRoadExtensions`、`residualLandProgram`、`agricultureProgram` 本来就保留原对象（本轮补了负例证明），只有 frontage 存在此缺陷。

| 反例 | 修复前 | 修复后 |
|------|--------|--------|
| `frontageProgram.unexpectedField=true` | compile 成功、字段被吞 | 拒绝：`unknown field unexpectedField, expected version or requests` |
| request 拼错键（`widhtM`） | 被吞并以默认值生成 | 拒绝（`unknown field`） |
| `automaticRoadExtensions` 容器/场景未知键 | 已拒绝 | 拒绝（补负例固定） |
| `residualLandProgram` / `agricultureProgram` 容器/场景未知键 | 已拒绝 | 拒绝（补负例固定） |
| 合法省略（缺尺寸/insets） | 可编译 | 仍可编译（未被误判为缺必填） |
| `null` 容器、数组容器、字符串 version、version=2、缺 requests、`count:"4"` | 拒绝 | 拒绝 |

测试：`unknown_keys_reach_the_strict_consumer_and_are_rejected`、`malformed_program_shapes_are_rejected_and_omissions_still_compile`。

## 3. T2 拆清完整地块 / 建筑 footprint / 院落

原实现把 `parcelWidthM/parcelDepthM` 直接写成 building `widthM/depthM`，再叠加 insets，village-field 得到 30×60m 完整地块（超出自身 48m 上界）。

新语义（`ParcelPolicy`）：

1. 地块盒 = `even_midpoint(parcelWidthM/parcelDepthM)`（偶数，落在声明区间内）；院落 = 各 inset 的 midpoint。
2. 缺省 building footprint = **地块 − 院落**：`width = parcelW − 2×side`、`depth = parcelD − front − rear`，全部 checked arithmetic，正偶数校验，否则**报错**（不 clamp）。
3. 显式建筑尺寸永远优先；显式尺寸 + profile insets 推出的完整地块若越出 profile 声明区间，只在 `report.notes` 里报告偏离，**不调整**显式值。
4. 推导出的 footprint 仍要过 consumer 合同（`frontage::from_style`）与 asset 合同（`WorldManifest::validate`）；拒绝时错误信息会列出所有 profile 派生字段。
5. 部分显式（如只给 width）时，来源表逐个字段标 `original-input` / `profile-default`，并给出推导算式（例如 `complete parcel 26m - 2x8m side inset = 10m footprint`）。

六 profile × 三种 kind 矩阵（`residential_home` 12–48×10–36×3–14、`residential_block` 24–100×16–60×12–60、`storefront` 8–36×8–24×3–12；profile 是否可用只取决于 consumer 合同，asset 合同由 manifest 校验再卡一次）：

| profile | insets(f/s/r) | 缺省 footprint | 高度 | home | block | storefront |
|---------|---------------|----------------|------|------|-------|-----------|
| compact-mixed-block | 4/2/4 | 14×16 | 16 | 拒绝(h) | 拒绝(w) | 拒绝(h) |
| perimeter-block | 4/4/22 | 34×30 | 24 | 拒绝(h) | **接受** | 拒绝(d) |
| low-density-residential | 8/8/26 | 10×16 | 10 | 拒绝(w) | 拒绝(w) | **接受** |
| clustered-residential | 6/4/16 | 26×22 | 24 | 拒绝(h) | **接受** | 拒绝(h) |
| terrain-constrained-compact | 4/2/8 | 16×16 | 30 | 拒绝(h) | 拒绝(w) | 拒绝(h) |
| village-field | 6/4/20 | 14×8 | 12 | 拒绝(d) | 拒绝(w) | **接受** |

（h/w/d = 高/宽/深越界；拒绝即编译报错，不做缩小。`parcelInsets` 通路六个 profile 全部可用。）

测试：`default_footprint_is_the_parcel_minus_its_yards`（并用真实 `parcel_geometry::boundary` 复原 26×50 地块）、`explicit_geometry_outside_the_profile_band_is_reported_not_adjusted`、`footprint_deduction_rejects_degenerate_and_odd_parcels`、`derived_parcel_box_holds_for_all_four_cardinal_fronts`（0/90/180/270 朝向的完整地块范围一致）、`profile_footprint_matrix_is_accepted_or_refused_never_shrunk`。

## 4. T3 比例语义：publicSpaceRatio ≠ pasture

`publicSpaceRatio` 只填 `urbanOpenSpaceRatio`；`ruralPastureRatio` 没有独立 profile 来源时：

- 显式牧草地程序逐字节保留（`ruralPastureRatio` 记 `original-input`，`profile_field` 为空）；
- 声明了 residual 程序但缺 `ruralPastureRatio` → **报错**：`must be stated explicitly; the morphology profile has no independent pasture parameter`；
- 完全没声明 residual 程序 → 不再凭空合成（保留 legacy 默认“无程序”），并在 `unsupported` 明示 `publicSpaceRatio → ruralPastureRatio` 未接入及原因；
- 同一份显式牧草地比例下，只改 profile 只影响 `urbanOpenSpaceRatio`。

revision 升级：`EFFECTIVE_PROGRAMS_REVISION = b04-c1-r1-effective-programs-v2`（旧 v1 产物全部保留，未覆盖）。

测试：`public_space_share_never_becomes_a_pasture_share`。

## 5. T4 可移植测试与旧断言

- `bevy/crates/cli/tests/morphology_compile.rs` 重写为**自给临时 fixture**（`TempDir`，不依赖任何 `work/collaboration` 包，无“缺目录 return-as-pass”）：
  1. `effective_programs_are_recompiled_from_the_authored_manifest`：自建 manifest（默认 assetContracts + 16 宅 24×24×8 + 8 店 12×10×6 + autoRoads v1 + profile）→ 编译/重编译一致；篡改发布文档 → 拒；换 profile → 拒；未知键 → 拒。
  2. `the_real_binary_binds_and_rejects_a_tampered_effective_document`：真实 `bake_world` 到临时目录 → 真实 CLI `validate-world` 通过、`quality-report` 通过 → 就地把 `effective-programs.json` 的 inset 改坏 → `validate-world` 退出 1 且报 `differs from the recompiled manifest+profile` → 还原后再次通过。
- 没有把普通测试标 ignored；没有依赖历史 work 产物的常规测试。
- `baked_audit.rs` 只改了一处旧断言字符串（`independently rebuilt remainder` → `authoritative remainder/policy`），保留 forged ownership 构造、非零退出与来源拒绝断言。该测试现通过。

## 6. T5 现有两世界道路诊断与最小修复

### 6.1 先纠正报告：profiled auto-road 确实存在

两份世界各 3 条 `grounding.roadProfile` 非空的 auto-road（compact 12/12/12m、village 12/12/8m），`auto_roads.rs:675` 经 `road_extensions::append_one` 铺设。C1 结果文档 §6/§8 的“auto_roads 不写 road_profile”诊断是错的，已在原文档追加勘误。

### 6.2 只读定位（`road-audit-probe.rs` + `road-probe-*.json`，未改世界）

按审核器完全相同的矩形与 `height_at` 复算，逐条定位最大误差样本（与已发布失败值逐毫米吻合，证明探针忠实）：

| 世界 | road | 宽 | 沿/横向半宽(旧审核矩形) | 旧 design error | 最大误差样本 | 样本位置性质 | 自然/最终/目标(mm) | 填充 |
|------|------|----|--------------------------|-----------------|--------------|--------------|--------------------|------|
| compact | road-auto.0 | 12 | 68 / 9 | 0.364 m | (211,87) 沿 51 横 **9** | 路面外、blend 带（横向 >7） | 5740 / 7964 / 7600 | 2224 |
| compact | road-auto.1 | 12 | 68 / 9 | 5.498 m | (151,156) 沿 68 横 **9** | 同上 | 1239 / 7239 / 1741 | 6000(=fill 上限) |
| compact | road-auto.2 | 12 | 68 / 9 | 0.699 m | (369,87) 沿 47 横 **9** | 同上 | 7283 / 7982 / 7283 | 699 |
| village | road-auto.0/1 | 12 | 68 / 9 | 0.364 / 5.498 m | 同上 | 同上 | 同上 | 同上 |
| village | road-auto.2 | 8 | 68 / 7 | 0（本来就通过） | — | — | — | — |

关键事实：**每一条路的“实际整平带”（横向 ≤7m）内误差都是 0.000 m**，误差样本全部落在 7–9m 的 blend/施工过渡带；`cut = 0`（只填不挖），填充未超 `maxFillMm`。8m 路的旧审核矩形横向 7m 恰好等于整平带，所以 b01–b04（全部只有 8m profiled road）从未暴露该问题；C1 引入 12m profiled road 后暴露。

阶段核查：这些世界在 bake 的 final 阶段（`bake_world` → `verify_with_style`）与 CLI `quality-report` 复算中给出同一组误差，`road-network-plan.json` 与 scene 内 plan 一致，实体 transform 与 plan 一致，故**不是“后续阶段覆盖”，也不是“生成实体/高程不一致”**。

### 6.3 根因（审核器范围与生产 carve 语义不一致）

`plan_terrain_carves` 对 profiled road 的整平带是 **沿 ±(长度/2+4)、横 ±7m**（横向为常量），其外只做 18m 平滑过渡；而 `road_network::verify_impl` 把**整个施工足迹**（横 ±(路宽/2+3)=±9m）当成“路面”，要求精确等于设计高程（容差 0.00005m）。在非平地上，blend 带永远达不到该精度，因此**任何合法的 graded road 都无法通过**——这就是本轮复现的失败机制。

### 6.4 最小修复（不改地形、不放宽容差、不跳过 profiled road）

- 新增共享定义 `worldgen::graded_flat_top_half_extents`（沿 ±(长/2+4)、横 ±7，按朝向映射到世界轴），`plan_terrain_carves` 改为读它（**数值与原来逐位相同，属纯重构，不改变任何已烘焙世界的地形**）。
- `verify_impl`：
  - 精确性只在**earthwork 实际整平的带**内要求（即整平带），与 carve 用同一定义，杜绝再次漂移；
  - 整个施工足迹仍然按土方审核：填充 ≤ 该 road 的 `maxFillMm`、**不允许下挖**（cut ≈ 0）、transform/身份比照不变；
  - 新增**覆盖规则**：若整平带横向半宽 < 路面半宽，直接报 `graded band {n}m is narrower than its {m}m pavement`，绝不静默跳过路面（当前 8/12m 满足 7 ≥ 4/6；16m 会被此规则点名，作为后续生成侧待办，见 §8）。
  - 失败信息补充最大误差样本坐标，便于定位（仍含 `changed after planning` 前缀，既有测试不受影响）。

### 6.5 修复后验证

- 只读探针（同一份已发布地形，未改世界）：6 条 profiled road 在整平带内 design error **0.000 m**，apron fill ≤ 6.000 m=上限，cut ≤ 0 → 修复后全部通过。
- 定向回归（`bevy/crates/core/src/worldgen/road_network.rs`，自给 fixture，不依赖 work 目录）：
  - **合法小控制 pass**：12m 坡地 ridge 场景，生产 `plan_with_style` 铺 1 条 graded corridor，生产 carve 生成 final，`verify_with_style` → `status=pass`、`realized_edges=planned_edges`、整平带误差 ≤0.00005m；
  - **真实扰动拒绝**：整平带边缘 +1mm → `changed after planning` 失败；下挖 0.2m → 失败；`worst_fill > maxFillMm` → 失败；
  - **覆盖规则断言**：8m/12m 的整平带覆盖路面，16m 不覆盖（`covered=false`，把 16m 的既有窄整平作为显式事实固定下来）。
- 交付的两次 512m CLI 生成（同一 C1 manifest、新编译器语义）用最终二进制复算：

| 观测 | compact-mixed-block | village-field |
|------|---------------------|---------------|
| `generate-world` | exit 0，11.8s，revision `9d95cb9d…` | exit 0，11.2s，revision `475986914c…` |
| `validate-world` | exit 0 | exit 0 |
| `quality-report` | exit 0，status=warn（4 warnings） | exit 0，status=warn（4 warnings） |
| `roadNetworkAudit` | **pass**，realizedEdges 34/34，maxDesignHeightErrorM 0.0 | **pass**，34/34，0.0 |
| `world-quality-gate` | exit 1，**6** 项（C1 时 7 项，道路审计项已消失） | exit 1，6 项 |
| 剩余 6 项 | frontage 未实现、auto-road 目标未达、purpose-area、landcover、land-use、development | 同 |

  两次生成的数据（地形/plan/entities/effective）与最终代码一致：本轮最终改动相对生成时构建只差审核带/纯重构（carve 数值逐位相同），因此 terrain 与 plan 不变；最终二进制用同一份 plan 复算得到 `pass`（见上表）。

### 6.6 frontage 缺额的真实原因（不再沿用错误诊断）

两份世界 frontage 仍 0/24：`b04c1-homes 0/16`、`b04c1-shops 0/8`，`attempted=30`，拒绝计数为 `wrong_purpose_or_protected 12`、`natural_support_or_fill_limit 12`、`existing_support_changed 3`、`occupied_or_road_support 3`。即：候选位置要么落在非 urban 分区/保留地，要么自然地形超出该项目自身的支撑/填充包络（`frontage` 的支撑合同要求整平平台 2m 缓冲内不高出且填充受限），要么与既有道路/地块支撑冲突。这是**内容缺额**，与道路工程失败分开报告，未为凑 24/24 改需求或放宽检查。

## 7. 文档口径修正（三处 + 一处事实错误）

1. `validate-world` 通过 ≠ 独立 `roadNetworkAudit` 或整世界 quality 通过（C1 两世界当时正是 validate 通过、road audit/gate 失败）。
2. effective 文档审核是 serde_json 值与结构相等，**不是原文件逐字节相等**。
3. 消费者共享同一份 effective style 正确，但 `bake_world`、`WorldManifest::validate`、`world_manifest_revision` 与 CLI 审核都会调用编译器；**不能**说“整个 bake 生命周期只调用一次”。
4. C1 §6/§8 的 auto-road roadProfile 诊断错误（见 §6.1）。

以上四条已作为醒目勘误追加在 `docs/collaboration/b04-c1-result.zh-CN.md` 顶部（历史正文与旧证据未删改）。

## 8. 预算、禁止项与旧证据

- 新的 512m 生成：**2 次**（§6.5，串行 11.8s/11.2s），对应 validate/quality/gate 各 2 次；未跑 release、最大图、240 块浏览器巡检、全 core/workspace、自然四档高程；R6 ignored 测试未执行。
- CLI crate 全量测试（既有测试二进制 + 新增）：`cargo test -p glyphweave-cli` exit 0，7+26+1+2+4+1 = **41 passed / 0 failed**（含此前陈旧的 baked_audit 断言与生态审计）。
- core 定向：morphology 24、frontage 10、auto_roads 9、road_extensions 6、road_network 9、road_parcel_feasibility 8/2 ignored、farm_program 3、residual_land 2 → 全绿。
- C1 旧世界 18 份文件 hash 与 C1 回执逐一致（未改）；两份 C1 manifest 未改；`settlement/morphology.rs` 未改。
- C1 两份世界在 v2 语义下**不再能通过有效程序绑定**（编译器语义已更新，已发布的 `effective-programs.json` 是 v1 快照），所以本轮对它们的复验只做**只读**探针，不用 CLI 重写它们的报告；对副本运行 CLI 会得到 `effective-programs.json differs from the recompiled manifest+profile`（`c1-copy-quality.log`，副本位于系统临时目录，未写入 C1 包）。C1 的 quality/gate 文件保持 v1 快照原样。
- 未 git add/commit/push/reset/checkout/stash；未启动 agent；未改 A 冻结输入/世界/runtime/证据与 F 盘；`pnpm doc-tree:check` PASS；`git diff --check` exit 0。

## 9. 未完成 / 剩余风险

- **16m graded road 的整平带仍窄于其路面**（carve 常量 7m < 8m）：本轮按“不重建道路架构、不改地形、最小修复”停在审核侧显式报点（覆盖规则），未改 carve 数值。最小变更提议：把 `graded_flat_top_half_extents` 的横向改为 `max(7, 路面/2 + 1)`（或直接 `路面/2 + 3` 与走廊 shoulder 合同一致），需单独一轮验证其对既有 graded 世界地形/审核的影响。
- CLI frontage 仍 0/24 内容缺额；strict gate 仍有 6 项内容失败；原自然 512m 案例仍 unresolved（未改、未重跑四档）。
- 六 profile 的 building footprint 缺省可用性很低（仅 perimeter/clustered × block、low-density/village × storefront）：这是修正后的真实覆盖结论，若要“只给需求”即可用，需要按建筑类型拆分 profile 或扩展 consumer 合同，属后续需求。
- `coverageRatio`、`serviceRadiusM`、`terrain` 仍 unsupported（发布文档明示）。

## 10. 结论

编译语义（未知键、地块 vs footprint、比例来源）已修正并有正负例覆盖；CLI 道路审核失败已定位为“审核范围与生产 carve 不一致”，按最小修复对齐并由自给小控制正例 + 真实扰动负例 + 两次 512m 世界复算（road audit pass 34/34）验证；内容缺额与工程问题分别如实报告。状态：**ready_for_review**。
