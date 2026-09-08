# A-T3-core Terra：stable-v1 可选候选与存量审计候选

日期：2026-09-08（Asia/Shanghai）。状态：`ready_for_review`。
**这是窄 core 候选，不是 A-03 全部通过；legacy transport 迁移有明确待裁定边界，CLI 未集成。**

唯一仓库：`G:/difyllmwiki/GlyphWeave`。HEAD 基线：`6020fbe2713e8c79c1b2934b244402dc93e1df14`。
本任务只写下列三个 Rust 文件、本文及 `G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/terra-t3/`。既有 dirty 保留，不从 HEAD
差异中归属先前 A-T1/T2/其他协作者改动。

## 文件与 SHA-256

- `G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/worldgen.rs`
  SHA-256：`5756793adfd9a341fe2e72c3ef7bad233e2f285815378d4f5e8597863876dbde`
- `G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/worldgen/entity_identity.rs`
  SHA-256：`c4a2196a760e680c11823d724fd36c075665166a9d0b9748d56bb2247ec47b14`
- `G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/settlement/ecology.rs`
  SHA-256：`329cfdb9d2199dbcd028d54ca294e234247f589fefe3bc32aff4fe0733350c38`

本轮起始副本及 hash：
- 原 `G:\difyllmwiki\GlyphWeave\bevy\crates\core\src\worldgen.rs`：`b1a6d8c45991cbafe0a9380149ab5c448419eccd9574d9496213e72ba520bb54`
- 原 `G:\difyllmwiki\GlyphWeave\bevy\crates\core\src\settlement\ecology.rs`：`a7ccde4c549c00d548692b6d47487d846960b5649672d64f7890c9749df442c6`
保存于 `G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/terra-t3/worldgen.rs.before` 和 `G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/terra-t3/ecology.rs.before`。

## 1. 已落地的候选身份

- 新 `EntityCandidateKey`：sceneId、planner、kind、原候选 slot（整数数组）。
- `stable_id()` 对 `(stable-v1, key)` 的长度明确 JSON 序列化求 BLAKE3，输出
  `generated.<planner>.stable-v1.<digest>`；不是全 manifest hash、成功序号或最终 Vec 下标。
- 原网格 x/z 或固定 bank/row/lane/slot 为来源；即使最终经过 jitter/工程挪位，
  key 不从最终实体位置重新推导，也没有构图之后统一重命名。
- 本轮迁移：natural filler、generic auto filler、非 transport landuse proposal、
  forest belt、waterfront civic/residential、pastoral rural/well/home、resort/tower、
  storefront 两排、street-tree 两排、inland urban、template/shore 网格 filler。
- planned_id、RoadsideSite/request ID、Manifest authored ID 原样保留；不以 generated
  前缀判定一个对象可被改名。候选 ID 在创建点绑定，规则请求/图引用携带该 ID。
- 共享 `CandidateIds` 记录重复 key；失败候选不能通过补 serial 获得第二身份。
  核心生成返回 WorldgenResult，重复 key/空 ID/重复最终 ID 以 Err 终止，不发布正常结果。
- generator revision 改为
  `worldgen-2026-09-08-r50-stable-v1-optional-identity`。
- 未改自然高度、候选位置、密度、过滤/失败预算；pastoral 的 `serial % 3` 原本
  控制设施 cadence，仍保留其算法，只把设施 ID 换成网格 key。

### 未盲改的 legacy transport：需总控裁定

下游耦合均只读发现：
1. `G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/worldgen.rs` 的
   generated_intent_entity / normalize_spatial_semantics 用 ID 中 road-ns 等词决定 yaw。
2. `G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/worldgen/access.rs` 用 road ID 打破等距路由并列。
3. `G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/settlement/agriculture.rs` 按 road ID 排序。

最小复现已成为测试：
`worldgen::tests::stable_v1_legacy_transport_identity_coupling_is_explicit_not_blindly_migrated`。
把 road-ns 旧 ID 换成不保留语义的 opaque ID，实际 constructor 的 yaw 从 90° 变为 0°。
因此本轮明确保留：固定 auto/lake/river 模板基础设施 ID、intent-road-ns/ew 的旧 core
serial、landuse road-ew/road-ns/sidewalk/canal/railway 的旧 serial；单例具名模板 ID
也不做猜测性重命名。**这意味着不是所有 optional transport 都已完成 stable-v1。**
已迁移非 transport 不依赖成功计数；保留的 transport 仍存在原重编号风险。

