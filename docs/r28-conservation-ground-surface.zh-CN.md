# r32：自然保育地表、EcologyPlant 与净空合同

> 日期：2026-09-06。权威仓库：`G:\difyllmwiki\GlyphWeave`；未修改 `F:\modenmapbox`。
> 生成修订：`worldgen-2026-09-07-r32-ecology-plant-clearance`；r30 增补 ecology 摘要合同，r31 增补 EcologyPlant 计划，r32 增补水体/硬质实体净空审计。
> 状态：已实现自然用途的可审计地表网格、地表分类和第一版 Rust 植物计划；完整生态群落、真实侵蚀和路线图仍未完成。

## 1. 背景与目标

r24 已保护自然区的高度不被道路、建筑填方改写，r25 建立用途×物理表面账本，r27 完成设施优先和住宅最低供给。但保护高度不等于自然表面已实现，HTML 绿色材质也不能作为 Rust 生成证据。

本轮将 `ConservationCandidate` 变成真实 `SurfacePatch`：按局部坡度和低位信号分类为 ground、forest_floor、wetland；每类仍由固定 16m 网格内的多个四边形/三角形组成，绑定 `sourceCellId`、`zoneKind` 和面积。r30 另发布 ecology 摘要，记录分类 patch 数、面积和来源单元。它仍不声称森林、湿地、物种群落或侵蚀模拟已完成。r31/r32 的植物是来源绑定、经水体/硬质实体净空检查的代理实体计划，不是物种生态模型。

## 2. 生成顺序

```text
自然高度场 / 水体 / 语义栅格
  → zoning、道路、设施、开发归属
  → final engineered heightfield
  → 庭院与农业 surface
  → conservation ground surface
  → land-use ledger 与独立审计
```

nature 分区仍受 r24 保留用途保护。保育 patch 只覆盖其来源单元，不创建道路、建筑或工程填方。

## 3. 代码变化

### Rust

`bevy/crates/core/src/settlement/landcover.rs`

- 新增 `Material::ConservationGround`、`Material::ConservationForestFloor`、`Material::ConservationWetland`，以及 `SurfaceRealization.ecology` 摘要和 `EcologyPlant` 计划。
- 新增 `SurfaceAudit.conservationGroundAreaM2`。
- `SurfaceRealization.ecology.plants` 发布每个自然 patch 的 Rust 植物计划：ID、来源 patch、kind、位置、高度和宽度。
- 对每个 `ConservationCandidate` 按 16m 网格生成 terrain-conforming patch，并依据局部地形分类。
- 保留原 `realizedAreaM2` 的庭院/农业兼容语义，避免破坏旧合同。
- 水体角点相交的单元不生成保育 patch。

`bevy/crates/core/src/settlement/landuse_ledger.rs`

- 新增 `PhysicalCover.conservationGroundM2`。
- nature 用途真实实现面积纳入 ledger。
- 全图面积守恒仍区分 realized、unresolved 和 transport 子集。

`adapters/html/app.js`

- 新增 `conservation_ground` 材质颜色。
- HTML 只消费 Rust 发布的 patch 和 ecology plants，不按用途自行制造面积或植物位置。每个计划植物在 Three.js 中必须恰好有一个 `ecology-plant:<id>` 根节点；根节点携带来源 patch ID。

### 生成和审计合同

生成后立即调用 `landcover::verify`；CLI 从 plan、final terrain 和实体独立重建。检查 patch ID、来源、用途、边界、三角形朝向、有限值、声明面积和高度 hash。保存的 `landcover-audit.json`、`landcover-surfaces.json` 或 ledger 不能伪造通过。

新增 `scripts/check-preview-conservation.mjs`，实际逐个检查保育网格是否存在、材质是否正确、顶点数量是否正确和页面是否报错。

新增 ecology 植物合同：forest_floor→tree，wetland→reed，ground→bush；每个计划植物恰好来自一个 source patch，HTML 只使用 Rust 的位置。r32 还拒绝植物进入水体或道路/建筑/铁路/运河等硬质 footprint。

## 4. 兼容性问题及修复

首版四角采样直接请求 `x+1/z+1`，在完整 raster 的最后一个边界样本越界，导致所有旧 bake 失败。现改为 `height_clamped` 处理表面边缘；它只处理合法边缘采样，不填补内部缺失 chunk。

首版把保育面积加入旧 `realizedAreaM2`，破坏了 208m² 庭院和农业旧合同。现改为独立 `conservationGroundAreaM2`，并只在用途 ledger 中加入自然实现。

