# B04-C1 结果：形态 profile 真实编译并接入生成/审核

日期：2026-09-16（Asia/Shanghai）。状态：**ready_for_review**（不签收 B、不推进 C）。

> **勘误（2026-09-16，B04-C1-R1 追加；历史正文与旧证据均未删改）**
>
> 1. §6 与 §8 把 CLI frontage 0/24 归因于“auto_roads 不写 road_profile”是**错的**：两份世界的 `generated.road-auto.scene-0.*` 都带非空 `grounding.roadProfile`（compact 12/12/12m、village 12/12/8m），`auto_roads` 经 `road_extensions::append_one` 铺设它们。正确的缺额解释见本轮 `b04-c1-r1-result.zh-CN.md` §5。
> 2. “`validate-world` 通过”只证明结构/身份/签名合同，**不等于**独立 `roadNetworkAudit` 或整世界 quality 通过：两份世界当时都是 `roadNetworkAudit.status=fail`、`world-quality-gate.status=fail`（7 项）。
> 3. “原文件逐字节比较”措辞不准确：`effective-programs.json` 的独立审核是 **serde_json 值与结构相等**，不是文件空白/键顺序逐字节相等。
> 4. “一次编译”措辞不准确：`bake_world`、`WorldManifest::validate`、`world_manifest_revision`（以及 CLI 审核）各自都会调用编译器；正确说法是**同一份输入得到同一份 effective 程序**，而不是整个生命周期只调用 compile 一次。
> 5. 本轮已修正编译语义：`parcelWidthM/parcelDepthM` 是**完整地块**，缺省建筑 footprint 由“地块 − 院落”推导（不再直接当地块当建筑）；`publicSpaceRatio` 不再派生 `ruralPastureRatio`；未知键不再被编译器吞掉；effective revision 升级为 `b04-c1-r1-effective-programs-v2`，并新增严格解析/地块推导/比例语义测试。C1 两份旧世界保留为 v1 语义历史产物，未在本轮重编译核对。

本轮把已有的六个 `MorphologyProfile` 从"只 resolve/validate"变成"编译成 effective programs 并真实驱动 Rust 生成与独立审核"。R6 约定队列不重开，不改 `road_parcel_feasibility.rs` 及其 joint/支撑 checker；原自然 512m 困难案例保持 unresolved，未重跑四档高程。

## 1. 身份、基线与白名单

- 仓库 `G:/difyllmwiki/GlyphWeave`，分支 `refactor/authoritative-baked-audit`，HEAD `fb36dc4f9fdd38e6aa9c5a89676a65b96e5001b1`（未提交）。
- 进入本轮时的 A 基线 `42cb4973fe6fdf985d21ee2a1f4be3c0ce10382b`；A 冻结证据、F 盘、原世界/runtime 未触碰。
- 本轮新增/修改（源码增量，SHA256 全量见 `bevy/work/collaboration/ds-b04-c1/receipt.json`）：

| 文件 | 角色 | bytes | SHA256（前 16） |
|------|------|-------|------------------|
| `bevy/crates/core/src/settlement/morphology_compile.rs`（新增） | 确定性编译 + 来源表 + 独立重编译 | 47316 | `4c2b7053a4ca3f02` |
| `bevy/crates/core/src/settlement.rs` | 仅新增 `pub mod morphology_compile;` | 39757 | `28eab83c9d3fd96c` |
| `bevy/crates/core/src/worldgen.rs` | 编译入口、effective 传递、revision 绑定、测试 | 414820 | `42a6aaf5dbf45fd2` |
| `bevy/crates/cli/src/main.rs` | 独立重编译/来源审核、审核消费 effective、报告接线 | 132348 | `bfa3282d283f1029` |
| `bevy/crates/cli/tests/morphology_compile.rs`（新增） | CLI 端到端复验 | 5841 | `a6eed842b2eaf3e0` |
| `examples/morphology/b04-c1-compact-mixed-block.manifest.json`（新增） | CLI 控制 A | 29208 | `6647ad66469bb6eb` |
| `examples/morphology/b04-c1-village-field.manifest.json`（新增） | CLI 控制 B | 29196 | `b4946e0bc7e8f32d` |

