# 2026-09-05 Rust Core 重构记录：权威 baked audit 与空间查询

## 背景

GlyphWeave 的 `bake_world` 与 CLI `rules audit` 曾有两套语义：bake 阶段会把实体接地到最终 heightfield，而 CLI 补入 authored landmark 时可能直接使用 manifest 的旧 `world_y`。此外，规则层的 `NotInWater` 主要使用 9 个探针点，无法证明一个完整水体没有被 footprint 包住。旧项目的 `docs/20260904-check-37.md` 已记录这两类风险。

## 本轮修改

### 1. 单一 baked terrain 数据源

新增 `bevy/crates/core/src/baked_terrain.rs` 的 `BakedTerrain`。它按 512m streaming chunk 装载 quarter-meter heightfield，并严格拒绝：缺失 chunk、重复 chunk、错位/越界 chunk、尺寸不一致、截断 payload 和 scene 不匹配。`bake_world` 和 CLI audit 都使用它，不再在 CLI 中维护一份全局 `BTreeMap` 高度副本。

新增 `worldgen::audit_baked_scene` 作为 baked audit 的共同入口：它负责完整性检查、authored landmark 补入、最终 footprint 接地和 `audit_scene` 调用。CLI 只负责读取文件和组装 scene 数据。

### 2. 统一接地语义

`rebase_entities_to_baked_surface` 提升为 Core API。bake 阶段和 CLI 补入 authored landmark 都先使用最终 baked footprint 高度，再进行规则审计。持久化的实体不会被 audit 自动修复；回归测试会把一个实体的 `worldY` 改成极低值，确认 audit 返回失败而不是偷偷抬高它。

接地同时考虑旋转 footprint，以及 center pivot、bottom offset 和 roadbed offset 的语义。

### 3. 统一水体 footprint 查询

新增 `bevy/crates/core/src/worldgen/water_query.rs`：

- 湖泊使用旋转矩形与椭圆水体的边界/内部相交检测；
- 河流使用有限 world-Z 区间和有界误差递归，检查弯曲河道的宽度变化；
- `NotInWater` 增加 `water_intersects` 完整 footprint 回调；
- placement、baked audit、authored landmark fail-closed 检查均共享 Core 的 `WaterGeometry` 查询；
- CLI 旧的展示统计逻辑已改为调用该 Core API。

### 4. 输入和报告门禁

Manifest 现在拒绝空场景、重复 landmark ID、非有限旋转角度和坐标加尺寸溢出。CLI audit 先校验 manifest，再校验 scene index 的尺寸和原点与 manifest 一致。任何 rules rejected item 都会导致非零退出码，避免只生成一份“看起来正常”的失败报告。

### 5. 回归测试

新增测试覆盖：

- 多 seed bake / CLI audit parity；
- 缺块、重复块、截断高度文件；
- scene index mismatch；
- 重复实体和持久化实体下沉；
- 内部地形峰值藏在旧 9 点探针之间；
- 窄河、有限河端点、旋转 footprint、湖泊内部包围。

## 验证结果

2026-09-05 执行：

```text
cargo fmt --manifest-path bevy/Cargo.toml --all
cargo test --manifest-path bevy/Cargo.toml -p glyphweave-core -p glyphweave-cli
cargo check --manifest-path bevy/Cargo.toml --workspace
```

结果：

```text
core + cli tests: 208 passed, 0 failed
workspace cargo check: passed
```

对重新生成的 `bevy/work/shenzhen-bay-clearance-20260905` 执行 CLI audit：

```text
checked=14 passed=14 rejected=0
floating=0 submerged=0 collisions=0
```

原始 `shenzhen-bay-minimal` fixture 的 fence 仍然会被正确拒绝，因为它的 footprint 确实与 river 几何相交；`examples/shenzhen-bay-clearance.manifest.json` 是明确把该 fence 移到岸上的修正版，而不是放宽规则。

## 尚未宣称完成

- 当前项目已经统一了 Core / bake / CLI 的主要空间检查，但 scale-audit 的 authored landmark 专用 asset contract 仍需独立收敛；
- 规则 descriptor 覆盖仍会报告特殊 water kind `river` 为 `unruled`，这属于覆盖率提示，不会被伪装为普通建筑规则；
- 已在 6000m × 10000m 极限场景完成 release 烘焙和 CLI audit；后续仍需更多真实 manifest、多 seed 的性能回归；
- HTML/Godot 适配器仍只能消费 baked 数据，最终视觉质量不等于规则和生成质量。


