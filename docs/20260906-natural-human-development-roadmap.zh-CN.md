# GlyphWeave 自然地形与人文环境下一阶段开发文档

> 版本：v1.0
> 日期：2026-09-06
> 状态：开发前设计与重构边界文档；本文件本轮不要求实现代码。
> 权威工程：`G:\difyllmwiki\GlyphWeave`
> 目标范围：Rust-first，Agent-assisted，最大 6 km × 10 km 的现代社会地图。


> **实现状态校正（2026-09-06）**：第 20–52 节是多轮实验记录，其中部分声称已完成的几何保证、实体来源和 gate 状态与实际代码不符。当前权威实现与验收说明见 `20260906-planning-partition-v2-implementation.zh-CN.md`。本轮发现并复现了宏区 Parcel 重叠；现已修复分区、稳定来源 ID 和 CLI 独立验证。整体目标仍未完成。

> **r16 阶段更新（2026-09-06）**：低地自然 raster→连通道路→临路住宅与朝向→独立审计已接通；512/1000 米专项审计通过，但旧地块未实现、完整 gate 仍失败。权威实测、静态相机修复及下一步缺口见 `20260906-lowland-road-network-implementation.zh-CN.md`。不得将本轮称为地形/人文路线图全部完成。

> **r18 当前实现（2026-09-06）**：road-first 场景已用 DevelopmentPlan 替代矛盾的旧 ParcelPlan，建筑/临路地块/道路父关系、跨 Zone 面积、道路并集及剩余土地归属均独立复算；未规划地表和仅预留庭院不算完成。最新证据、迁移边界及下一步 LandcoverPlan 见 `20260906-road-first-development-ownership.zh-CN.md`。整体 gate 仍 fail，未完成自然地形/人文总目标。

> **r19 接续（2026-09-06）**：LandcoverPlan 已覆盖地块外剩余地和预留庭院，记录农业/保护/未来开发候选；全部候选仍无实际表面 realization。当前验收见 `20260906-landcover-candidate-plan.zh-CN.md`，不可把候选 schema 当作地表生成已完成。

> **r21 当前进展（2026-09-06）**：庭院已实际生成草地 mesh 与可复算铺装交集；入口延伸到真实外墙，BuildingPlan 住宅 renderer 不再虚构院落。512m 实现 208m²，1km 实现 1817/1872m²，55m² 因庭院斜向坡度拒绝。最终验收见 `20260906-yard-surface-realization.zh-CN.md`；农业/自然保护地、全地图生成与视觉目标仍未完成，整体 gate 仍 fail。

> **r22 当前进展（2026-09-06）**：农业候选已进入实际田面/地头/4m 农用通道网格，独立检查来源、道路、全宽地形和保留地，旧 farm proxy 不再算实际生成。新 plains 用例实现 4 块，原起伏地形用例仍 0 块；量化/坡度和道路覆盖缺口未隐藏。最新证据与下一步见 `20260906-agriculture-surface-realization.zh-CN.md`。整体 gate 仍 fail，完整目标未完成。

> **r23 当前实现**：可选 GWHM 毫米高度已贯穿自然/最终 bake、CLI 和 HTML；明确共享 terrainDomain 与 origin。平缓用例田块 4→26，但路网/住宅也改变，保护用途冲突及未规划住宅仍未解决；整体 gate fail。权威证据与剩余旧预检查/Godot/性能边界见 `r23-height-precision-and-terrain-domain.zh-CN.md`，不可称为完整路线图实现。

> **r24 当前实现（2026-09-06）**：用途约束前置到道路/住宅候选，保留区高度逐像素保护；TerrainContext 统一关键预检与最终量化；旧模板未规划建筑/运河等改为延后请求而非直接实体。三个用例保护/道路专项通过、越界开发及未规划建筑为 0，但完整 gate 仍 fail。真实结果、失败修复、来源与后续缺口见 `r24-zoned-planning-and-terrain-protection.zh-CN.md`。公共设施延后不等于实现。

> **r25 当前实现**：新增用途×物理表面账本，修复 road-first 报告按旧实体代理把农村记为 0 的错误；quality-report v2 保留全图分母与真实未实现缺口，新增严格账本门禁及 HTML 只读表格。平原农村用途实际 36176m²、全图未实现 218992m²；所有真实实体/地形 payload 与 r24 相同，不代表新增覆盖。完整 gate 仍 fail。接口迁移、实测与后续缺口见 `r25-purpose-cover-ledger.zh-CN.md`。

> **r26 当前实现**：显式 settlementProgram 已接入 Rust 可变尺寸设施选址/建筑/入口与独立审计；平原实际新增 2 家便利店和 1 座教堂，原 18 栋住宅/29 段道路保留，40×30m 学校仍未找到可行候选。修复 HTML 重复便利店和单次断连杀死预览进程的问题。完整 gate 仍 fail，不等于公共服务/自然路线图完成。合同、真实取舍与验收见 `r26-explicit-service-program.zh-CN.md`。

> **r27 当前实现**：保留 v1 兼容行为，新增 settlementProgram v2 的设施预留与住宅最低供给补足。权威平原示例 4/4 设施、18/18 住宅及指定道路入口通过，包含原尺寸学校；16 个原住宅候选保留、2 个替代候选实际生成，29 段道路不变。完整 gate 仍 fail；中间仅 16 栋住宅的试验不算完成。版本、验收、反例和局限见 `r27-joint-frontage-and-housing-minimum.zh-CN.md`。

> **r28 当前实现**：nature ConservationCandidate 已生成 terrain-conforming Rust SurfacePatch，并进入 nature 用途账本；平原 49152m² 自然用途已有真实保育地表网格。旧庭院/农业 realizedArea 语义保持兼容。生态群落、森林/湿地、水文真实性、农村剩余覆盖和完整 gate 仍未完成。详见 `r28-conservation-ground-surface.zh-CN.md` 的 r29 更新记录，以及 `bevy/work/ecology-r29-final-evidence.json`。

> **r29 当前实现（2026-09-06）**：nature 保育地表按 16m 网格生成，并按坡度/低位信号分类为 ground、forest floor、wetland；平原实际 49,152m² 自然地表保持面积守恒，HTML 和独立审计通过。分类不是完整生态群落或真实湿地/侵蚀模拟，完整 gate 仍 fail。详见 r28-conservation-ground-surface.zh-CN.md 的更新记录和 ecology-r29-evidence.json。

> **r30 当前实现（2026-09-07）**：保育地表按 16m 网格分类为 ground、forest floor、wetland，并发布可审计 ecology 摘要；平原自然用途 49152m² 的 patch 面积守恒，HTML/CLI 验证通过。分类不等于生态群落或真实水文，完整 gate 仍 fail。证据见 `bevy/work/ecology-r30-evidence.json`。

> **r31 当前实现（2026-09-07）**：自然 patch 已发布 192 个来源绑定 EcologyPlant 计划（tree/reed/bush），r32 增加水体和硬质实体净空审计，HTML 只渲染 Rust 位置；独立测试和浏览器验证通过。植物数量仍是 patch 粒度代理，不是物种群落，完整 gate 仍 fail。详见 `r28-conservation-ground-surface.zh-CN.md` 的 r32 更新。

## 0. 文档目的

本文件不是重新描述一个理想中的地图生成器，而是对当前 GlyphWeave 的真实状态做一次“来源—能力—缺口—优先级”盘点，并给出下一阶段必须完成的重构顺序。

本文件重点回答：

1. 当前 GlyphWeave 每个算法来自哪里；
2. 哪些思想来自 Mapgen4；
3. 哪些思想来自 symbios-ground；
4. 哪些是 GlyphWeave 自己实现的；
5. 哪些内容只是近似，不能称为真实侵蚀或真实 GIS；
6. 哪些代码和合同可以保留；
7. 哪些代码应该拆分；
8. 哪些外部开源项目适合引入、适合只参考、或不应直接复制；
9. 项目最终开源时的许可证策略；
10. 如何形成可审计的 Rust-first 自然地形核心；
11. 自然地形和人文环境如何在同一条生成流水线中协作。

## 1. 总体判断

### 1.1 当前不是“没有地形系统”，而是多个层次被混在了一起

当前代码已经有相当多可复用的工程基础：

- 以 Rust Core 为生成和检查中心；
- 分块 heightfield；
- 统一的最终地形查询；
- terrain semantic sidecar；
- 水体 footprint 和水面查询；
- terrain carve；
- 道路、建筑、植被和障碍的规则检查；
- ReservationLayer；
- AccessGraph 和 AccessSurface；
- authored asset contract；
- scale-audit、rules audit、world-quality-gate；
- HTML renderer 消费 Rust 生成的地形数据。

问题主要在于自然层仍然以一个巨大的 `worldgen.rs` 为中心，里面同时包含：

- 噪声高度场；
- 山脊和山谷近似；
- 固定水位；
- 河流/水体判定；
- 简化侵蚀雕刻；
- 城市区域 mask；
- 建筑/道路地基 carve；
- 实体生成；
- 统计和输出。

这使得“自然地形”和“人类改变地形”之间的边界不够清楚，也让算法来源难以追踪。

### 1.2 下一阶段的核心不是继续增加随机噪声

必须将系统拆成两个相互协作、但数据上分离的核心：

```text
NaturalTerrainCore
  ├─ GeologicalRelief
  ├─ ClimateField
  ├─ DrainageGraph
  ├─ ErosionSediment
  ├─ LandformSemantics
  ├─ EcologyHazards
  └─ NaturalTerrainAudit

HumanEnvironmentCore
  ├─ SettlementPlanning
  ├─ InfrastructureGraph
  ├─ ParcelAndLandUse
  ├─ EngineeredTerrain
  ├─ BuildingsAndEntrances
  ├─ ExistingFeatures
  ├─ PublicSpaceAndVegetation
  └─ HumanEnvironmentAudit
```