白名单外文件未改：`development.rs`、`landcover.rs`、`landuse_ledger.rs`、`road_parcel_feasibility.rs`、`parcel_geometry.rs`、frontage/services/auto_roads/road_extensions 的既有语义均未修改（本轮的 `worldgen.rs`/`main.rs` 改动仅限编译入口与 effective 传递/审核接线）。`bevy/crates/core/src/settlement/morphology.rs` 未改（沿用 B04 的六 profile 定义），其 SHA256 `a17718fdd192ace1`。

## 2. 编译接口与字段映射/优先级

`morphology_compile::compile(style, scenes) -> Ok(None|Some(CompiledStyle))`：

- 无 `morphologyProfile` → `None`，legacy 语义逐字节不变（测试 `legacy_manifest_without_profile_is_byte_identical`）。
- 有 profile → 返回 `CompiledStyle{style, report}`；`report` 就是发布的 `glyphweave.effective-programs` v1 文档，内含 `effectiveStyle`（生成真正消费的对象）、`manifestStyleHash`、`effectiveStyleHash`、`fieldSources`、`unsupported`。
- 范围→标量规则（唯一处）：`midpoint(range)`；米制量向下取到偶数，比例量四舍五入到 3 位小数。该规则写在模块头注释里，属工程参数，不宣称为国家实测标准。
- 编译结束前用各消费者**自己的严格读取器**再验一次（roadExtensions / settlementProgram / autoRoads / frontage / farm / residual），任何 profile 默认值都不得放宽合同。

字段映射与优先级（`original-input` > `profile-default` > `derived`；`unsupported` 明示不生效）：

| profile 字段 | 消费位置 | 规则 | 来源标记 | 备注 |
|--------------|----------|------|----------|------|
| `parcelWidthM` / `parcelDepthM` / `heightM` | `frontageProgram.requests[*].widthM/depthM/heightM` | 仅当请求未写该字段时填 midpoint（偶数） | profile-default | 越出 consumer/asset 合同 → **报错拒绝**，不缩小 |
| `frontInsetM` / `sideInsetM` / `rearInsetM` | `frontageProgram.requests[*].parcelInsets.{frontM,sideM,rearM}` | 仅当请求未写 `parcelInsets` 时填 midpoint | profile-default | 逐个字段入 `fieldSources` |
| `roadHierarchy` | `automaticRoadExtensions.scenes[*].hierarchy`（并把 program 升级到 v2） | 角色来自 profile，宽度用模块内角色表 | profile-default + derived | 仅当声明了 `automaticRoadExtensions` 且该 scene 未写 hierarchy |
| `publicSpaceRatio` | `residualLandProgram.scenes[*].{urbanOpenSpaceRatio,ruralPastureRatio}` | midpoint，超出 consumer 边界（urban 0.25 / rural 0.50）→ 拒绝 | profile-default | 缺 program 时按 scene 生成 |
| `villageFieldTemplate` | `agricultureProgram.scenes[*].defaultField` | 直接映射 `{widthM,depthM}` | profile-default | 仅对确有 rural 分区的 scene；`naturalOnly` 或缺 `landUseProfile` 时标 unsupported |
| `coverageRatio` | — | 无消费者 | **unsupported** | 明示"管线没有覆盖率目标" |
| `serviceRadiusM` | — | 无消费者 | **unsupported** | 设施只由显式请求程序驱动，不用半径 |
| `terrain` | — | 描述性元数据 | **unsupported** | manifest 的 terrainProfile 仍是权威 |