建议下一步先给 transport/source-slot 和显式方向/稳定选择顺序单独冻结合同并授窄锁，
再迁移其 ID；对 access 等距选择和农业选择做 geometry/reference 对照。
本 worker 不改 access.rs 或 agriculture.rs，也不以减少旧测试要求绕过此耦合。

## 2. 显式存量审计 API

worldgen 边界 re-export：`audit_entity_inventory(old, new, approvals, mode)` 及 DTO。
输入是 old/new 最终 EntityInstance slice；输出逐 ID 明细，不修改任何输入。

分类规则固定：
- preserved：完整序列化字段一致；added/removed：ID 集合差异。
- moved：worldX/worldY/worldZ/yaw/bottomOffsetM 任一改变，包括 1mm offset。
- updated：其余字段变化（asset/kind/scale/footprint、anchors、其他 grounding、bounds 等）。
  同一实体同时移动并修改其他字段会同时列 moved+updated，必须分别精确批准。
- changedFields 及原因保留在结果中，不只报数量或 hash；这是身份差异，不是几何合法证明。
- 拒绝重复/空 ID、未授权 remove/move/update、通配/重复/不匹配/未使用 approval；
  approval 必须匹配精确 ID+change kind，不能 default allow-all。
- fresh 禁止非空 old；新增实体被显式列为 added，不能冒充 preserved。
- preserving 要求调用方明确提供方案元数据。legacy/stable-v1 不同即
  `migration-required`；未知相同方案也 unsupported，不按字符串前缀猜旧世界来源。

旧保存 scene 的读格式未变，不自动改写旧 ID。旧 world 与新生成候选不可静默当同一
存量；读旧数据与使用当前 generator revision 重新计算 manifest/patch revision 是
两件事。迁移必须单独批准精确映射，本候选没有隐式批量迁移。

## 3. 未集成的 CLI/门禁

现有 apply-patch 只操作 manifest landmarks，不提供 old generated inventory。
成功 apply-patch 不代表保留已发布对象；本候选只证明库 API 能审计，不声明 CLI 守住 A-03。

后续建议接口（**建议，不是已实现 flag**）：
- preserving 模式要求 BASE_WORLD（世界根+revision/hash+identity scheme）和精确 change approvals；
- 读取 old final entities，与候选 new final entities 分 scene 调用审计；任何 fail 阻止发布；
- fresh 明确不声称保存旧库存；不同 scheme 需另行 migration-required/精确迁移表；
- 通过身份审计后仍独立执行现有 geometry、gate、frontage/访问引用审核。

CLI 文件/入口生产逻辑/门禁挂接待总控后续授锁，本轮全部未写。

## 4. T2 ignored 真实对照入口（未运行）

`G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/settlement/ecology.rs` 只在测试模块追加一项
`#[ignore]`；原生产及原有测试逐文本完整保留。此 ignored 测试已随窄测试编译，但没有执行。

流程：必须明确 WORLD_DIR；BufReader 读取发布 manifest、world.json、SceneIndex；
核对 scene 身份/尺寸/origin，逐 bounded height chunk 重建 BakedTerrain，核对最终
heightfield hash，使用同一发布 patches/entities/scene spec 分别 linear/indexed
realize+独立 verify。完整序列化相等，并同发布 ecology 比较；原输入不匹配会明确断言失败，
不把旧世界错误地批准为同输入。canonical path 防输入逃逸，测试不写 world。
报告 worldRevision、sceneId、entities/patches/plants/rejections、输出 BLAKE3、
两路真实 SAT 计数与耗时。缺 WORLD_DIR 显式失败，不默认选择旧 r49。

总控后续一次性 RUN 对照命令：先在该进程明确设置环境变量 WORLD_DIR 为**获准且来源冻结的
绝对 world 根**（内含 glyphweave.manifest.json/world.json），然后执行：

```text
cargo test --manifest-path G:/difyllmwiki/GlyphWeave/bevy/Cargo.toml -p glyphweave-core settlement::ecology::tests::real_world_dir_linear_indexed_ecology_equivalence -- --exact --ignored --test-threads=1 --nocapture
```

该 test 遍历根内发布场景，明确相同输入才断言等价；旧缺 1 株不能在此被消除。
**本轮此命令 not_run。** 线性最大图可能很慢，不能将它当成已完成的 600s/8192MiB gate。

## 5. 已运行窄验证与证据

