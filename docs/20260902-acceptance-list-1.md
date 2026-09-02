# 规则引擎验收清单

- **日期**：2026-09-02
- **版本**：第 1 版
- **适用范围**：`bevy/crates/core/src/rules/`、worldgen 接入、CLI 审计与对象 descriptor

> 使用方式：每个发布候选版本都逐项勾选。所有“必须”项通过，才能标记为“规则引擎 MVP 完成”。

## A. 描述文件与资源

- [x] 仅加载 `*.object.toml`；普通 TOML 不会被误解析为对象规则。
- [x] 顶层与嵌套 section 都拒绝未知字段。
- [x] geometry、distance、weight、slope、anchor side、重复 anchor id 有校验。
- [x] 空 rules 目录、缺失目录、损坏 TOML 均返回非零。
- [x] `fallback = shrink` 明确拒绝；不会静默降级。
- [x] `rules check-assets DIR ASSET_ROOT` 能发现缺失资产。
- [x] 当前基准 `assets/objects/` 可通过资产检查。
- [x] descriptor id / canonical kind / `applies_to` 重复覆盖会报错。

## B. 规则表达与验证

- [x] InsideBounds。
- [x] OnGround。
- [x] NotInWater。
- [x] MaxSlope。
- [x] AvoidKind。
- [x] AvoidTag（TOML：`[[relations.avoid_tag]]`）。
- [x] RequireNear。
- [x] ClearAnchor：side、soft blocker、基础 must_face。
- [x] NoGeometryCollision：普通实体碰撞有对称检查；交通表面例外有明确策略。
- [x] biome/hazard 基础约束和拒绝原因。
- [x] 冲突 reject 含冲突实体 ID。
- [ ] ClearAnchor/must_face 支持 tag 与完整 target-footprint 走廊几何。
- [ ] 基于精确 shape/rotation 的碰撞，而非只使用 AABB。

## C. Footprint 与地形语义

- [x] 审计 CLI 读取 baked heightfield。
- [x] 高度单位从四分之一米正确换算为米。
- [x] OnGround、water、slope、biome、hazard 采样中心、四角和边中点。
- [ ] 缺失 baked 高度采样必须返回明确错误，而不是回退或推断。
- [ ] 坡度查询应直接按对象的实际 width/depth/rotation 计算，而非复用固定采样半径。
- [ ] pivot、实际对象底部与路床高度有统一定义和测试。

## D. 审计与报告

- [x] `rules audit WORLD_DIR --rules DIR --report PATH` 可执行。
- [x] 报告含 checked / passed / rejected / unruled 覆盖数据。
- [x] checked_items 与 CLI 摘要一致。
- [x] reject 含 item_id、坐标、原因、规则文件和冲突实体 ID。
- [x] 水体、biome、hazard、坡度、越界统计字段分离。
- [x] 规则目录失败不会生成伪造的全零 clean report。
- [ ] 多 seed 的 JSON 报告有固定基线和阈值回归。

## E. 规则主导放置

- [x] `place_one()` 支持确定性候选、0/90 度旋转和 `fallback = move`。
- [x] `place_all()` 按 phase -> priority -> descriptor id -> request index 稳定排序。
- [x] 所有失败候选会返回诊断记录。
- [ ] `generate_entities_with_profile()` 已实际调用 `place_all()`。
- [ ] `place_all()` 与 worldgen OccupancyGrid/精确空间索引统一。
- [ ] 放置提交具有 reservation / rollback / terrain-carve 一致性。
- [ ] 规则模式下道路、建筑、功能设施、植被均由 descriptor 和 PlacementRequest 驱动。

## F. 最终判定

- **当前状态**：**规则模块与审计可验收；完整规则引擎 MVP 不可验收。**
- **阻塞项**：E 节中所有未完成项，特别是 `generate_entities_with_profile()` 接入 `place_all()` 与统一提交事务。