最终输出仍然由 Rust Core 统一编排，但自然高度场与工程差分必须可分别保存、分别审计、分别回放。

## 2. 当前实现的来源地图

### 2.1 来源分类原则

后续所有核心算法都必须在代码注释和文档中标注来源类别：

- `external-adaptation`：基于外部项目算法或数据结构改写；
- `external-inspiration`：只借鉴思想，没有复制实现；
- `glyphweave-original`：当前项目自行设计；
- `domain-approximation`：为了实时性或工程边界做的近似；
- `integration-contract`：GlyphWeave 的数据、审计和输出合同。

不能只写“参考了某项目”，因为这无法回答许可证、算法准确性和维护责任问题。

### 2.2 当前代码来源对照表

|当前能力|主要位置|判断|来源/说明|
|---|---|---|---|
|`value_noise2d`|`bevy/crates/core/src/worldgen.rs`|项目自写基础实现|连续格点 value noise；不是直接复制外部库|
|`fbm2d`|同上|项目自写组合|多尺度 value noise 组合；可保留，但只能做残差，不应承担地质骨架|
|山脊/山谷场|同上|混合实现|受 Mapgen4 distance-field/ridge 思路启发，当前实现不是 Mapgen4 原码的完整移植|
|固定水位与海岸/水体判定|同上|项目集成逻辑|结合世界合同和场景参数；不是完整水文模拟|
|`erosion_carve`|同上|项目自写近似|通过 domain warp、噪声和 mask 模拟谷地切割；不是粒子侵蚀或质量守恒侵蚀|
|道路/建筑地基 flatten|同上|强烈受 symbios-ground 思路启发|当前是 GlyphWeave 自己的实体 footprint 与 heightfield carve 合同|
|分块 baked heightfield|`baked_terrain.rs`、`worldgen.rs`|GlyphWeave 原创工程合同|为 512m chunk、严格加载、CLI audit 服务|
|terrain semantic sidecar|`worldgen.rs`、baked 输出|GlyphWeave 原创集成合同|坡度、曲率、湿润度、扰动等同格栅数据|
|水体连续 footprint|`worldgen/access.rs` 等|GlyphWeave 原创工程实现|用于道路、入口、建筑和可达性检查|
|AccessGraph v2|`worldgen/access.rs`|GlyphWeave 原创工程实现|有界路径、障碍、坡度和最终 heightfield 联合检查|
|AccessSurface|`worldgen/access_surface.rs`|GlyphWeave 原创工程实现|持久化 Rust 生成的铺装面和三角形|
|ReservationLayer|`worldgen.rs` 和相关规划逻辑|GlyphWeave 原创架构|提前保留道路、铁路、水体缓冲和工程走廊|
|authored asset contract|rules/scale-audit 相关代码|GlyphWeave 原创合同|将外部/用户资产的尺度、footprint、入口等纳入审计|
|城市形态和 land-use profile|`intent.rs`、`worldgen.rs`、docs|GlyphWeave 原创领域层|吸收真实城市观察，但不是任何单一城市复制|
|HTML terrain renderer|`adapters/html/render/terrain.js` 等|GlyphWeave 原创适配|渲染器消费 Rust authority，不应自行推导地形逻辑|

### 2.3 直接引用 symbios-ground 的地方

当前代码注释明确提到 `symbios-ground-lab` 的 terrain carving 思路。当前实际继承的主要是：

- 连续高度场上的世界坐标采样；
- 结构 footprint 对地形进行平整或压平；
- 建筑/道路先获得可建造地面，再生成实体；
- 坡地建筑不能只依靠悬空模型，而需要地基/地形工程关系。

但是当前 GlyphWeave 并没有把 `symbios-ground` 作为完整依赖集成，也没有复制其完整 erosion API。当前工程中的 carve 是针对 GlyphWeave 的道路、建筑、入口和审计合同重新实现的。

外部 `symbios-ground` 本身提供 Diamond-Square、FBM、Voronoi、液压侵蚀、热侵蚀、splat mapping、双线性采样和 tiled heightmap；其仓库声明使用 MIT 许可证。它适合作为可选实验依赖和算法对照，不应被误称为已经集成到 GlyphWeave。citeturn0search1

### 2.4 直接参考 Mapgen4 的地方

当前已存在明确历史提交和文档：

```text
6f5283d feat(world): ridge-noise terrain skeleton (mapgen4 hydrology phase A)
ad9cf08 feat(world): hydrology rivers and climate biomes (phase B+C)
docs/mapgen4-hydrology-plan.zh-CN.md
```

当前吸收的 Mapgen4 思想主要是：

- 从峰点/距离场组织宏观地形，而不是只做独立 cell 噪声；
- 用山地、丘陵、海岸距离等低频结构控制高度；
- 把河流、水文、降水、蒸发、风和 biome 看作相关派生场；
- 采用稳定 seed 和世界坐标，使分块生成连续；
- 使用河网和自然区域来提高地图整体可读性。

当前并没有完整采用 Mapgen4 的 dual mesh/Voronoi 网格、完整河流算法和其全部渲染管线。Mapgen4 官方明确说明其设计目标是实时重生成和视觉上好看，而不是生成真实地形，因此其代码不能直接被当作工程 DEM 或真实地质模型。Mapgen4 主仓库使用 Apache-2.0，并且其 README 还列明 Delaunator 为 ISC、Poisson sampling 和 simplex-noise 为 MIT、flatqueue 为 ISC。citeturn0search2turn0search3

### 2.5 当前项目自己的核心

以下部分不是外部项目可以替代的，而是 GlyphWeave 的长期差异化能力：

1. 最大 6 km × 10 km 的地图尺寸和 512m 分块；
2. baked terrain 严格契约和损坏 payload 拒绝；
3. 自然地形、最终工程地形和渲染读取同一份权威结果；
4. 道路/建筑/树木/岩石/水体的统一空间规则；
5. 入口可达性图而不是只检查碰撞；
6. AccessSurface 和真实铺装几何输出；
7. land-use、urban ratio、parcel、建筑基底面积的审计闭环；
8. authored asset contract 和 scale-audit；
9. Agent 只提供意图和候选约束，Rust Core 负责几何与拒绝；
10. 可回放的生成报告、质量门禁和 fresh baked output 验收。

这些不应为了“引入开源算法”而被替换。

## 3. 哪些当前实现只是近似

### 3.1 `erosion_carve` 不是真实侵蚀

当前 `erosion_carve` 的注释本身已经说明它不是粒子模拟。它是一个确定性纯函数，利用 domain-warped noise、主要/次要沟槽和城市 mask，在野外地形中向下切出谷地。

它可以被准确称为：

```text
hydrology-inspired deterministic valley carving
```

或中文：

```text
受水文启发的确定性谷地雕刻近似
```

不能称为：

- 真实液压侵蚀；
- 质量守恒侵蚀；
- GIS DEM 水文侵蚀；
- 物理真实河流冲刷。

它没有完整表达：

- 水量；
- 流速；
- 沉积物容量；
- 沿程侵蚀/沉积质量守恒；
- 岩性和土壤差异；
- 地表径流汇流过程；
- 河口和冲积扇沉积；
- 暴雨/季节性流量；
- 地下水或渗透。

因此它可以暂时保留作为 `L1/L2` 形态残差，但不能作为下一阶段自然地形核心的终点。

### 3.2 当前河流不是完整 DrainageGraph

有水体、河流和 climate 逻辑，但目前不能把它描述为完整的 GIS 流域系统。下一阶段必须有显式：

- upstream/downstream；
- basin id；
- flow accumulation；
- discharge class；
- channel width；
- floodplain；
- delta/fan/terrace relation；
- bridge/culvert requirement。

### 3.3 当前山地不是地质构造模拟

山峰距离场和多尺度噪声可以产生具有宏观可读性的山地，但没有板块、断层、岩性、抬升和长期剥蚀的因果模型。因此应称为：

```text
procedural macro-relief
```

而不是地质真实模拟。

### 3.4 当前地基 carve 不是施工模拟

carve 能保证地形与实体接触、道路和建筑有平面基础，但没有表达：

- 土方来源和去向；
- 施工机械或阶段；
- 边坡稳定性；
- 挡墙结构；
- 排水和邻地影响；
- 地基类型；
- 桥台和涵洞的工程关系。

因此它应升级为 `EngineeredTerrainOperation`，并保留原始自然场差分。

## 4. 可以保留的代码和合同

### 4.1 必须保留

1. `BakedTerrain` 的严格覆盖检查、重复 chunk 拒绝和缺失拒绝；
2. 最终 heightfield 作为规则审计和 renderer 的唯一地形权威；
3. 512m chunk 和世界坐标连续采样；
4. terrain semantic sidecar 的同格栅布局；
5. 水体 footprint 查询；
6. AccessGraph v2、AccessSurface 和入口路径折线；
7. ReservationLayer；
8. authored asset contract 和 scale-audit；
9. fresh output、CLI audit、quality gate 验收方式；
10. Rust Core 拒绝非法几何而不是由 Agent 或 renderer 偷修。

这些内容与“自然地形算法来自哪里”无关，是整个工程可靠性的基础。

### 4.2 可以暂时保留，但必须改名或隔离

- `fbm2d`：保留为残差细节，不作为地质主地形；
- `erosion_carve`：保留为临时 valley-carving fallback，并在报告中标注 approximation；
- 当前 city mask：保留为过渡兼容层，最终由 SettlementPlan/BuildabilityScore 替代；
- 当前道路/建筑 flatten：保留功能，但迁移到工程操作层；
- 当前固定水位：保留为世界合同参数，但不能代替水文模型；
- 当前 terrain profile：保留为用户意图入口，内部改为 NaturalWorldIntent。

### 4.3 不应继续扩张的代码