被 profile 明文拒绝的默认值（防"悄悄缩小"）：`widthM/depthM/heightM` 由 profile 供给时，只有 `village-field` 的 22×34×12（residential_home）能过 consumer/asset 合同；其余五个 profile 的默认尺寸被合同拒绝并直接报错，例如 `compact-mixed-block` 默认高 16m > 宅高度上限 14m。六个 profile 的 `parcelInsets` 默认值（4/2/4、4/4/22、8/8/26、6/4/16、4/2/8、6/4/20）全部合法，故 inset 通路对六个 profile 均可用；测试 `profile_dimension_defaults_are_accepted_or_refused_never_shrunk` 固定该表。

需求侧绝不发明：请求缺 `id/sceneId/purpose/kind/count` → 报错（"a morphology profile never allocates buildings"）；缺 `stationSpacingM/setbackM/maxCandidateAttempts` → 报错（profile 无该消费者映射）。同一输入两次编译逐字节相同，追加无关请求不重排既有 id（`compilation_is_deterministic_and_request_order_is_stable`）。

## 3. 生成与审核接线（同一编译规则）

- `bake_world`：`manifest.validate()` 后**只编译一次**，得到 `effective_style`；场景循环内 road_extensions / auto_roads / frontage / services / farm / plan_landcover_with_program 全部改用该 `effective_style`（`worldgen.rs` 内 6 处 `&manifest.style` → `&effective_style`）。生成前把 `report` 写入 `effective-programs.json`。
- 发布态保持原样：`glyphweave.manifest.json` 仍是**用户原始 manifest**（`style.morphologyProfile` 保留、frontage 仍无 `parcelInsets`、autoRoads 仍 v1、无 residual/agriculture），原需求不丢；展开结果只存在于 `effective-programs.json`。
- `WorldManifest::validate()` 现在用编译后的 effective style 跑各 program 校验器（profile 补默认后仍走原合同），morphology 身份校验仍打在原始 style 上。
- `world_manifest_revision`：**仅当** manifest 带 profile 时，才把 `effectiveProgramsRevision` + `effectiveStyleHash` + 来源表并入 revision payload；无 profile 的世界保持历史 payload 不变（因此冻结 A 世界的 revision 仍可验证）。
- 独立审核：CLI 新增 `audit_effective_programs()`，从**原始 manifest+profile** 重新 `compile` 并与发布的 `effective-programs.json` 做整文档比较（不信任生成器写出的报告）；`validate-world` 与 `quality-report` 都调用它，且 `quality-report` 后续的 service/road/farm/residual 审核改用重编译出的 effective style。
- generator 身份：`WORLD_GENERATOR_REVISION` 未改（避免让冻结 A 证据失效），新增独立标签 `EFFECTIVE_PROGRAMS_REVISION = "b04-c1-effective-programs-v1"`，同时出现在 `effective-programs.json.revision` 与 `generation-report.json.effectivePrograms.revision`。

## 4. 定向测试

`cargo test -p glyphweave-core --offline morphology` → **18 passed / 0 failed**（日志 `test-morphology.log`）：

1. 六 profile 有效展开、来源表、unsupported 声明（`six_profiles_expand_with_sources_and_declared_unsupported_fields`）
2. 无 profile 逐字节不变（`legacy_manifest_without_profile_is_byte_identical`）
3. 类型错误/未知 ID（`profile_type_and_unknown_id_are_rejected`）
4. 未写几何→默认；显式几何优先（`unstated_geometry_is_defaulted_and_explicit_geometry_wins`）
5. 缺 count 拒绝（`a_missing_count_is_never_allocated_by_the_profile`）
6. 不相容默认值拒绝而非 clamp（`incompatible_default_is_rejected_instead_of_clamped`）
7. 缺搜索预算拒绝（`a_missing_search_budget_is_not_invented`）
8. 同 seed/输入确定性 + 请求顺序稳定（`compilation_is_deterministic_and_request_order_is_stable`）
9. village-field 打通 hierarchy/residual/fieldTemplate（`village_field_profile_wires_hierarchy_residual_land_and_field_template`）
10. 无 fieldTemplate 的 profile 不假装生成 agriculture（`a_profile_without_a_field_template_declares_it_absent_not_defaulted`）
11. 发布报告被重编译核对，篡改/换 profile 版本被拒（`published_report_is_recompiled_from_source_not_trusted`）
12. 原 B04 example 的显式程序全部保持（`b04_village_field_example_keeps_every_explicit_program`）
13. 尺寸默认值接受/拒绝表（`profile_dimension_defaults_are_accepted_or_refused_never_shrunk`）
14. 两个 CLI 控制同需求编译（`cli_control_manifests_compile_from_the_same_authored_demand`）
15–17. 真实函数集成三条（见 §5）
18. `settlement::morphology::tests::six_profiles_are_distinct_valid_and_unknown_ids_fail`（既有）

