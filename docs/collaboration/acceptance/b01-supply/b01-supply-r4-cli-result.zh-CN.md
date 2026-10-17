# B01-SUPPLY-R4-CLI 结果：CLI 独立来源审核闭环

- 任务：`docs/collaboration/ds-v41f-b01-supply-r4-cli-task.zh-CN.md`；先读的独立审查：`docs/collaboration/b01-supply-r3-inspection.zh-CN.md`
- HEAD：`fb36dc4f9fdd38e6aa9c5a89676a65b96e5001b1`；工作区 `G:/difyllmwiki/GlyphWeave`
- 状态：**partial（ready_for_review）**。本轮只做"CLI quality-report 的来源声明审核接线"这一项；C2 内部声明链、C3、A/B/C1 按审查结论**未改**。
- 未做：无 Git 写操作、无其他 agent、无 F 盘/A 冻结材料改动；本轮冻结 128 扫描 0 次、carve 诊断 0 次、80 点时钟扫描 0 次、正式世界生成 0 次；未签收 B/C。

## 1. 漏洞与最小实现

审查所指的实际漏洞：CLI `quality-report` 调用的是公开旧包装 `road_network::verify_with_style`，它固定传
`NaturalSource::Undeclared`，因此任何 version-3 输入都会在来源重规划前被拒
（`version 3 supply requires a declared natural source`）。`cargo check` 通过不能证明该运行路径。

本轮改动（均在白名单内，无顺手重构）：

1. `worldgen.rs`：`NaturalSource` 改为 `pub`；新增共享推导
   `pub fn effective_scene_seed(world_seed, scene) = world_seed ^ scene.seed_offset` 与
   `pub fn declared_natural_source(manifest, scene)`。后者从**可信原 manifest**推导
   `AuthoredSeed { seed, precise }`（`precise = height_encoding_from_style(&manifest.style) == Millimetre`），
   并 fail-closed：manifest 未声明该 scene、或声明了本 build 无法作者化的 terrain 来源
   （例如 `"imported-raster"`）时明确拒绝，**不**猜成 `SuppliedRaster`。
   `bake_world` 的两处声明（规划与正式审核）改为调用同一函数，生成与 CLI 使用同一规则。
2. `road_network.rs`：新增公开门面
   `pub fn verify_with_manifest(record, manifest, scene, natural, final_terrain, water, entities, zones, program, style)`。
   它先用上面的规则推导来源，再进入**同一个** `verify_impl`（`trusted_record = false`）——
   来源重规划 + plan 对照 + final 支撑审核；没有走 trusted-record 叶子，也没有暴露
   `AlignmentContext`/`ZoningMask` 等内部类型。
3. `cli/src/main.rs`：`quality_report_command` 的审核调用点改走该门面，并把 `&manifest` 一并传入。
   `world-quality-gate` 先 `validate-world` 再 `quality-report`，因此同一条路径被同步。
   未改任何 gate 阈值，质量缺额照常进入 warnings/failures。
4. 旧路径行为保留：`verify_with_style`（无声明）对 v1/v2 语义不变，对 v3 仍明确拒绝；
   未恢复 seed 0，未恢复抽样 fallback。

## 2. 真实 CLI 往返（本轮最重要，实际子进程）

新集成测试 `bevy/crates/cli/tests/frontage_supply_source.rs`（自给 TempDir，**只 bake 一次**，
正负例复用同一份世界）。fixture 基于 `examples/b02-road-supply.manifest.json`：
`world.seed = 0x00C0FFEE = 12648430`（非零）、`scenes[0].seedOffset = 0xBEEF = 48879`（非零，
与 world seed 不抵消），有效 seed `= 12648430 ^ 48879 = 12599553`（非零）；
`heightPrecision = millimeter`；`automaticRoadExtensions.version = 3`，
`supply = { version: 1, maxCandidateStates: 1, preflightBudgetMs: 180000, mode: "append" }`；512×512 自然小场景。

证据：`bevy/work/collaboration/ds-b01-supply-r4-cli/cli-roundtrip.log`、`cli-roundtrip-evidence.json`。

