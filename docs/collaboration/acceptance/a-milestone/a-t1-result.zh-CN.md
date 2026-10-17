# A-T1 结果：固定 frontage 入口与最终访问图一致性

日期：2026-09-08（Asia/Shanghai）  
任务：A-T1  
状态：`ready_for_review`（仅候选；尚未由总控验收，不代表 Milestone A 或整图质量通过）  
基线提交：`6020fbe2713e8c79c1b2934b244402dc93e1df14`

## 1. 范围与未触碰部分

本任务仅修改了下列 A-T1 授权文件：

- `bevy/crates/core/src/worldgen.rs`
- `bevy/crates/core/src/worldgen/access.rs`
- `bevy/crates/cli/tests/baked_audit.rs`
- 本文档及同任务机器回执。

未修改 ecology、landcover、pedestrian index、HTML、CLI gate、manifest、资产、许可证或任何最大图产物；未启动 preview，未运行最大图、完整 workspace、Node/Vitest 或全域 HTML 回归；没有新增依赖或外部代码/资产。`F:\modenmapbox` 未修改。

任务执行期间共享工作树还出现了 `engineered_placement.rs`、`frontage.rs`、`road_network.rs`、`services.rs` 和 `scripts/collaboration/` 的非本任务变动。本候选没有编辑、格式化、暂存或归属这些文件；总控审查时必须将它们与本任务的三个源码/测试修改分开。

## 2. 发现的问题

原顺序是：

1. 对可生成建筑调用 `place_generated_entrances`，得到 `accessGraph`；
2. 随后将带 `grounding.frontageRoadPoint` 的 frontage 实体整体替换回 `RoadsideSite::entity()`；
3. 用前一步图生成/保存访问路面。

因此若 generic facade fallback 曾移动固定 frontage 的 `front` anchor，持久化图可能描述替换前实体，而最终实体和最终访问路面描述替换后实体。这个问题不能靠构图后重置实体掩盖，也不能把失败站点删除。

## 3. 实现

### 3.1 固定 frontage 不进入 generic facade fallback

`access::place_generated_entrances` 现在要求：仅当
`entity.grounding.frontage_road_point.is_none()` 时，才允许对未连通的
synthetic generated building 尝试四面 facade fallback。

有 `frontageRoadPoint` 的实体已由 Rust 规划同时确定：建筑位置、毫米底部偏移、yaw、门 anchor、局部连接工程和道路接点。若其固定门不能路由，函数保留原实体与失败 `AccessLink`；不改门、不换建筑、不降低约束。

### 3.2 删除构图后的实体回写

`bake_world` 不再在构图之后恢复 frontage 实体。现在 `accessGraph` 和
`accessSurfaces` 都来自同一个最终 `entities` slice。对此只增加顺序说明；不改变道路、地形、保护区或质量门禁。

## 4. 回归与负向证明

### 4.1 红测（修复前）

新增的窄 fixture 将 generated building 标为固定 frontage，保留北侧门，使用墙阻挡该门；普通 generated building 在同一几何情形下能够选择另一侧门。

在加入 `frontageRoadPoint.is_none()` 条件前，命令：

```text
cargo test --manifest-path bevy/Cargo.toml -p glyphweave-core fixed_frontage_entrance_is_never_reassigned_even_when_another_facade_is_reachable
```

退出码为 `1`，断言记录原门 `(64, 58)`，实际被 fallback 改为 `(70, 64)`。这证明旧逻辑确实重写了固定入口，而非仅有顺序上的理论风险。

### 4.2 绿测

修复后同一 fixture 验证：

- 固定 frontage 的门坐标不变；
- 原门被阻挡时实体/图报告 `0 connected / 1 disconnected`，失败保留；
- `place_generated_entrances` 返回图与对最终实体执行
  `build_access_graph` 的独立重建逐 JSON 相等；
- 普通非固定 generated building 的旧 fallback 仍可换门；
- landmark/authored doorway 的既有不可改写测试仍通过。

512m `examples/local-height-frontage.manifest.json` 集成测试还验证：

- 持久化 `scene.accessGraph` 与 `quality-report` 的独立最终实体重建相等；
- 篡改固定 frontage 的 anchor 后，`quality-report` 非零退出；
- 篡改固定 frontage 的 `grounding.frontageRoadPoint` 后，`quality-report` 非零退出；
- 恢复原 scene 后，原有 frontage、道路、保护区、服务和 1mm 高程篡改负向检查继续通过。

这不是最大图证明。小图中原有的 8 个需求和 3 个住宅缺额没有被改写；本任务没有运行 `world-quality-gate` 来将其包装为通过。

## 5. 已运行命令

完整输出与退出码在：

`G:\difyllmwiki\GlyphWeave\bevy\work\collaboration\terra\a-t1-targeted-tests.log`

所有以下命令退出码均为 `0`：

```text
cargo test --manifest-path bevy/Cargo.toml -p glyphweave-core worldgen::access::tests::fixed_frontage_entrance_is_never_reassigned_even_when_another_facade_is_reachable
cargo test --manifest-path bevy/Cargo.toml -p glyphweave-core worldgen::access::tests::generation_selects_another_facade_but_never_rewrites_authored_door
cargo test --manifest-path bevy/Cargo.toml -p glyphweave-core worldgen::frontage::tests::negative_millimetre_frontage_is_not_raised_by_legacy_quarter_metre_rebasing
cargo test --manifest-path bevy/Cargo.toml -p glyphweave-core worldgen::road_network::tests::roadside_front_doors_face_the_assigned_road
cargo test --manifest-path bevy/Cargo.toml -p glyphweave-cli local_height_frontage_bakes_real_city_buildings_and_rejects_forged_plans -- --test-threads=1
```

最后一项是一次 512m 小图 bake + CLI `quality-report` 集成测试，耗时约 21.67 秒；没有使用旧 r49 runtime，也没有启动 release binary。

## 6. 未证明事项 / 交总控验收

- 未跑完整 workspace、最大 6×10km、独立最大图 gate、HTML 或性能测量；旧 r49 结果不能自动证明当前源码。
- 未重新证明所有四个 frontage yaw、所有非零 origin、自然 hash、保护区样本或总土地账本；已有窄测试覆盖负毫米、非零 origin 的 road-network fixture 和道路门朝向，但不构成全域重验。
- 仍需总控按 v2 验收合同审查差异、确认当前共享工作树的其他协作者变动、决定是否授予更大规模运行槽。

本协作者已停止，不自动开始 A-T2。