其它定向回归（全绿，日志同目录）：`worldgen::frontage` 10、`worldgen::auto_roads` 9、`worldgen::road_extensions` 6、`worldgen::road_network` 8、`settlement::farm_program` 3、`settlement::residual_land` 2、`worldgen::road_parcel_feasibility` **8 passed / 2 ignored**（与 R6 记录一致，未改该模块）。

CLI 端到端测试 2 passed（`bevy/crates/cli/tests/morphology_compile.rs`）。

已知既有失败（**非本轮引入，白名单外**）：`cargo test -p glyphweave-cli` 里 `tests/baked_audit.rs::quality_report_rebuilds_development_and_rejects_forged_ownership` 期望旧消息 `landcover plan differs from independently rebuilt remainder`，而 R1 的"residual 来源绑定"已把 CLI 改为调用带 `expected_residual` 的 `development::audit_landcover`，现在输出 `landcover plan differs from authoritative remainder/policy`（`b-review-r1-result.zh-CN.md` §F1 记录该改动，R1 只跑了 `--test residual_source`）。该测试与 `development.rs` 都不在 B04-C1 白名单内，本轮未改，保留原报告口径。另 `ecology_audit_accepts_valid_layers_and_rejects_both_copies_forged` 在一次 26 并发整包运行时失败、单独运行通过（39.6s），判为并发资源型波动，不据此宣称其已修好或已坏。

## 5. 真实函数集成（两个 profile 实际影响几何）

用 R4/R6 已验收的**平地可行控制**做输入（512×512、`levelM=0`、urban 全场景分区、底座 `fixture.base` 节点 (32,160)-(96,160)），由 `road_extensions::append` + `frontage::append` 这两条生产函数消费**编译后的 effective style**；同一 16 住宅 + 8 店铺需求、同 seed，只改 profile：

| 输入 | 每请求计划/需求 | 院落边界(宽×深) | 依据 |
|------|------------------|------------------|------|
| 无 profile（legacy 2/2/2） | 16/16、8/8 | 16×14 | legacy 语义未变 |
| `compact-mixed-block`（4/2/4） | 16/16、8/8 | 16×18 | profile-default inset |
| `clustered-residential`（6/4/16） | 16/16、8/8 | 20×32 | profile-default inset |

三组 `stopReason=demand_satisfied`、`unfilled=0`，未削减需求；`compact_plan.roadside_sites[*].parcel_insets` 为 `Some`、legacy 为 `None`，两者发布计划 JSON 不等（`two_profiles_change_real_parcel_geometry_on_the_feasible_control`）。第二条集成测试确认同 profile 两次运行逐字节相同、不同 profile 的 plan 不同。

本轮同时暴露该控件的真实工程耦合（如实记录，未改验收口径）：`frontage` 的站位把建筑放在 `road_half_depth + setback + building_half_depth` 处，于是院落后缘与路面不重叠要求 `setbackM ≥ frontInsetM + 4`，且 `road_half_depth + setbackM ≤ 40`（`validate_entity` 的连接长度上限）。上表用 `setbackM=12`/`stationSpacingM=24` 使两 profile 同时可行。