以下方向不应继续在同一函数中加条件：

- 在 `terrain_height_with_geometry_carved` 中继续加入城市、道路、建筑、河流和装饰规则；
- 在 renderer 中推导水体、坡度或可种植性；
- 用一个 `urbanization` mask 同时决定侵蚀、建筑、植被和土地利用；
- 用“没有实体”代替自然地形没有问题；
- 用放宽坡度/碰撞阈值掩盖自然层缺失。

## 5. 必须重构的部分

### P0：自然场与工程场分离

必须新增并持久化两个明确版本：

```text
NaturalTerrainSnapshot
EngineeredTerrainSnapshot
```

`NaturalTerrainSnapshot` 至少包含：

- natural heightfield；
- climate fields；
- drainage graph；
- water bodies；
- landform semantics；
- hazard layer；
- ecology/protection layer；
- source/provenance/version。

`EngineeredTerrainSnapshot` 至少包含：

- ordered operations；
- cut/fill volume；
- retaining/embankment/reclamation/bridge/channel entities；
- affected chunks；
- drainage impact；
- stability result；
- final heightfield hash。

### P0：把自然地形从 `worldgen.rs` 拆成模块

建议目标结构：

```text
bevy/crates/core/src/natural/
  mod.rs
  intent.rs
  relief.rs
  climate.rs
  hydrology.rs
  erosion.rs
  landforms.rs
  ecology.rs
  snapshot.rs
  audit.rs

bevy/crates/core/src/engineering/
  mod.rs
  operation.rs
  cut_fill.rs
  retaining.rs
  waterworks.rs
  bridge.rs
  snapshot.rs
  audit.rs

bevy/crates/core/src/settlement/
  mod.rs
  morphology.rs
  infrastructure.rs
  parcels.rs
  buildings.rs
  public_space.rs
  audit.rs
```

这不是要求马上创建所有文件，而是禁止继续让一个函数承载所有层。

### P0：显式 DrainageGraph

水流和河流必须成为 Rust 数据结构，而不是只从高度查询临时推导。至少包含：

```text
DrainageNode
DrainageEdge
Basin
DischargeClass
Floodplain
SedimentZone
```

### P0：重新命名近似算法

把 `erosion_carve` 及其报告输出明确标记为 approximation，防止后续文档或 Agent 将其宣传成真实侵蚀。

### P1：人文环境的关系图

将建筑位置从“独立采样 + 距离评分”升级为：

```text
SettlementNode
InfrastructureEdge
District
Parcel
Building
PublicSpace
ExistingFeature
```

每个建筑要能回答：

- 它属于哪个聚落/街区/地块；
- 它为什么靠近这条路；
- 它的入口朝向什么公共边；
- 它与河流、坡度、排水和灾害层是什么关系；
- 它是否因为历史锚点、宗教、市政、市场或交通节点而位于此处。

### P1：土地利用比例从结果统计升级为预算闭环

当前 land-use 统计可以发现 urban ratio 偏离，但还不够。下一步应分开审计：

```text
region_area_budget
parcel_area_budget
building_footprint_budget
public_space_budget
infrastructure_area_budget
```

面积预算必须在生成前分配，在生成中消耗，在生成后核对。不能仅统计生成结果后给 warning。

## 6. 可选但有价值的重构

### P2：引入真正的侵蚀/沉积模块

可选路径：

1. 先自己写 Rust 的河网导向侵蚀/沉积，保持可解释和可审计；
2. 在独立 feature 下实验性引入 `symbios-ground` 的液压/热侵蚀；
3. 只把外部算法用于自然层预生成，再转换为 GlyphWeave snapshot；
4. 不把第三方 heightmap 类型暴露到人文生成和审计合同。

推荐顺序是 1 → 2 → 3，而不是直接把第三方 HeightMap 作为全项目核心。

### P2：板块或构造先验

WorldEngine 这类项目可以启发宏观构造、降雨阴影、侵蚀和 biome 的因果顺序，但其输出尺度和目标不同，不应直接替代 GlyphWeave 的局部工程地形。WorldEngine README 描述了板块模拟、雨影、侵蚀、湿度/渗透性和 Holdridge life zones，并使用 MIT 许可证；适合作为参考或离线实验工具。citeturn0search0

### P2：dual mesh / Voronoi

Mapgen4 的 dual mesh 适合宏观河网和自然区域拓扑，但当前建筑和 1m terrain raster 已经使用另一套合同。可将 dual mesh 作为低分辨率自然规划网格，向下采样/投影到 1m baked heightfield；不要用它替换实体碰撞和入口路径数据。

### P2：自然统计 golden fixtures

为多个 seed 保存：

- 高程分位数；
- 坡度分位数；
- 河网长度和等级分布；
- 流域数量；
- 洪泛区面积；
- 山脊/谷地数量；
- 沉积区比例；
- 边界连续性。

## 7. 外部开源代码引入策略

### 7.1 推荐“借鉴/适配”，不推荐整库吞入

|项目|适合借鉴|不适合直接承担|建议|
|---|---|---|---|
|Mapgen4|距离场、河流组织、自然地图交互、dual mesh|建筑工程、真实 DEM、6×10km 施工审计|Apache-2.0；可适配并保留 NOTICE/版权说明|
|symbios-ground|高度图 API、液压/热侵蚀、采样、tile 思路|GlyphWeave 的聚落、工程、入口、asset contract|MIT；适合 feature-gated 试验或抽取明确模块|
|WorldEngine|板块/气候/雨影/生态因果顺序|局部道路、建筑和最终工程地形|MIT；适合离线参考和 fixture 生成|
|FastNoiseLite|噪声库、FBM、OpenSimplex/Perlin/Cellular|水文、侵蚀、聚落布局|MIT；可作为明确依赖，但要保留其许可证文本|
|GIS/DEM 工具|验证真实 DEM、离线对照、流域算法参考|直接作为运行时的全部地图生成器|按各项目许可证逐项核对，不默认可复制|

### 7.2 引入准则

外部代码只有在满足下列条件时才能进入主仓库：

1. 仓库许可证明确，且与 MIT 项目兼容；
2. 能锁定 commit/tag；
3. 能记录版权、许可证和修改说明；
4. 有独立 wrapper，不把外部类型传播到核心合同；
5. 有许可证扫描和构建验证；
6. 有与 GlyphWeave seed、chunk、边界和 audit 的适配测试；
7. 失败时不会让空集合或 fallback 冒充成功。

如果只借鉴算法思想，不复制代码，则在文档中注明 inspiration；如果复制或改写代码，则必须保留对应版权和许可证文本，并在源文件中注明来源和修改范围。

## 8. 许可证策略

### 8.1 当前项目

`bevy/Cargo.toml` 当前声明 MIT。项目最终开源时建议继续保持：

```text
GlyphWeave 自有代码：MIT
```

同时不要把所有第三方代码简单合并进一个“全项目 MIT”声明。应区分：

```text
LICENSE                 项目自有代码许可证
THIRD_PARTY_NOTICES.md  第三方组件、版本、许可证、修改说明
third_party/<name>/     必要的许可证和版权文件
```

### 8.2 Apache-2.0 组件

Apache-2.0 与 MIT 通常可以组合使用，但引入 Apache 代码时必须保留：

- 原始版权声明；
- Apache License 2.0 文本或明确副本；
- NOTICE 文件中的相关内容；
- 对修改过的文件作出修改说明；
- 专利条款和免责声明。

因此 Mapgen4 若只参考思想，不需要复制其源码；若复制或改写，应在 `THIRD_PARTY_NOTICES.md` 中单独记录。Mapgen4 仓库明确标为 Apache-2.0。citeturn0search2turn0search3

### 8.3 MIT 组件

MIT 组件一般允许修改、再发布和商业使用，但必须保留版权和许可文本。symbios-ground 和 WorldEngine 的仓库页面都标明 MIT。citeturn0search1turn0search0

### 8.4 ISC 组件

ISC 许可通常也允许宽松使用，但仍需保留版权和许可条件。Mapgen4 README 列出的 Delaunator 与 flatqueue 属于 ISC，应在依赖树和 notices 中保留记录。citeturn0search2

### 8.5 必须避免的许可证错误

- 不要从许可证不明的博客、视频、截图或闭源工具中复制代码；
- 不要把“GitHub 可见”当成“允许复制”；
- 不要因为一个上游项目是 MIT 就忽略它的依赖许可证；
- 不要把第三方资源模型、纹理和代码混成同一套许可；
- 不要只在 README 写“参考了某项目”，而不保留实际版权文本；
- 不要将生成结果本身错误地宣称为上游项目的原始输出。

## 9. 推荐的 Rust-first 最终架构

### 9.1 Agent 输入

Agent 输出结构化意图，而不是几何：

```text
NaturalWorldIntent
  climate
  relief_archetype
  coast_or_inland
  water_preference
  mountain_preference
  hazard_preference
  naturalness_target
  settlement_intent
  infrastructure_intent
  land_use_budget
  cultural_anchors
```

### 9.2 Rust 自然阶段

```text
Intent normalization
→ seed and world bounds
→ macro relief
→ climate fields
→ drainage graph
→ erosion/sediment approximation or simulation
→ landform semantics
→ ecology/hazard fields
→ NaturalTerrainSnapshot
→ natural audit
```

自然阶段不得读取建筑实体来“让地形更好看”。如果用户要求城市位于平原，应输出偏好/选址评分；不能事后把整座山抹平而不产生工程记录。

### 9.3 Rust 人文阶段

```text
NaturalTerrainSnapshot
→ buildability analysis
→ conservation / hazard / reservation layers
→ settlement seeds
→ infrastructure graph
→ districts and public-space network
→ parcels and land-use budgets
→ buildings and entrances
→ vegetation, rocks, markets and decorations
→ engineered terrain operations
→ final heightfield
→ access graph and audit
```