- `cargo test --manifest-path bevy/Cargo.toml -p glyphweave-core worldgen::entity_identity::tests:: -- --test-threads=1`
  退出码：0；日志：`G:\difyllmwiki\GlyphWeave\bevy\work\collaboration\terra-t3\worldgen-entity_identity-tests.log`
- `cargo test --manifest-path bevy/Cargo.toml -p glyphweave-core worldgen::tests::stable_v1_ -- --test-threads=1`
  退出码：0；日志：`G:\difyllmwiki\GlyphWeave\bevy\work\collaboration\terra-t3\worldgen-tests-stable_v1_.log`
- `cargo test --manifest-path bevy/Cargo.toml -p glyphweave-core worldgen::tests::patch_chain_rejects_stale_base_revision_and_changes_revision -- --test-threads=1`
  退出码：0；日志：`G:\difyllmwiki\GlyphWeave\bevy\work\collaboration\terra-t3\worldgen-tests-patch_chain_rejects_stale_base_revision_and_changes_revision.log`
- `cargo test --manifest-path bevy/Cargo.toml -p glyphweave-core worldgen::tests::authored_identity_does_not_require_landmark_prefix -- --test-threads=1`
  退出码：0；日志：`G:\difyllmwiki\GlyphWeave\bevy\work\collaboration\terra-t3\worldgen-tests-authored_identity_does_not_require_landmark_prefix.log`
- `cargo test --manifest-path bevy/Cargo.toml -p glyphweave-core worldgen::tests::generated_spatial_semantics_infer_road_rotation_and_building_front -- --test-threads=1`
  退出码：0；日志：`G:\difyllmwiki\GlyphWeave\bevy\work\collaboration\terra-t3\worldgen-tests-generated_spatial_semantics_infer_road_rotation_and_building_front.log`
- `cargo test --manifest-path bevy/Cargo.toml -p glyphweave-core worldgen::tests::explicit_intent_uses_generic_settlement_planners -- --test-threads=1`
  退出码：0；日志：`G:\difyllmwiki\GlyphWeave\bevy\work\collaboration\terra-t3\worldgen-tests-explicit_intent_uses_generic_settlement_planners.log`
- `cargo test --manifest-path bevy/Cargo.toml -p glyphweave-core worldgen::entity_identity::tests:: -- --test-threads=1`
  退出码：0；日志：`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/terra-t3/identity-final.log`
- `node bevy/work/collaboration/terra-t3/check-static.mjs`
  退出码：0；日志：`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/terra-t3/static-check.log`
- `node scripts/generate-tree.mjs --check`
  退出码：0；日志：`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/terra-t3/tree-check.log`
- `git diff --check -- bevy/crates/core/src/worldgen.rs bevy/crates/core/src/settlement/ecology.rs bevy/crates/core/src/worldgen/entity_identity.rs`
  退出码：0；日志：`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/terra-t3/diff-check.log`

累计独立测试名称覆盖：identity 模块 4 项、新 stable_v1 生成/patch 5 项、旧精准回归
4 项；均通过且非零匹配。identity-final 为补充编译/复跑，仍 4/4。
包括无关 authored/optional 不改既有候选、前序被阻挡不重排存活对象、scene namespace、
小 modern/pastoral 重复生成一致、精准授权/拒绝及序列化 roundtrip、旧 patch 移动/设置/
移除和未知对象失败、author 不依赖 landmark 前缀。
首个编译有未使用 serial 赋值提示（已移除不再使用的 forest-belt 尾部赋值），无测试失败。

静态证据：`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/terra-t3/static-evidence.json`。
其检查保留旧 A-T1 构图顺序、apply_patch、signed_noise/natural sanitation 不变，
以及 T2 只追加 ignored 测试。完整机器回执：`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/terra-t3/receipt.json`。

## 6. 未验证项和停止状态

- legacy transport 完整 stable-v1 迁移未完成，需总控裁定窄依赖改动；最小复现已交。
- 新 revision 的 release/bake/CLI 集成和 final inventory gate 未运行/未挂接。
- 未运行完整 workspace、真实 T2 ignored 对照、6×10km、大图 gate、HTML或服务。
- 无全域自然/保护/引用一致性新证明；不宣布 A-03/A 已完成。
- 未写 access.rs、Luna HTML/CLI、lib.rs、其他 Rust、资产/输入、review、F:/modenmapbox。
- 未新增模型或依赖；未执行 Git 写操作/暂存/提交/推送/全库 fmt。
- 所有本轮窄测试已结束，无后台句柄，交候选后等待总控下一步裁定。