## 6. 小型 CLI 验证（两次 512m 生成）

控制清单：`examples/morphology/b04-c1-*.manifest.json`，同一 512 场景、同 seed、同 landUseProfile、同 16+8 显式需求与同 `automaticRoadExtensions` v1，仅 `morphologyProfile` 不同（A=`compact-mixed-block`、B=`village-field`）。未给 CLI 加任何"跳过审核/强制平坦"入口。

命令与耗时（串行、各 <120s）：

| 步骤 | A compact | B village-field | 日志 |
|------|-----------|-----------------|------|
| `generate-world` | exit 0，13.1s，revision `92264f05…` | exit 0，12.7s，revision `ee067109…` | `generate-*.log` |
| `validate-world` | exit 0 | exit 0 | `validate-*.log` |
| `quality-report` | exit 0，status=warn（5 warnings） | exit 0，status=warn（5 warnings） | `quality-*.log` |
| `world-quality-gate` | exit 1，status=fail（7 项） | exit 1，status=fail（7 项） | `gate-*.log` |

两个 profile 的实际差异（同一需求、同一地形，只有编译结果不同）：

| 观测 | compact-mixed-block | village-field |
|------|---------------------|---------------|
| `effectivePrograms.profileId` | `compact-mixed-block` | `village-field` |
| 来源条目 / unsupported | 23 / 3 | 25 / 3 |
| `effectiveStyleHash`（前 16） | `813504360134a103` | `2aad00df68755022` |
| autoRoads tiers | collector:16, local:12 | collector:16, local:12, **farm_access:8** |
| 实际道路宽度分布（scene entities） | 8m×31 + 12m×3 | 8m×32 + **12m×2** |
| `landcoverPlan.residualProgram` | 0.10 / 0.10 | 0.14 / 0.14 |
| `landcoverPlan.farmPattern` | 无 | `{defaultField:{24,24}}` |
| `landcoverPlan.cells` | 364 | 363 |
| `quality-report` 城市已实现比例 | 0.1879（49260/180224 m²） | 0.2097（54965/180224 m²） |
| 农村已实现比例 | 0.0511 | 0.0570 |

即：profile 的道路层级、公共空间/牧草地比例与田块模板确实穿透到生产生成物的道路宽度、landcover plan 与实现面积，而不是只改了一个字符串。

原输入与 effective 绑定（读回两个世界核对）：`glyphweave.manifest.json` 仍是原始 style（`morphologyProfile=compact-mixed-block|village-field`、request 无 `parcelInsets`、autoRoads v1、无 residual/agriculture、需求 16/8 完整）；`effective-programs.json` 内 `effectiveStyle` 才有 insets(4/2/4 或 6/4/20)、autoRoads v2、residual、agriculture。`validate-world` 用原始 manifest 重编译后与发布文档逐字节一致才通过。

**CLI 内容缺额（如实）**：两个世界的 frontage 需求都未实现——`b04c1-homes 0/16`、`b04c1-shops 0/8`，`attempted=30`，拒绝原因是 `wrong_purpose_or_protected 12`、`natural_support_or_fill_limit 12`、`existing_support_changed 3`、`occupied_or_road_support 3`。接口层原因已定位：`frontage::append` 只接受 `grounding.road_profile` 非空的 graded 道路（`frontage.rs:522-525`），而 `auto_roads::append` 生成的道路不写 `road_profile`；要真正实现本地高程 frontage 必须声明 `roadExtensions`，而 `road_extensions::append_one` 又强制两端固定接点高程且起点必须是已有道路节点（`road_extensions.rs:155-177`）。在程序化 hills/plains 地形上这需要按生产函数先解出合法走廊高程，本轮未把它塞进 CLI 控制（否则会为了演示而改地形或放宽约束）。因此 `world-quality-gate` 在两个世界都 fail，7 项与缺额一致：frontage 未完成、automatic road 目标仍有 1 个不可达、最终路网独立设计审核失败、purpose-area/landcover/land-use/development 未完成。这是"CLI 内容仍 partial"，不与 §5 的函数级几何正例混同。