这里的人文环境不是“把建筑散布在自然地形上”，而是先生成道路、公共空间、地块和工程，再生成建筑和细节。

### 9.4 Agent 的后续作用

Agent 可以：

- 解释报告；
- 发现用户要求与地形约束冲突；
- 提出替代聚落方案；
- 调整 profile 和预算；
- 生成文化锚点的高层描述。

Agent 不可以：

- 直接修改 heightfield；
- 绕过水体/坡度/入口/碰撞审计；
- 删除不符合预期的建筑后声称通过；
- 直接给 renderer 发放置坐标作为最终事实。

## 10. 下一阶段执行顺序

### 第一批：必须完成

1. 完成算法来源注释和 `THIRD_PARTY_NOTICES.md`；
2. 将自然高度场、工程 carve、实体生成从 `worldgen.rs` 中分层；
3. 为自然场和工程场增加独立 snapshot/version/hash；
4. 将 `erosion_carve` 重命名为明确的 approximation；
5. 建立最小 `DrainageGraph`；
6. 把现有水体和洪泛判断接到 DrainageGraph；
7. 保留现有 baked terrain、AccessGraph、scale-audit 作为验收底座；
8. 为自然阶段增加独立 audit，禁止仅靠实体规则 gate 宣称自然地形质量合格。

### 第二批：必须完成但可以在第一批后拆分

1. land-use area/parcel/building/public-space 多级预算闭环；
2. `EngineeredTerrainOperation` 记录 cut/fill/retain/embank/channel/reclaim；
3. settlement/infrastructure/parcel/building 关系图；
4. ExistingFeature 持久化合同；
5. 多 seed 的自然统计 golden fixtures；
6. natural snapshot 到 final snapshot 的差分回放。

### 第三批：可选增强

1. symbios-ground 液压/热侵蚀 feature；
2. Mapgen4 dual mesh 作为低分辨率自然规划网格；
3. WorldEngine 风格构造/气候离线预生成；
4. 更复杂的岩性、地下水和沉积物模型；
5. 真实 GIS DEM 对照测试；
6. 复杂交通流、人口和土地经济模拟。

## 11. 完成定义

下一阶段不能以“预览看起来更像山”作为完成标准。至少需要：

### 自然层

- 同 seed 可复现；
- chunk 边界连续；
- DrainageGraph 无逆流和孤立非法边；
- 水体、洪泛区、河谷和高程关系一致；
- 山脊、谷地、盆地和沉积区有可审计统计；
- 自然输出明确标注 approximation 或 simulation；
- natural snapshot 可以独立加载和审计。

### 人文层

- 聚落选择受到 BuildabilityScore 约束；
- 主路、次路、地块和建筑具有父子关系；
- 公共空间、绿地、市场、学校、教会/寺庙、市政等不再是同权重随机物体；
- 每个建筑都有入口和可达路径；
- 工程行为有操作记录、土方/排水/稳定性状态；
- land-use 预算和实体统计一致；
- authored 与 generated 对象都经过同一套最终 audit。

### 开源和许可证

- `bevy/Cargo.toml` 的许可证声明与根目录说明一致；
- `THIRD_PARTY_NOTICES.md` 与 Cargo/npm 依赖一致；
- 所有复制/改写的外部源码有来源、版本、许可证和修改说明；
- 资源模型、纹理、字体和代码分别有许可证记录；
- CI 能执行许可证扫描或至少对清单做一致性检查。

## 12. 最终决策

当前最合理的路线不是全面替换 GlyphWeave，也不是直接把 symbios-ground 或 Mapgen4 整个搬进来，而是：

```text
保留 GlyphWeave 的工程合同、审计和分块系统
+ 把 Mapgen4 的宏观地形/水文思想放到 NaturalTerrainCore
+ 把 symbios-ground 的侵蚀和连续高度场能力作为可选自然模块
+ 把其 terrain editing 思想改造成 GlyphWeave 的 EngineeredTerrainOperation
+ 用 GlyphWeave 自己的关系图、土地预算和 AccessGraph 完成现代人文环境
```

最重要的认知边界是：

```text
当前 erosion_carve = 谷地形态近似，不是真实侵蚀
当前 terrain carve = 工程化地基近似，不是施工模拟
当前 Mapgen4 参考 = 算法思想和部分结构，不是完整集成
当前 symbios-ground 参考 = 局部思想和可选依赖，不是完整集成
```

只有在这几个边界被写入代码、文档、报告和许可证清单之后，GlyphWeave 才能形成一个可开源、可维护、可解释的 Rust-first 地图生成核心。

## 13. 参考资料

- Red Blob Games Mapgen4：<https://github.com/redblobgames/mapgen4>
- Mapgen4 源码入口与 Apache-2.0 头部：<https://github.com/redblobgames/mapgen4/blob/main/mapgen4.ts>
- TheJanusStream symbios-ground：<https://github.com/TheJanusStream/symbios-ground>
- TheJanusStream bevy_symbios_ground：<https://github.com/TheJanusStream/bevy_symbios_ground>
- WorldEngine：<https://github.com/Gapi34/WorldEngine>
- FastNoiseLite：<https://github.com/Auburn/FastNoiseLite>
- Apache License 2.0：<https://www.apache.org/licenses/LICENSE-2.0>
- MIT License：<https://opensource.org/license/mit>

## 14. 本文件的实施边界

本轮只新增设计文档，不修改生成代码，不引入第三方依赖，不声称自然地形重构已经完成。后续任何代码重构都必须在本文件的 P0/P1 范围内逐项对应，并同步更新实现状态、测试证据和许可证清单。

## 15. 2026-09-06 首批落地状态

本轮开始按 P0 落地，但只完成了“合同和输出边界”的第一步，没有声称水文重构完成：

- 新增 `bevy/crates/core/src/natural.rs`：版本化 `NaturalTerrainSnapshot`、最小 `DrainageGraph`、来源分类和明确的侵蚀近似标签；
- 新增 `bevy/crates/core/src/engineering.rs`：版本化 `EngineeredTerrainSnapshot` 与有序 footprint operation；
- `SceneIndex` 同时记录自然快照和工程快照；
- 每个新 bake 输出 `natural-terrain.json` 和 `engineered-terrain.json`；
- 当前 DrainageGraph 仍是 authored water footprint 的保守合同，节点出口、流量、流域和沉积尚未计算；
- 当前工程操作仍从既有 terrain carve 投影而来，尚未具备土方守恒、挡墙稳定性或排水重算。

本轮验证：`cargo test --manifest-path bevy/Cargo.toml -p glyphweave-core` 通过（228 tests），`cargo check --manifest-path bevy/Cargo.toml --workspace` 通过，`pnpm doc-tree:check` 通过。下一步仍应优先实现真正的自然阶段审计和 DrainageGraph 派生，而不是继续增加噪声或渲染逻辑。

## 16. 2026-09-06 P0 继续落地状态

本轮继续完成了最小可计算水文合同：

- `natural.rs` 新增 32m 采样间距的有界 D8-style surface-flow graph；
- 每个采样节点记录高程和 flow accumulation；
- 边记录 surface flow 或 boundary outflow，并按累积量给出 headwater/tributary/main-channel 分类；
- 生成阶段从完成的 `BakedTerrain` 派生自然快照，而不是从 renderer 或实体猜测；
- 新增 `natural-terrain-audit.json`，检查边引用、上游到下游高程关系和环；
- strict `world-quality-gate` 现在要求每个场景存在并通过自然地形审计；
- 该模型仍明确标注为 `derived-d8-sampled-v1`，不是完整 GIS 水文或真实侵蚀模拟。

本轮验证：workspace tests 229 passed、CLI 集成测试 2 passed、`cargo check --workspace` 通过；512m strict fresh bake 的 scale-audit、rules audit 和 world-quality-gate 通过，自然审计为 `nodeCount=257`、`edgeCount=248`、`invalidEdges=0`、`descendingEdges=0`、`cycles=0`。HTML 预览已通过本地浏览器真实加载并渲染 1m terrain geometry/data。

## 17. 土地利用预算合同首版

本轮将土地利用从“质量报告末端统计”提升为场景合同：

- `LandUseBudget` 记录归一化后的 urban/rural/nature 目标面积；
- 同时记录已生成面积、已分配面积和未分配面积；
- `SceneIndex.landUseBudget` 持久化该合同；
- 每个场景额外输出 `land-use-budget.json`；
- quality report 的场景数据会携带该预算，供 CLI、Agent 和后续规划阶段读取；
- `status=underfilled_or_mismatched` 不会被伪装成达标，当前生成器仍会明确报告未分配面积。

这一步完成了“预算可见、可持久化、可审计”的首版，但尚未完成“按预算自动补足地块”的闭环。下一步必须让 Settlement/Parcel planner 消耗预算，而不是仅输出差额。

## 18. 2026-09-06 事实校正与当前未完成项

本轮发现并修正了一个重要的语义问题：自然水文不能从已经叠加道路、建筑地基和入口 apron 的最终 heightfield 推导，否则工程地形会被错误地当成自然分水岭。生成阶段现在同时构建：

- `natural_samples`：无 terrain carve 的自然高度场，仅用于 `NaturalTerrainSnapshot` 和 DrainageGraph；
- `baked_samples`：包含工程 carve 的最终高度场，用于实体 grounding、AccessGraph、规则审计和 renderer。

因此自然水文与工程地形的边界现在是真实数据边界，而不是只写在 JSON 中的标签。

土地利用方面，本轮确认不能用追加保护区/草场实体把未分配面积伪装成预算完成。当前 `LandUseBudget` 已能持久化目标面积、实际实体覆盖和未分配面积，并在 quality report / world-quality-gate 中报告 `underfilled_or_mismatched`；但 Settlement/Parcel planner 还没有真正按预算生成足够的宏观地块。因此该项仍是未完成的 P1，不得称为土地利用闭环完成。