| 用例 | 命令 | exit | 结果 |
|---|---|---|---|
| 正向：生成 | `bake_world`（测试内一次；正式 CLI 世界 0 次） | — | 世界落盘 |
| 正向：结构校验 | `glyphweave validate-world <dir>` | 0 | 通过 |
| 正向：独立审核 | `glyphweave quality-report <dir>` | 0 | **不再出现 `declared natural source` 错误**；`scenes[0].roadNetworkAudit.status = "pass"`；报告 `status = "warn"`（内容缺额，非接线错误） |
| gate 路径 | `glyphweave world-quality-gate <dir>` | 1 | 内容判定失败（`world-quality-gate.json` 的 failures 全为需求/实现类：frontage 需求未完成、`candidate_state_budget_exhausted`、joint housing 未达、land-use/landcover 未实现），**没有**来源接线错误 |
| 负例 1：源记录篡改 | `glyphweave quality-report <dir>` | 1 | `invalid baked terrain: road plan differs from natural raster reconstruction` |
| 负例 2：错 seed | `glyphweave quality-report <dir>` | 1 | `scene-0 natural snapshot seed mismatch` |

负例 1 的做法：把 `scenes/scene-0/road-network-plan.json` 与 `scene.json` 内的 plan 副本
**同步**改写 `naturalHeightfieldHash`（`...0000`），使"两份副本一致"这一较早的保护通过，
从而让拒绝**必然**来自被测来源绑定层——门面按 manifest 推导来源后重规划，重建 plan 与 record 不等。
恢复原值后正向再次 exit 0（测试内断言）。负例 2 改可信输入侧 `world.seed`，被 CLI 的
seed 绑定检查拒绝（较早的一层绑定，同样属于源绑定而非缺字段）。

### 2.1 旧无声明 v3 API 仍拒绝（未额外 bake）

新增 core 定向测试 `worldgen::road_network::tests::the_legacy_undeclared_entry_and_unknown_sources_are_refused`：
对合法 v3 style 调用公开旧包装 `verify_with_style` 仍返回
`version 3 supply requires a declared natural source …`；同一测试还断言
`declared_natural_source` 的推导结果等于 `world.seed ^ scene.seed_offset` 且 `precise = true`，
并把 `style.terrain` 改为 `"imported-raster"` 时明确拒绝（`unsupported authored terrain source`）。

## 3. 其余内容缺额（如实列出，未隐藏）

同一份正向世界的 quality-report `status = "warn"`，warnings 为：

- `scale-audit.json missing`（本测试未跑 `scale-audit`；不影响来源接线）；
- `scene-0 joint housing minimum remains unmet`：residential_minimum 18 / planned 9 / realized_with_access 9；
- `scene-0 landcover has 172446 m2 without surface realization`；
- `scene-0 road-first land-use development is incomplete`（`remaining_land_has_no_development_or_conservation_plan`、`yard_reserved_but_landcover_not_realized`）；
- `rural land realization 37762.0/196608.0 m2`（ratio 0.1441，target 0.8000）；
- `urban land realization 2784.0/16384.0 m2`（ratio 0.0106，target 0.0400）。

这些是**内容**缺额（`maxCandidateStates = 1` 的极小供应预算 + 该 profile 的 landcover/land-use 未实现），
与本轮的来源接线无关；gate 的 7 条 failures 同源。本轮不追求 24 见证、不放宽任何阈值。

## 4. 测试、预算与 hash

| 命令 | 结果 |
|---|---|
| `cargo test -p glyphweave-cli --test frontage_supply_source -- --nocapture --test-threads=1` | 1 passed，54.3s（1 次 bake + validate-world + quality-report + gate + 2 负例 + 恢复复验） |
| `cargo test -p glyphweave-core "worldgen::road_network::tests::"` | 14 passed / 1 ignored，26.6s |
| `cargo test -p glyphweave-cli --test residual_source`（v1/v2 CLI 回归） | 1 passed，25.8s |

预算：**bake 1 次**（≤512m，TempDir，正负例复用；正式世界生成 0 次）；
本轮冻结 128 扫描 0 次、carve 诊断 0 次、80 点时钟扫描 0 次；未跑全 core/CLI/workspace、release、浏览器、最大图。
`git diff --check` exit 0；`node scripts/generate-tree.mjs --check` PASS。

`source-hashes-r4.txt`：