新增单元测试证明：保育网格确实绑定自然来源；16m 细分后总面积守恒且每个 patch 仍绑定原来源；篡改 source 或 material 会被独立重建拒绝。生态植物的来源 patch、ID、kind、位置、高度和宽度随 SurfaceRealization 独立比较，浮点位置使用明确容差。

## 5. 权威实测

输出：`bevy/work/ecology-r32-verified-final`。
证据：`bevy/work/ecology-r32-evidence.json`。

| 指标 | 结果 |
|---|---:|
| nature 规划面积 | 49,152 m² |
| conservation patch | 192（forest_floor 160、wetland 17、ground 15） |
| 三类 conservation 地表实现 | 49,152 m²
| Rust EcologyPlant 计划 | 192 株（tree 160、reed 17、bush 15）
| EcologyPlant 水体/硬质实体净空审计 | pass |
| nature 未实现 | 0 m² |
| 农村实际实现 | 39,104 m² |
| 农村未实现 | 157,504 m² |
| 设施 / 住宅 | 4/4、18/18 |
| 道路专项 | pass |
| 保留地专项 | pass |
| 对象规则 | 55/55 pass |
| 完整 gate | **fail** |

nature 分区实际面积为 18.75%，profile 目标为 15%，差额仍在规划容差内；不通过删除自然地表来“修比例”。完整 gate 仍因农村和城市剩余土地/地表未实现而失败。

## 6. HTML 查看

预览端口：`8851`。

- 页面：`http://127.0.0.1:8851/preview/?view=services`
- 截图：`bevy/work/ecology-r32-verified-final/ecology.png`
- 浏览器验证：`bevy/work/ecology-r32-preview.log`

已实际查看截图：保育地形网格、道路、住宅、学校、农田均可见。当前草过密、地面偏白，保育区现在有地表分类（forest floor / wetland / ground），但还没有对应的森林/灌木群落，设施仍为简化体量；这不是最终视觉验收。

## 6.1 r30 ecology 摘要合同

r30 在 `SurfaceRealization.ecology` 发布 `terrain-semantic-surface-classes-v1`：每种自然地表分类记录 patch 数、面积和来源单元，`totalAreaM2` 必须等于分类面积之和。当前权威输出中的分类为 forest_floor 40960m²（160 patches）、wetland 4352m²（17 patches）、ground 3840m²（15 patches），合计 49152m²。HTML ownership 诊断显示这一 Rust 摘要，不自行推导面积。

r31 另外发布 192 个 `EcologyPlant`；r32 对每个计划植物执行来源 patch、水体和硬质实体净空检查：forest_floor 生成 tree，wetland 生成 reed，ground 生成 bush；每个自然 patch 生成一个确定性植物计划。HTML 只选择轻量 primitive 表现，不更改 Rust 坐标。位置和尺寸可审计，但植物数量是当前 16m patch 粒度下的代理密度；r32 的净空是 footprint 级别，不是冠层/根系的完整生态碰撞模型。

这一分类仍是地表语义近似：局部坡度和低位信号不能证明森林群落或湿地水文。

## 7. 测试

最终 Rust workspace：Core 312、App 44、CLI 单元 6、真实集成 17，合计 **379 项通过**；最终日志：`bevy/work/ecology-r32-tests.log`。

前端 Vitest 170 项、Node 高度/住宅/设施合同 27 项通过；lint、typecheck、doc-tree 退出 0，lint 仍有既有 unused/self-assign 警告。

## 8. 仍未完成

1. 三种 Conservation*、r30 摘要和 r31 植物计划仍不是完整森林、林缘、草地、湿地、泥滩或自然公园生态系统。
2. 已有第一版 Rust EcologyPlant 计划和净空审计，但树木/灌木密度、群落层级、资产合同、成长/演替和生态容量仍未完成。
3. sampled drainage 不是完整水量、洪泛、沉积或湿地演替模型。
4. 现有自然高度算法仍标为 `domain-approximation`，不是质量守恒真实侵蚀。
5. 农村仍有 157,504m²、城市仍有 9,048m² 未实现。
6. 未完成 6×10km 的耗时、峰值内存、空间索引和跨 chunk 性能证明。

## 9. 来源与许可证

本轮为 GlyphWeave 自己的 surface、ledger、CLI 和 HTML 集成；未复制 Mapgen4、symbios-ground 或其他外部实现，未新增依赖和资产。

自然地貌、排水和侵蚀相关代码仍必须按 `domain-approximation` 理解。后续引入外部算法必须固定版本，核对 LICENSE、NOTICE、依赖和资产来源，并更新 `THIRD_PARTY_NOTICES.md`。