## 19. 2026-09-06 人文关系图首版落地

新增 `bevy/crates/core/src/settlement.rs`，并把 `SceneIndex.settlementGraph` 接入 bake：

- `SettlementNode`：场景级聚落节点；
- `DistrictNode`：按建筑功能派生的居住、商业、工业和公共机构片区；
- `ParcelNode`：每个建筑对应的最小可审计地块；
- `PublicSpaceNode`：绿地、保护区和公共空间实体；
- `BuildingRelation`：建筑所属聚落、街区、地块、最近道路、道路距离和关系原因。

每个场景新增：

```text
settlement-graph.json
```

当前状态是 `derived-relationship-contract-v1`。它已经把“建筑为什么属于哪个空间层级”从隐含的 renderer 逻辑提升为 Rust 输出合同，但还不是完整的道路边图、真实 parcel polygon 或多聚落规划器。下一步应让 SettlementPlanner 在生成前创建多个聚落和真实地块，而不是只在 bake 末端从实体反推关系。

## 20. 2026-09-06 自然高度场持久化与独立重算

为避免“自然水文只在 bake 内部存在、CLI 只能信任旧审计 JSON”的问题，chunk descriptor 现在同时记录：

```text
heightFile              最终工程高度场
naturalHeightFile       无 carve 自然高度场
```

每个场景会发布自然高度 chunk：

```text
chunk-x-z.natural.height.bin
```

CLI quality report 会重新加载这些自然 chunk，并重新计算 D8 drainage audit；它不再只读取 `natural-terrain-audit.json` 的历史结果。这样能检测自然高度文件损坏、缺失或与快照不一致。

## 21. 人文关系图当前边界

`settlement-graph.json` 已成为 Rust 输出合同，包含聚落、片区、最小地块、公共空间和建筑关系。但当前它仍是从已生成实体派生的关系图，不能等同于完整的预先规划器。真正的 P1 完成仍要求：

1. 先生成 Settlement/District/Parcel 规划；
2. 再让建筑消耗地块预算；
3. 再让道路和公共空间消耗基础设施预算；
4. 最后从规划事实生成实体和关系图；
5. CLI 独立重建关系图并比较 hash/计数。

当前文档和质量报告会明确保留这一边界。

## 22. 2026-09-06 预算真实性修正

本轮实验验证了一个问题：如果只把 profile 比例投影为规则网格，就会得到 `within_tolerance`，但这并不代表实体、地块和公共空间真的占用了这些土地。因此合同现在同时区分：

- `planned_*_m2`：规划层分配的宏观区域面积；
- `actual_*_m2`：最终生成实体实际覆盖面积；
- `assigned_m2/unassigned_m2`：实际实体覆盖闭环。

预算状态继续依据 `actual_*_m2` 判断，规划网格不能单独让 gate 通过。`land-use-zones.json` 是规划输入/中间事实，不是对最终土地利用完成的证明。

这意味着下一步必须让 ParcelPlanner 在 zone 内产生可消费的地块和公共空间，并在最终实体审计中对照 `planned_*` 与 `actual_*`；在此之前，报告显示 underfilled 是正确行为。

## 23. 2026-09-06 后续校正：不把规划网格当作完成

本轮将 `LandUseZone` 作为独立的规划层合同接入，但验证发现：仅生成规则网格并把网格面积写入预算，会错误地产生 `within_tolerance`，即使实体仍未覆盖这些地块。

因此当前合同同时输出：

- `plannedUrbanM2/plannedRuralM2/plannedNatureM2`：规划层面积；
- `actualUrbanM2/actualRuralM2/actualNatureM2`：最终实体覆盖面积；
- `assignedM2/unassignedM2`：实体闭环；
- `status`：只依据实际实体覆盖判断。

`land-use-zones.json` 现在是后续 ParcelPlanner 的输入，不是完成证明。当前最终产物仍会正确报告 `underfilled_or_mismatched`，直到规划层真正生成地块、公共空间和实体并消耗预算。

同时，DrainageGraph 审计增加 `invalid_samples`，不再把缺失自然高度悄悄当作 0m；最终高度、自然高度和审计数据的职责边界继续保持分离。

## 24. 2026-09-06 当前阶段验收结论

本轮没有把“规则网格规划”误报为土地利用闭环。最终 fresh 产物 `bevy/work/current-final-20260906` 的事实状态是：

- 自然高度场和工程高度场分别持久化；
- `natural-terrain-audit` 通过；
- `settlement-graph` 已持久化；
- `land-use-zones` 已持久化为规划输入；
- `land-use-budget` 同时记录 planned/actual/unassigned；
- 最终实体的实际土地利用仍未达到 profile 目标，因此质量报告保持 `warn`，而不是伪造 `pass`；
- 规则、尺度、入口、碰撞和自然水文门禁仍通过。

HTML 预览已修正 street 相机高度/纵深，当前页面：

```text
http://127.0.0.1:8807/preview/?view=street
```

当前截图可以看见道路、树木、草地、山体和地形边界，但材质/植被密度仍不是最终视觉质量，且人文建筑密度受当前实际实体预算限制。

## 25. 2026-09-06 生成前 Zone → Parcel 首轮接入

本轮将 `plan_landuse_zones` 真正接入 `append_modern_landuse_content`：

```text
profile
→ land-use zones
→ zone center/footprint candidate
→ Rust add() hard placement gate
→ accepted entity
```

Urban、rural、nature zone 现在会在实体生成前尝试生成：

- `residential_block`；
- `farmland`；
- `nature_reserve`。

候选被拒绝时不会扣除预算，原因仍由坡度、道路、水体和碰撞规则记录。这个首轮把 zone 从单纯 JSON 规划输入推进到生成前候选来源，但当前实体覆盖仍明显低于预算目标，说明坡度和既有基础设施约束下还需要真正的 ParcelPlanner：候选排序、zone 内多 parcel 分割、道路走廊预留和公共空间消耗。

最新 1000m fresh 实验的预算事实：

```text
planned urban   249344 m²
planned rural   547328 m²
planned nature  203328 m²
entity urban     86989 m²
entity rural    165741 m²
entity nature    60581 m²
unassigned      686687 m²
```

因此质量报告继续保持 warning，而不是把规划面积当成生成完成。

## 26. 2026-09-06 Zone → multi-parcel realization pass

本轮继续按“宏观规划不等于实体完成”的边界推进 Zone → Parcel：一个 `LandUseZone` 不再只尝试一个大矩形实体，而是被拆成四个确定性的候选地块中心。每个候选仍必须通过统一的 `add()` 硬门禁：场景边界、道路走廊、水体、坡度和占用冲突均可拒绝候选；被拒绝的候选不会扣除土地预算。

这不是把四块矩形直接当作完整城市街区。它只是第一阶段的可实现地块采样，为后续真正的 `ParcelPlanner` 保留内部道路、排水、公共空间和建筑前场。当前生成器仍需要继续记录拒绝原因，并在后续把一个 parcel 拆成 frontage、building pad、public-space 和 service access 等子事实。

同时，对边长至少 5 km 的场景，规划网格使用 240 m 宏单元；小场景保持 64 m 单元。这样避免 6 km × 10 km 场景在规划阶段创建近百万个过细 zone，保持大地图生成的可预测内存和时间成本。宏单元只影响规划候选粒度，不改变自然高度场、最终工程高度场或审计精度。

本轮仍不宣称土地利用闭环完成：最终是否达标必须看 `entity_*`、`assignedM2`、`unassignedM2` 和拒绝统计，而不是看 zone 数量或宏单元面积。

### 26.1 细分地块的尺度合同修正

Zone 内部候选地块现在采用至少 40 m 的宽度和深度下限。这是因为 `scale-audit` 的 `farmland` authored asset contract 明确要求 40–500 m；不能为了提高覆盖率而生成审计不接受的 28 m 田块。

小场景仍使用 64 m 规划单元，大于等于 5 km 的场景使用 240 m 宏单元。这样小场景仍然保留足够的候选数量，而大场景不会把规划层错误地细化到百万级。候选实体仍然服从规则门禁，审计失败时不会被强制保留。

## 27. 2026-09-06 土地利用空间先验修正

`LandUseZone` 不再按行优先顺序把 urban、rural、nature 分别塞进连续条带。规划器现在按到场景中心的径向距离排序：urban 优先靠近中心，rural 位于过渡带，nature 偏向外围。该规则只是人文规划先验，不替代自然高度、水文、坡度和最终实体门禁；所有实体仍需经过同一个 Rust placement contract。

这样修正的是“比例正确但空间形态不真实”的问题：土地利用比例仍由 profile 预算决定，空间关系则开始符合聚落核心—乡村过渡—自然边缘的基本形态。后续仍需将 radial band 替换为 Settlement/District/Parcel 的道路可达性和地形适宜性联合规划。

## 28. 2026-09-06 小场景地块采样尺度修正

验证发现，64 m Zone 内放置四个最小 40 m 地块会产生几何重叠，导致规则阶段大面积拒绝。现在小于 5 km 的规划单元提高到 128 m，并使用约 0.38 的地块比例；四个候选地块之间保留内部间隔，可以同时满足 `farmland` 的最小尺度合同和实体碰撞规则。大地图仍使用 240 m 宏单元。

这不是降低碰撞规则，而是让规划单元的尺度与资产合同一致。后续 ParcelPlanner 应把这些内部间隔显式解释为内部道路、排水、公共空间或服务通道，而不是把它们视为未解释的空白。

## 29. 2026-09-06 Zone 尺度与可实现覆盖率平衡