极限场景实测（2026-09-05）：release generate-procedural-world 成功生成 6000m × 10000m，240 个 512m streaming chunks，2918 个实体；耗时约 3 分 11 秒，产物约 436 MB。该结果证明分块导出可运行，但尚未代表最终生产性能目标。


## 2026-09-05 第五次修正：最终地形工程后的二次选址

极限场景测试发现，规则 placement 在自然地形上通过的停车场、路灯和水井，在道路/建筑 pad 应用后可能出现最终坡度超限。此前如果直接把这类对象交给最终 audit，严格模式会失败。新增 `bevy/crates/core/src/worldgen/engineered_placement.rs`：

- 在正式 heightfield 烘焙前，以最终道路/建筑 carve 重新审计；
- 只对 `generated.*` 的普通细节做有限、确定性的邻域搜索；
- authored landmark、道路、桥、铁路、运河等固定对象不被偷偷移动；
- 候选迁移会重新采样最终地形、重新计算 world_y、更新 anchors/bounds；
- 经过有限轮次后仍无合法位置就明确失败，不删除对象、不放宽 descriptor；
- 输出 `engineering-placement.json`，记录每次迁移的原因和前后坐标。

6000m × 10000m strict release 实测结果：

```text
240 chunks
2700 entities
rules-audit: checked=2700 passed=2700 rejected=0
CLI audit: checked=2700 passed=2700 rejected=0
```

实际生成耗时约 3 分钟，产物约 422 MB。


极限 strict world-quality-gate 复核：world-quality-gate 返回 pass；rules checked=2700、passed=2700、rejected=0，floating/submerged/collision/disconnected 全为 0。仅有 land-use rural share 的软偏好 warning，不影响硬约束门禁。


## 2026-09-05 第六次修正：让 land-use profile 真正影响大场景城市带

极限场景质量报告虽然通过硬门禁，但 `urbanRatio=0.059`、`ruralRatio=0.286`，说明固定的 `0.72/0.50/0.28` region 阈值把大尺寸城市压缩成了少量中心点。此次修改让 `RegionType::classify` 根据 profile 的 urban target 动态调整 UrbanCore/Urban/Suburban 阈值：城市目标越高，城市带越宽，但仍保留 Rural/Forest/Mountain 过渡。新增大场景 region 分布回归测试，禁止 profile-aware generator 退化为 rural-only。


配置 3000m × 5000m、`urbanCoreRatio=0.25` 的回归生成结果：urbanRatio=0.100、ruralRatio=0.135、natureRatio=0.011；quality report 不再报告 rural share 越界。大场景动态阈值现在会扩大城市/郊区带，而不是只在地图中心生成很小的城市岛。


最终 r5 极限回归（2026-09-05）：6000m × 10000m strict 生成 2721 个实体，baked rules audit 2721/2721 通过；world-quality-gate 返回 pass，failures=[]、warnings=[]，floating/submerged/collision/slope/disconnected 全为 0。动态 land-use 阈值修改没有破坏二次选址或最终审计。


## 当前判断（2026-09-05）

Rust Core 的生成、terrain engineering、baked terrain、rules audit、CLI audit 和 quality gate 已形成一条可复核闭环。仍保留的工作属于视觉适配、更多真实 manifest 回归和性能优化，不再是生成结果绕过核心规则的问题。


## 2026-09-05 第七次修正：性能、比例和 authored contract

### 性能

`scale-audit` 现在每个 scene 只加载一次 `BakedTerrain`，边界连续性和实体接地都从内存高度场读取，不再对每个边界像素或实体重复打开 chunk 文件。bake 的 chunk hash 改为增量写入 BLAKE3，避免为每个 512m chunk 创建额外的四 payload 拼接缓冲。

### 土地利用

`RegionType` 阈值按 `urbanCoreRatio + suburbanRatio` 动态调整；quality report 继续输出按 footprint 面积计算的 urban/rural/nature 实际比例。大场景城市 profile 不再被固定阈值压缩成中心小岛。

### Authored asset contract

scale-audit 现在对 Manifest authored landmark 优先查找 `assetContracts["asset:<assetId>"]`。如果 authored asset 没有精确 contract，会记录失败并显示需要补充的 asset key，而不是静默复用 generic kind contract。两个 Shenzhen Bay 示例 manifest 已为 authored assets 增加 asset-keyed contracts。报告新增 `authoredCount`、`authoredAssetContractCount` 和 `authoredKindFallbackCount`。