## 7. 正例与负例

- 正例：§5 两个 profile 的可行平地控制（24/24 + 不同院落几何）；§6 两个 CLI 世界生成成功且 `validate-world` 通过。
- 负例（CLI 实跑）：把 `effective-programs.json` 的 `urbanOpenSpaceRatio` 从 0.1 改成 0.2（其余字节同正例）→ `validate-world` 退出 1：`effective-programs.json differs from the recompiled manifest+profile`（目录 `negative-effective-tamper-1/`，日志同名 `.log`）。把发布 manifest 的 `morphologyProfile` 改成 `village-field` → 同样退出 1 且报同一句（`negative-profile-mismatch-1/`）。两者只是正例世界的副本，便于逐字节对比。
- 负例（定向测试）：profile 类型错误、未知 profile、profile 覆盖显式尺寸、删/缺 count、缺搜索预算、profile 默认越界、伪造 effective 文档、换 profile 版本比对——均按预期拒绝（§4 测试 2–6、11 与 13）。
- 工程与固定入口负例仍由既有 `worldgen::road_parcel_feasibility`（8 passed / 2 ignored）与 R6 已验收证据承担，本轮未改其支撑/来源/容差。

## 8. 未完成 / unsupported / 需要后续

- `coverageRatio`、`serviceRadiusM`、`terrain` 仍无消费者，已在发布文档中显式 `unsupported`，不冒充生效。
- CLI 控制的 frontage 0/24 与 strict gate fail 未解决；接口阻断点见 §6（graded 道路/固定接点高程）。原自然 512m 案例仍 unresolved，未被替换或改写。
- 服务路径距离、完整研究来源、多场景最大图、240 块浏览器巡检按 B04/B05 后续需求，不在本轮。
- `widthM/depthM/heightM` 的 profile 默认目前仅 `village-field` 能通过 consumer 合同；其余五个 profile 的尺寸默认会在请求省略时直接报错（这是设计上的 fail-closed，不是缺失，但若要六个 profile 都能"只给需求"还需按建筑类型拆分 profile 或扩展 consumer 合同，属后续需求）。

## 9. 运行预算与禁止项

- 新 512m **CLI** 生成：2 次（§6，各 13.1s/12.7s，串行；另有 2 次 `validate-world`、2 次 `quality-report`、2 次 `world-quality-gate`，均 0.4–3.1s）。
- 为验证 `main.rs`/`worldgen.rs` 改动，另外跑了 CLI crate 的既有测试二进制（`baked_audit` 26 项、`inventory_preserving` 1 项、`preview_parity` 4 项、`residual_source` 1 项、新增 `morphology_compile` 2 项）。这些测试内部用 `bake_world` 在临时目录各自烘焙 512m 世界（属库级测试，不是 CLI 生成运行，也不产生交付世界）；`baked_audit` 里正是这样暴露出 §4 记录的既有陈旧断言。core 未跑全量（只按过滤器跑 §4 列出的定向套件）。
- 未运行：release 构建、全 workspace/全 core 测试、最大图、240 块浏览器巡检、原自然四档高程扫描。
- 未提交/未推送/未切分支（`git status`/`git diff --check` 见日志，diff-check exit 0）；未启动其它 agent；未改 A 冻结证据、原世界/runtime、F 盘；未改 R6 joint/支撑 checker。
- `pnpm doc-tree:check` PASS（`doc-tree-check.log`）。

## 10. 结论

profile 已真实编译为 effective programs 并被生成与独立审核同时消费；原输入/展开结果/实际产物三者分别绑定且可复核；至少两个 profile 在可行控制上产生真实几何与地表执行差异。CLI 内容仍 partial（frontage 未实现、strict gate fail），按任务要求如实披露，不签收 B、不推进 C。状态：**ready_for_review**。