继续验证后发现，128 m Zone 虽然满足 40 m 最小地块尺度，但在 512 m 场景中候选地块过小，实际覆盖率仍偏低。小于 5 km 的 Zone 规划单元调整为 160 m，并将候选地块比例恢复到 0.42；大于等于 5 km 的场景继续使用 240 m 宏单元。该调整增加的是合法地块的可实现面积，不改变碰撞、坡度、水体和 authored asset contract。

如果候选仍被拒绝，质量报告应继续显示 underfilled；不能通过删除拒绝或放宽硬约束来取得表面覆盖率。

## 30. 2026-09-06 ParcelPlan 生成前合同

新增 `parcel-plan.json` 和 Rust `ParcelPlan` 合同。它从 `land-use-zones.json` 生成可审计的候选地块事实，记录：

- `zoneId` 与 parcel 的父关系；
- urban / rural / nature 土地用途；
- 地块世界坐标和尺寸；
- urban frontage 预留；
- rural/nature drainage 预留；
- urban neighborhood green 公共空间候选。

当前 `ParcelPlan` 已在场景索引中持久化，但仍处于 `planned-before-entity-generation-v1` 阶段。现有实体生成器还需要下一步改为直接消费这些 parcel，而不是再次从 zone 计算候选。这样可以避免规划事实和实体事实分叉；在完成接入前，`parcel-plan.json` 是规划合同，不是实体完成证明。

## 31. 2026-09-06 ParcelPlan 持久化验证

`parcel-plan.json` 已正式写入每个场景目录，并加入 `SceneIndex.parcelPlan`。最新 fresh bake 已同时输出：

```text
land-use-zones.json
parcel-plan.json
settlement-graph.json
land-use-budget.json
```

当前生成顺序为：

```text
LandUseZone
→ ParcelPlan（生成前规划事实）
→ 现有实体候选生成
→ placement / access / terrain audit
→ SettlementGraph（现阶段仍为实体关系派生）
```

这里仍保留一个明确的重构边界：现有实体候选函数还没有完全消费 `ParcelPlan`，因此 `parcel-plan.json` 目前是独立可审计的规划合同，不应被误认为实体已按 parcel 全部实现。下一步将删除 zone 内重复候选计算，让实体生成直接读取 ParcelPlan，并为每个 parcel 记录 accepted / rejected / rejectionReason。

## 32. 2026-09-06 实体生成开始消费 ParcelPlan

本轮将实体候选入口从“再次遍历 Zone”改为直接遍历 `ParcelPlan.parcels`，并额外消费 `PlannedPublicSpace` 生成公共绿地候选。生成顺序现在明确为：

```text
LandUseZone
→ ParcelPlan
→ parcel candidate
→ public-space candidate
→ add() hard gate
→ rules/access/terrain audit
```

`ParcelPlan` 和持久化的 `parcel-plan.json` 使用同一份事实，避免规划文件与生成器重新计算出的地块发生漂移。候选被道路走廊、坡度、水体、边界或碰撞拒绝时，仍然不扣除实际土地预算。

这一步仍不是最终闭环：当前 `ParcelPlan` 尚未记录每个 parcel 的 accepted/rejected 状态和拒绝原因，`SettlementGraph` 也仍是实体关系派生版本。下一步必须把 placement outcome 回写到 parcel 事实，并将内部道路、排水保留带、建筑前场和公共空间从候选变成可审计的基础设施子事实。

## 33. 2026-09-06 SettlementGraph 写入顺序修正

此前 `settlement-graph.json` 在土地利用规划前写入，容易让读者误以为关系图是规划事实的最终视图。本轮将 SettlementGraph 的最终持久化移动到 `ParcelPlan` 创建和实体生成之后：

```text
LandUseZone
→ ParcelPlan
→ parcel/public-space candidates
→ entities
→ SettlementGraph
→ scene.json
```

这保证关系图至少观察到同一轮最终实体和地块规划上下文。当前 graph 的 `status` 仍是 `derived-relationship-contract-v1`，因为它还没有把每个 planned parcel 的 accepted/rejected 状态完整回写；这一点仍是下一阶段必须完成的审计工作。

## 34. 2026-09-06 Parcel realization 状态回写

`PlannedParcel` 现在增加了：

- `realizationStatus`；
- `acceptedEntityIds`；
- `rejectionReason`。

在最终实体生成后，Rust 会按 parcel 的土地用途、位置和 footprint 邻域匹配实际实体，并把结果回写到 `parcel-plan.json`。当前状态为：

```text
planned → accepted
planned → rejected / not-realized-by-placement-gate
```

这是比单纯的实体面积汇总更强的证据，因为每个候选地块都能说明是否真正消耗了实体。当前拒绝原因仍是第一版聚合原因，尚未把 `add()` 和 rules placement 的具体拒绝分类逐条传入 parcel；质量报告仍以最终实体和规则审计为准。

最新 512m fresh bake `continuation-20260906-r16` 验证了该合同：`parcel-plan.json` 已包含 accepted/rejected 状态和 accepted entity IDs。由于实体覆盖下降，`world-quality-gate` 正确返回失败/警告，而不是伪造通过；这暴露出当前 radial zone 与道路/坡度门禁之间仍需要更好的候选排序和地形适宜性规划。

## 35. 2026-09-06 备用候选搜索的验证结论

本轮试验了在 parcel 内移动候选中心以规避道路和坡度阻塞，但审计表明 parcel footprint 已经接近其可用边界，局部移动不能合法增加自由度，反而会降低实体覆盖。该试验已回退，避免引入“候选看似搜索、实际仍只能占用原 footprint”的伪优化。

下一步正确方向不是继续在同一个 parcel 内抖动中心，而是让 `PlannedParcel` 显式携带 zone 级可开发包络，并在包络内重新划分 parcel：先扣除道路 frontage、排水保留带和公共空间，再对剩余区域做可行地块布局。只有这样备用候选才有真实的几何自由度。

## 36. 2026-09-06 小 Zone 公共空间与地块冲突修正

验证 `continuation-20260906-r19` 后发现，小于 240m 的 Zone 已经被定义为单一 neighborhood-scale Parcel，却仍然额外生成一个中心公共绿地候选；这会让公共空间与该 Parcel 发生自碰撞，降低实体覆盖率。现在只有被切分为四个子 Parcel 的大型 urban Zone 才生成独立的 `PlannedPublicSpace`。

小 Zone 不再伪造第二个重叠实体；其公共空间应在下一阶段作为单一 Parcel 内的内部子空间显式扣除，而不是作为外部矩形再次参与硬碰撞。这样规划语义与几何占用一致。

## 37. 2026-09-06 Parcel realization 改为 footprint 包含关系

之前 realization 只按实体中心点距离判断“附近实体”是否属于 Parcel，这会把跨越 Parcel 边界的实体错误计入规划地块。本轮改为使用实体旋转后的 `computed_bounds()`，要求完整 footprint 被包含在 Parcel 的世界矩形内，才允许写入 `acceptedEntityIds`。

这条规则更严格，但更符合可审计地块合同：实体不能只把中心点放在地块附近，必须完整消耗该地块范围。测试夹具同步使用地块内部坐标，避免测试依赖错误的邻近语义。

## 38. 2026-09-06 Parcel 子地块边界修正

大型 Zone 的四个子 Parcel 现在使用显式边距和第二子格偏移计算：

```text
margin + parcel width + second gap + parcel width + margin = zone width
```

不再直接把完整 `zone.width - parcel.width` 同时作为两次偏移量，避免第二个子 Parcel 越出父 Zone。该修正与 footprint 包含式 realization 一起保证：

- Parcel 完整位于所属 Zone；
- accepted entity 的旋转后 footprint 完整位于 Parcel；
- 同一个实体不会仅因中心点接近而被错误计入 Parcel。

最新 `continuation-20260906-r22` 的 scale audit、自然审计和基础规则审计均通过；土地利用实体覆盖仍保持 underfilled，未被伪装为完成。

## 39. 2026-09-06 DevelopmentEnvelope 首版接入

`ParcelPlan` 现在增加 `developmentEnvelopes`。每个 Zone 先生成一个可开发包络，再在包络内切分 Parcel。当前首版显式记录：

- frontage 预留宽度；
- drainage 预留宽度；
- 可开发包络坐标和尺寸；
- urban 大 Zone 的公共空间预留面积。

生成顺序进一步明确为：

```text
LandUseZone
→ DevelopmentEnvelope
→ frontage / drainage reserve
→ Parcel subdivision
→ public-space candidate
→ entity placement
```

这不是把保留带当作实体面积，而是把不能开发的空间先从 Parcel 几何中扣除。`parcel-plan.json` 同时输出 `developmentEnvelopes`，并由测试验证所有 Parcel 的 footprint 都完全位于所属 envelope 内。

## 40. 2026-09-06 DevelopmentEnvelope 保留带测试覆盖

新增测试验证 rural/nature Zone 的 drainage reserve 位于父 Zone 内，并且生成后的 envelope 不会越界。当前 envelope 仍是规划层合同，不直接证明排水沟或道路实体已经生成；它的作用是先把不可开发空间从 Parcel 候选几何中明确扣除。

## 41. 2026-09-06 生成器版本与规划合同同步

`DevelopmentEnvelope` 和 footprint 包含式 Parcel realization 改变了场景规划语义，因此生成器 revision 已从 `worldgen-2026-09-06-r11-natural-integrity` 更新为 `worldgen-2026-09-06-r12-parcel-envelope`。这样旧产物不会被误认为与新 Parcel 合同具有相同的可回放语义。

## 42. 2026-09-06 InfrastructurePlan 预留合同

`ParcelPlan` 现在包含 `InfrastructurePlan`，并单独持久化为：

```text
infrastructure-plan.json
```

首版 corridor 类型：

- `road-frontage-reserve`；
- `drainage-reserve`。