```
628f7dfee1fc9b545dca5ba985e04707b1c1d86351563dae7ed370ca2b4d46d0  102869  bevy/crates/core/src/worldgen/road_network.rs
b191bf8a0861006a739aab0ce0ea875fc9194de79114bb0efcdb5641f57accd7  431717  bevy/crates/core/src/worldgen.rs
d0d68412f3b9980d739aa7a5c174a33b49409e19c8a7d8fb57e6299d886153e2  133402  bevy/crates/cli/src/main.rs
dc3a23296c63de5388e5c0e7cdfee419af266ee01289fba0d6b02b230fe69f8e    8875  bevy/crates/cli/tests/frontage_supply_source.rs
5c8e4fd19794bca31af3dd6f249efe448c41e9b70a7d5dd2a6d2333cca4100d5  167215  bevy/crates/core/src/worldgen/auto_roads.rs      (与 R3 相同，未改)
52622a54ae472b3744df55778450ac41dfc0648960eef975478eb019b6fe2f6b   91683  bevy/crates/core/src/worldgen/frontage_supply.rs (与 R3 相同，未改)
9d2808b76b4390b26fb0bec629e96233c04168abfa32580f497ac955106f0d67   60586  bevy/crates/core/src/settlement/landcover.rs     (与 R3 相同，未改)
```

`auto_roads.rs` / `frontage_supply.rs` / `landcover.rs` 与 R3 逐字节相同：已验收算法未动。
冻结完整性：W1 登记 7 文件 `unchanged = true`；旧 receipt 仍为 `69ff2c9b…`（L1）、`d84a59be…`（R1）、
`b3c39e92…`（R2）、`a59c1ad6…`（R3），均未改写。

## 5. 未完成项与限制

1. **仍未追求 24 见证**，本轮 fixture 的供应预算只有 1 个状态，frontage/landcover/land-use 内容缺额如 §3；
   不因此降低需求或放宽阈值。
2. `SuppliedRaster` 在 CLI 侧**没有合法来源入口**：本轮按任务要求 fail-closed（未知 terrain 来源直接拒绝），
   没有为 CLI 猜测该模式，也没有新增导入栅格的 manifest 通道。
3. 非 yard `unresolved_area_m2` 仍是总量口径（R3 已披露，本轮不扩展）。
4. R3 回执的口径问题（"本轮执行"与"R2 历史纠错"混写、把结构更严写成验证结论）已在
   `b01-supply-r3-result.zh-CN.md` §7 追加勘误；R4 回执把两者分列，历史 1 次不复制进本轮 budget。
5. 未签收 B、未推进 C 里程碑。

## 6. 勘误（B01-G1 轮追加，2026-09-18）

独立验收 `docs/collaboration/b01-supply-r4-cli-inspection.zh-CN.md` §4 的三条非阻塞记录纠正如下；只改描述，不改结论，不改旧 receipt 字节（R4 receipt 仍为 `3ad745e8…`）。

1. **整轮 bake 至少 2 次，不是 1 次。** `frontage_supply_source` 往返测试本身确为 1 次 bake；但本轮还额外跑了
   `bevy/crates/cli/tests/residual_source.rs::residual_policy_is_bound_to_the_frozen_manifest` 回归（1 passed），
   该用例自身也调用一次 `bake_world`。因此**本轮已披露命令合计至少 2 次测试 bake**（检查员本轮只运行前者，实际 1 次）。
   §4 表格里"bake 1 次"应读作"往返测试 1 次；含 residual_source 回归则整轮 ≥2 次"，而 receipt 的
   `budget.thisRound.bakes = 1` 只覆盖往返测试那一次，属口径不全。
2. **`scale-audit.json missing` 是报告时序提示，不是内容缺额。** 首次 `quality-report` 在 `scale-audit` 之前运行，
   因此提示该文件缺失；随后 `world-quality-gate` 会执行 `scale-audit` 并写出报告。它**不是**地形/容量缺额，
   也**不能**说成是"一状态供应预算"造成的。
3. **不做单因归因。** 一状态预算确实可能造成搜索不足，但本文 §3 列出的住宅/土地实现缺额
   （joint housing 18 需求只实现 9、landcover 未实现、land/rural/urban 实现比例偏低）**同样可能是独立的业务缺口**；
   没有对照证据时不应把它们全部归因于这一个预算。
4. 另外，独立验收指出当前往返测试的部分正向断言只检查"audit 对象存在 / gate 没有旧错误 / 能读报告"。
   这一点属实，后续可顺手把期望状态与退出码写成明确断言；本轮不为此再 bake、不改代码。