每条 corridor 记录父 Zone、世界坐标、宽深和保留面积。它们目前是生成前基础设施预留事实，尚未伪装成已经生成的道路或排水实体。下一步应让道路图和工程地形 carve 消费这些 corridor，并把 corridor 的 applied/rejected 状态回写。

## 43. 2026-09-06 Infrastructure corridor realization 回写

`PlannedInfrastructureCorridor` 现在也记录：

- `realizationStatus`；
- `acceptedEntityIds`；
- `rejectionReason`。

场景生成完成后，Rust 会用最终实体的完整旋转 footprint 检查 corridor 是否被兼容的道路、人行道、运河或水体实体实际消耗，并将结果写回 `infrastructure-plan.json`。当前仍允许 `reserved-not-realized`，因为规划预留不能自动等同于工程已经实施。

## 44. 2026-09-06 Infrastructure corridor realization 回写验证

最新 `continuation-20260906-r27` 已输出独立 `infrastructure-plan.json`，并在生成后根据最终实体 footprint 回写 corridor 状态。道路 frontage 和 drainage corridor 可以明确显示 `realized` 或 `reserved-not-realized`，不再把预留事实自动当成工程完成。

本轮全量测试达到 244 passed；scale audit、自然水文、规则和访问审计均通过。world-quality-gate 仍因土地利用实体覆盖不足而保持 warn，这是当前规划闭环的真实剩余问题。

## 45. 2026-09-06 frontage corridor 开始消费

本轮将 `road-frontage-reserve` 从仅记录预留，推进到实体候选生成入口：每条 frontage corridor 现在尝试生成一条局部 `road-ew`，仍必须经过统一 `add()` 硬门禁。被坡度、水体、边界或碰撞拒绝时，corridor 会保持 `reserved-not-realized`；成功时由后续 infrastructure realization 写回 `realized`。

这一步没有把 frontage 预留强行当成道路完成，也没有降低道路坡度或碰撞规则。它把规划合同与 Rust 实体生成真正连接起来，为下一步 drainage corridor 的 canal / water-compatible 工程实体消费提供同一入口。

## 46. 2026-09-06 frontage corridor 消费试验边界修正

前一轮尝试将 `road-frontage-reserve` 直接追加生成局部 `road-ew`，但 workspace 的 baked-audit fixture 要求保留真实 pavement geometry；额外道路候选会改变既有审计语义并导致 fixture 失败，因此该试验已回退。

当前保留的可靠合同是：

- corridor 作为 InfrastructurePlan 预留事实持久化；
- 最终实体可以用完整 footprint 包含规则审计；
- `realized` / `reserved-not-realized` 状态可以回写；
- 生成器暂不强制为每条 frontage corridor 追加道路实体。

下一步应在规则和既有 pavement fixture 明确支持 corridor-aware road planner 后，再将 frontage corridor 接入道路生成，而不是在现有道路生成之后追加会改变审计基线的道路。

## 47. 2026-09-06 Parcel 几何一致性门禁

新增 `validate_parcel_plan`：在场景写入前验证每个 Parcel 完整位于所属 DevelopmentEnvelope，并拒绝同一 Zone 内的 Parcel footprint 重叠。该门禁与实体 placement 分开：它验证规划几何本身，不依赖最终 renderer，也不会用实体覆盖率掩盖规划层错误。

当前规划输出因此至少满足：

```text
Zone parentage
→ envelope containment
→ same-zone non-overlap
→ entity footprint realization
```

## 48. 2026-09-06 Parcel 几何一致性门禁验证

`continuation-20260906-r29` 已验证 ParcelPlan 在场景写入前执行：

```text
Zone parentage
→ DevelopmentEnvelope containment
→ same-zone non-overlap
→ Parcel realization
→ Infrastructure realization
```

新增 `validate_parcel_plan` 测试后，workspace 达到 245 passed。fresh bake 的 scale、自然水文、规则和访问审计均通过；world-quality-gate 仍因实体土地利用覆盖不足保持 warn。

## 49. 2026-09-06 保留 corridor 与 envelope 同轴修正

前一版 urban frontage reserve 和 rural drainage reserve 使用了不同的坐标偏移，容易让 corridor 的语义方向与 envelope 扣除方向不一致。本轮统一为轴对齐 v1 合同：保留带位于 Zone 南侧，`DevelopmentEnvelope` 从全部保留带之后开始，Parcel 只在剩余深度内生成。

该版本明确保证：

- corridor footprint 不进入 DevelopmentEnvelope；
- Parcel 不占用 frontage/drainage reserve；
- envelope containment 校验和 infrastructure reserve 语义使用同一坐标轴。

未来如果引入真实道路方向或河网方向，需要升级为带方向的 corridor polygon，而不是继续扩展当前轴对齐合同。

## 50. 2026-09-06 公共空间 envelope 门禁

Parcel 几何门禁现在同时验证 `PlannedPublicSpace`：公共空间必须有所属 envelope，且完整 footprint 位于该 envelope 内。这样规划层不会出现 Parcel 合法但公共空间越界的分裂事实。

## 51. 历史拒绝原因推断说明（撤回）

该节曾声称已经实现 blocked_by_road 推断，但对应编辑未落地。当前采用准确的缺失实体、类型、几何和来源 ID 验证；真实候选拒绝原因仍须从 placement 传递，不能从最终附近道路猜测。

## 52. 2026-09-06 corridor parentage 校验修正

验证中发现，frontage corridor 位于 envelope 之外是设计意图：它属于 Zone 的保留带，而不是 envelope 的内部对象；此前校验错误地把 corridor 当作 envelope 子对象，导致所有带 urban frontage 的 fresh bake 在写入前失败。本轮修正为：

```text
corridor ∈ parent Zone
corridor ∩ DevelopmentEnvelope = ∅（对于同轴保留带）
Parcel ⊂ DevelopmentEnvelope
PublicSpace ⊂ DevelopmentEnvelope
```

因此校验分别验证 Zone containment 和与 envelope 的非重叠，而不再要求 corridor 位于 envelope 内。

## r33 实现记录：生态群落合同（2026-09-07）

### 目标

r32 已将保育地表落实为 patch，并为每个 patch 生成一个代理植物锚点；这仍不足以表达森林、灌丛和湿地的垂直层次。本轮在不改变自然高度场、不让 HTML 反推生态位置的前提下，增加 `EcologyCommunity` 合同。

### 实现

- `SurfaceRealization.ecology.communities` 与来源 conservation patch 一一对应；
- 每个群落记录 `communityKind`、冠层/下木层/地被覆盖率、边缘因子和水文类别；
- Rust `realize` 确定性生成群落，`verify` 独立重建并逐字段比较；
- 合同拒绝未知来源、重复 patch、错误生态分类和越界覆盖率；
- 旧 `ecology.plants` 合同保持不变，因此此项是向后兼容的 schema 扩展；
- HTML 暂时继续使用轻量代理几何，不能把覆盖率字段宣称为完整植被实例化。

### 当前边界

这不是完整生态模拟：覆盖率是确定性规划参数，不包含物种竞争、演替、年龄结构、火灾、地下水或真实生物量守恒。下一步应将一个群落展开为多个 Rust-authored asset anchors，并以密度预算、净空、LOD 和独立重建测试约束它们。

## r34 实现记录：多层生态植物计划（2026-09-07）

### 目标

r33 的群落合同已经描述冠层、下木层和地被层，但只有一个植物锚点，无法让群落在几何上成立。本轮将每个 conservation patch 展开为 Rust-authored 的分层植物计划。

### 生成规则

- forest patch：`canopy`、`understory`、`ground` 三层；
- wetland patch：`emergent`、`understory` 两层；
- shrubland patch：`shrub`、`ground` 两层；
- 每个层有独立类型、尺寸、确定性位置和来源 patch；
- 位置从 patch 的包围范围内生成，并使用最终地形采样高度；
- `verify` 检查每个 patch 的层集合恰好完整、没有重复层和非法类型；
- HTML 只消费这些 Rust 计划，不为没有计划的 patch 添加植物。

### 验收结果

当前平原样例包含 192 个自然 patch、544 个分层植物计划：森林 480 个、湿地 34 个、灌丛/地被 30 个。自然地表面积仍为 49,152 m²；本轮没有改变高度场、土地所有权或城市/农村面积账本。

### 诚实边界

这些是群落结构和资产计划，不是物种级生态模拟；覆盖率仍为启发式规划参数。下一步应加入密度预算、完整 footprint 净空（而非仅中心点）、LOD 合并以及物种/年龄分布。

## r36 实现记录：生态密度预算与完整净空（2026-09-07）

r35 的分层计划已经能表达层级，但没有把群落密度和计划数量作为可审计输入。本轮为 `EcologyCommunity` 增加：

- `densityPerHa`：群落规划密度（启发式计划值，不是现实测量值）；
- `plannedPlantCount`：该 patch 必须展开的层计划数；
- 独立验证要求覆盖率、密度和数量均合法；
- 植物层集合与群落的计划数量必须一致；
- 植物完整 footprint 必须留在 patch 内，并用四角参与硬质实体净空检查。

本轮仍不宣称实现真实生态学、生物量守恒或物种竞争；字段的职责是把后续资产实例化所需的预算变成 Rust 可复核合同。


## r37 状态校正与修复（2026-09-07）

当前实现与实测范围见 [r37 生态审计正向闭环与精确净空](r37-ecology-positive-audit-and-exact-clearance.zh-CN.md)。r33–r36 是历史阶段记录，不作为完整交付证据：五点探测不是完整 footprint 验证，固定 densityPerHa 没有驱动实例数。本轮修复群落索引读取顺序，补充合法原件接受、双份伪造拒绝、精确矩形相交的回归测试。总体开发目标仍未完成。


## r38 面积驱动生态实现（2026-09-07）

见 [r38 面积密度、实例配额与缺额闭环](r38-area-driven-ecology-and-shortfalls.zh-CN.md)。生态规划从landcover.rs拆出；densityPerHa开始实际控制按面积计算的数量，多个实例按层生成，不可放置的实例保留缺额并阻止质量门禁通过。r36/r37固定544个实例的结果是历史基线，不作为本轮预算实现的验收标准。


## r39 生态实例化渲染（2026-09-07）

见 [r39生态chunk实例化与性能验收](r39-ecology-chunk-instancing.zh-CN.md)。保持r38生成数据不变，替换逐植物绘制；按chunk/kind合批、保留instanceIndex至Rust ID映射、释放失活块矩阵，并核对非零origin和重复重绘。该阶段不替代整个6km×10km地图的生成/内存/视觉验收。


## r40 农用通道绕行（2026-09-07）

见 [r40有界农用通道绕行](r40-bounded-farm-access-detours.zh-CN.md)。在保持旧田块和4m/8%/160m限制的前提下，追加全宽验证的绕行搜索。seed=7默认场景仍有主要坡度缺口，不能声称覆盖率改善；另以同输入seed=42的新旧版本对照证明新增通道和田块，完整目标仍未完成。


## r41 显式多尺度田块（2026-09-07）

见 [r41田块长宽与分区程序](r41-explicit-multiscale-field-program.zh-CN.md)。尺寸由manifest表达，Rust校验/执行，面积从真实矩形与mesh复算；保留默认24×24路径，不允许指定大田块时静默缩小。较大田块不保证更高覆盖率，地形整地与农业排水尚未完成。


## r42 精确保护距离查询加速（2026-09-07）

见 [r42保护距离索引](r42-exact-protection-distance-index.zh-CN.md)。以矩形层次索引替换每采样点全分区扫描，保留原Chebyshev栅格距离、0.5倍填土衰减、未知用途保护和边界约定。必须通过逐点参考算法对照及同输入bake不变检查；该微基准不是最大地图完整性能证明。


## r43 真实最大地图基线（2026-09-07）

见 [r43受控最大地图测量](r43-maximum-world-baseline-and-process-measurement.zh-CN.md)。6000×10000m严格release输入保留人文需求、生态与农业；新增只读阶段遥测与原生Windows进程测量。运行若超时/超内存则保留真实失败，不把局部微基准当作总体性能通过。

## r44 实现记录：权威场景与活动区块投影分离（2026-09-07）

详见 [r44 分块读取与权威数据保真](G:/difyllmwiki/GlyphWeave/docs/r44-authoritative-render-stream.zh-CN.md)。Rust 保留完整审计源，同时导出轻量渲染目录和内容寻址地表/生态区块；HTML 不再为单块近景读取整份 scene.json。真实 6×10km 原件的 37,462 个 patch、342,377 个植物全量逐字段核对通过，1,498 个非预览文件在派生导出前后 SHA256 不变。

全量核对发现并修复 serde_json f64 往返一位偏差；增加接缝/部分块/非法输入与异步缓存竞态测试。首块场景数据从 367,474,626 字节降至目录加区块 4,308,728 字节，并验证同页首尾反复导航，不等同于全 240 块漫游稳定性。

本轮没有改变自然/人文生成结果或降低验收阈值：最大图的城乡地表大面积未实现、3 株生态缺额以及地貌/水文科学性仍是必须继续完成的工作。优先级保持为实际开发实现、自然—工程耦合与独立验收；材质、压缩、预取等不能替代这些生成工作。

r44 最终 release CLI 已对最大图再次运行独立 world-quality-gate：166.353 秒、退出 1，仍保留五类用地/地表/生态缺额失败；规则 623/623 和 scale-audit 通过。此阶段结论是读取与保真重构通过，不是整图生成质量通过。

## r45：公共道路标高瓶颈与自然纵断面工程核（2026-09-07）

详见 [r45 自然道路纵断面设计](G:/difyllmwiki/GlyphWeave/docs/r45-natural-road-corridor-design.zh-CN.md)。当前最大图 51,635 个农田候选缺少邻近道路，选中的 246 条道路全部约束于同一 5m 标高。不能通过扩大无路通道预算或填平自然地形掩盖这一前提问题。

新增 Rust 全宽、整数毫米、坡度有界的纵断面求解/网格/验证核，以及与权威世界隔离的 design-road-corridor / verify-road-corridor CLI。真实最大图中一条 1024m 城市走廊，整数平平台至少需要 15.652m 填土，而新纵断面在最大 2.578m 填土和 8% 纵坡内可行；独立源栅格/全枚举/几何核对通过。

**集成尚未完成**：设计只读自然层，没有写入最终地形或增加道路、农田和已实现面积。下一阶段必须将共享路口、非等高道路合同、边坡/工程应用、设施入口和农业接入贯通，再重新做整图验收；不得把研究 HTML 当作整图最终效果。完整目标保持不变。

## r46：非等高道路进入真实 world bake（2026-09-07）

详见 [r46 非等高道路接入与验收](G:/difyllmwiki/GlyphWeave/docs/r46-graded-roads-in-world-bake.zh-CN.md)。新增 manifest 显式连通扩展程序、道路纵断面实体合同、profile 工程操作与最终毫米栅格应用；修复 64m 道路所有权硬编码、旧过渡抬高新路肩、HTML 重复旋转及 f32 紧凑元数据变化。

真实最大图重生成：原 246 道路和 156 站点计划保留；新增两段各 512m 的连通道路后，248/248 道路通过独立检查，农田 270→331，实际覆盖增加 53,810m²。自然层哈希不变，保护区改变样本 0；实体没有删除，一个原绿地实体随最终地形升高 1m。

完整 gate 仍 fail：全图未实现 50,481,522m²，生态缺 1 株，城市建筑仍为 0。当前走廊仍是显式请求，不是自动全域路网；下一步必须做 Rust 自动候选/标高与选线、非等高沿路城市开发、外侧边坡/排水，而不是把手工连接样例当作总目标完成。

## r47：Rust 自动道路生长与实际分区进入（2026-09-07）

详见 [r47 自动道路生长、证据与后续开发](G:/difyllmwiki/GlyphWeave/docs/r47-automatic-road-growth.zh-CN.md)。自动程序只输入用途优先级、候选长度、距离与预算；Rust 从已有路网/不可变自然地形推导端点和毫米标高，经共享纵断面、工程写入及独立审计接入真实世界。不接受逐条坐标，不把分区中心接近道路等同于实际进入；目标要求道路与分区正面积相交且中心距离满足，仍只是规划可达性代理。

最终 6×10km：自动新增 24 条道路，41 次尝试；道路 270/270、站点 156/156、设施 4/4、规则 644/644。农田 270→501，实际覆盖比 r43 增加 269,500m²，总实现 9,734,168m²；城市 106,384m² 仅为道路/农用通道，**城市建筑仍为 0m²**。882 个目标只有 69 个满足，预算耗尽后保留 813 个缺额；全图未实现 50,265,832m²，生态缺 1 株，完整门禁保留六类 fail。

最终生成 452.017 秒，完整 gate 216.332 秒；不是性能 SLA 或整图质量通过。补充整数毫米埋地校验以消除 f32 抵消假阳性，并拒绝没有双方端点节点的隐式 T 路口。Rust 431 测试通过；源高程/几何与派生投影独立检查通过，人工预览仍可见大片空地。

原路网/站点计划和自然哈希保留，但可选对象顺序 ID 不稳定：空间匹配有 218 个改名，另有 1 路灯和 2 绿地代理未在重生成结果出现，不能声称全部存量实体保持。详见 r47 文档的保留差异及原始证据。

**下一步必须优先实现局部毫米标高 frontage、实际城市地块/建筑/入口与用途相容规则，同时建立稳定 ID/存量约束**，而不是继续仅增加道路或手工样例。合法内部路口拆分、绕障和空路网起点、外侧边坡/排水、自然地貌核心、全域土地兑现及开源许可证清单仍未完成。自然层本轮未换算法，也未引入 Mapgen4/symbios-ground 或新增依赖；不能将道路纵断面称为真实侵蚀。完整目标保持未完成。

## r48：局部标高沿路建筑与最大图性能回退（2026-09-07）

详见 [r48 局部道路标高驱动的沿路开发](G:/difyllmwiki/GlyphWeave/docs/r48-local-height-frontage.zh-CN.md)。新增 Rust frontageProgram：在非等高道路完成后按精确类型/尺寸/数量、目标用途与预算选址；平台使用局部整数毫米标高，不使用全局 levelM。入口至道路显式工程支撑解决旧过渡填土形成的单采样凸起，支撑范围与 2m parcel 边缘一致，并保护旧硬支撑。修复商铺道路邻近规则的中心/边界语义、旋转入口朝向以及 canonical JSON 的 f32 升宽误判。

最终小图：请求 6 住宅 + 2 商铺，实现 3 住宅 + 2 商铺，新增真实建筑面积 1,968m²；道路 34/34、站点 27/27、原设施 4/4，保留 3 住宅缺额和用地缺口。实际网格、毫米标高、入口及全支撑独立检查通过；Rust workspace 440 测试通过。HTML 已显示新建筑，但只作为小图阶段成果，不是整体目标完成。

**最大图未交付**：在原 6×10km 背景增加 120 住宅/24 商铺需求后，地表实现耗时从 r47 的 104.514s 增至 281.765s；地表独立重建过程中触及 600s 实验预算。失败目录没有 world.json 和完整场景，不能称为最大图成功。需求未降低，独立校验未取消，失败证据保留。

下一阶段优先定位/优化地表、农用通道与生态的放大成本，并以同输入、完整内容与独立重建继续最大图验收；不能继续堆道路/诊断而不兑现人文地块。自然—工程水文、文化形态、剩余用地、稳定存量 ID 和开源许可证清单仍需完成。总目标保持 active，不以本阶段小图替代。
