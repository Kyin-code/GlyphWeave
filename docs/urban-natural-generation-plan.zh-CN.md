# 现代世界随机地图生成器方案（架构 + 规则 + Rust 落地 + 自检）

> 版本：v0.2 修订稿（2026-09-01）  
> 目标：让 GlyphWeave 在最大约 **6 km × 10 km** 的地图范围内，生成结构合理、可复现、可验证的现代世界地图。  
> 本文是技术设计方案，不是要求 Agent 无条件执行的指令。实现时应以当前引擎代码、资源格式和测试结果为准。

---

## 0. 本次修订解决的问题

本版本在原 v0.1 的基础上，明确回答三个实际问题：

1. **增加新物体时，应该描述什么内容？**
2. **这些文本/数据描述如何变成 Rust 代码并接入引擎？**
3. **生成器如何自动自检，而不是靠人工看地图？**

同时修正以下架构问题：

- 不再只用中心点或简单半径做碰撞判断，而是使用 footprint、clearance 和必要的三维体积；
- 道路地形修改必须在建筑生成前完成，不能在建筑和树生成完后才随意改高度图；
- 增加预留层 `ReservationLayer`，防止未来道路、铁路、河流缓冲区被提前占用；
- 增加入口可达性、道路连通性和最终全局验证；
- 统一优先级定义，避免“数字越大越优先”与“道路优先于树木”的矛盾；
- 新物体优先通过数据描述接入，只有需要复杂行为时才增加 Rust 代码。

---

## 1. 问题定义和设计目标

### 1.1 需要解决的错误

| 错误 | 需要保证的结果 |
|---|---|
| 房子在水里 | 建筑 footprint 不进入水体、洪泛区或禁建水岸缓冲区；特殊建筑必须显式声明例外 |
| 房子悬空 | 建筑底面与最终地形接触，或者有明确的地基、台阶、挡土墙方案 |
| 房子撞墙/重叠 | 建筑主体、屋檐、台阶、围墙和地下部分不发生非法几何相交 |
| 道路穿过房子 | 道路实际走廊与建筑 clearance footprint 不相交 |
| 石头在马路上 | 石头避开道路表面、路肩、人行道和道路安全区 |
| 树挡住小卖部门 | 树避开门前净空区、人行道和顾客到达路径 |
| 建筑没有入口 | 至少一个合法入口连接到人行道或道路网络 |
| 道路断裂 | 主要道路连通城市核心、聚落或地图边界；孤立道路必须被修复或删除 |
| 每个物体单独合法但整体不合理 | 生成结束后进行全局验证、修复、重试或拒绝 |

### 1.2 非目标

本方案暂不解决：

- 高质量最终渲染；
- 复杂室内布局；
- 真实交通流模拟；
- 精确的城市经济和人口模拟；
- 无限地图无分块生成。

这些可以在生成稳定后逐步加入。

---

## 2. 核心原则

### 2.1 生成顺序优先于随机摆放

生成器必须遵循从宏观到微观的顺序：

```text
自然地形/水文
  → 可建设性分析和预留区
  → 城市核心、功能区
  → 道路规划
  → 道路工程和地形修改
  → 街区
  → 地块
  → 建筑和入口
  → 功能设施
  → 树木、岩石和装饰物
  → 全局验证和修复
```

### 2.2 硬约束和软偏好分离

- **硬约束 Hard Constraint**：不满足就拒绝候选位置，例如在水里、与道路相交、坡度超限、越界。
- **软偏好 Soft Preference**：满足多个合法位置时进行评分，例如靠近商业区、靠近公园、面向主路。
- **修复策略 Repair**：候选不合法时尝试地基、旋转、缩放、换位置或删除。

软偏好不能覆盖硬约束。

### 2.3 “高优先级”使用明确枚举

不要依赖容易误解的数字。推荐使用生成阶段：

```rust
pub enum PlacementPhase {
    Terrain = 0,
    WaterAndHazards = 1,
    Railway = 2,
    Road = 3,
    Lot = 4,
    Building = 5,
    Functional = 6,
    Vegetation = 7,
    Decoration = 8,
}
```

阶段数字越小，越早生成。推荐顺序为：

```text
道路 > 地块 > 建筑 > 小卖部/路灯 > 树木/岩石 > 装饰物
```

如果同一阶段内部还需要排序，再使用 `priority`；并明确规定数值越小越优先。

---

## 3. 总体生成流水线

### Stage 0：自然环境

输入或生成：

```text
HeightField       高度图
SlopeField        坡度图
WaterBody         水体、河流、湖泊
WaterSurface      水面高度
BiomeField        生物群系
Floodplain        洪泛区
```

### Stage 1：可建设性分析和预留层

生成以下图层：

```text
BuildabilityLayer   可建设性
HazardLayer         悬崖、滑坡、洪泛、危险坡度
ReservationLayer    道路、铁路、公园、桥梁、管线等预留区
ObstacleLayer       已有不可移动障碍
```

重要区域在对象正式生成前就可以被预留。例如，未来的主路走廊即使还没有生成道路实体，建筑也不能占用。

### Stage 2：宏观城市规划

- 根据坡度、水源、地形、资源和边界选择聚落种子；
- 生成城市核心和功能区：住宅、商业、工业、公园等；
- 规划主路、次路和支路的大致连接关系；
- 生成铁路、桥梁和重要基础设施的预留区。

### Stage 3：道路规划和道路工程

道路不能只表示为一条线，需要生成实际走廊：

```text
RoadCenterline     中心线
RoadSurface        路面
RoadShoulder       路肩
RoadSidewalk       人行道
RoadClearance      道路安全区
RoadCutFill        道路挖方/填方体积
```

道路规划必须检查：

- 最大纵坡；
- 最小转弯半径；
- 路口是否连接；
- 道路是否进入禁建区；
- 跨河时是否生成桥或涵洞；
- 穿山时是否生成隧道或改变路线；
- 是否为后续街区留下足够面积。

道路工程完成后，先提交道路 cut/fill、桥梁、边坡和排水，再更新最终地形。

> **建筑不能在道路地形修改之前生成。**

### Stage 4：街区和地块

- 从道路网络提取街区 `Block`；
- 根据最长边、面积、坡度递归切分成 `Lot`；
- 计算地块的街道边、后边、侧边和入口候选；
- 过滤太小、太陡、被水体切断或没有道路接入的地块；
- 为地块标记 `owner_kind`，例如住宅、商铺、工业或公共设施。

### Stage 5：地块地形和建筑

建筑生成前对地块进行检查和必要的地形处理：

```text
LotGrade       地块平整
Foundation     地基
StepFoundation 台阶式地基
RetainingWall  挡土墙
Drainage       排水
```

建筑必须同时满足：

- 不进入水体、洪泛区和禁建区；
- footprint 不越过地块边界；
- clearance footprint 不与道路、铁路、墙和其他建筑相交；
- 坡度不超过建筑类型限制；
- 至少一个入口朝向道路或人行道；
- 入口有净空和可达路径；
- 建筑底面贴合最终地形，或者有明确地基结构。

### Stage 6：功能设施

生成：

```text
小卖部、商铺、公交站、路灯、停车位、垃圾桶、围墙、电线杆
```

它们必须读取道路、建筑、入口、人行道和地块信息，而不是在全地图上独立随机。

### Stage 7：自然和装饰物

最后生成：

```text
树木、灌木、岩石、草地、广告牌、长椅、装饰物
```

自然物体采用 Poisson disk 或候选点采样，但必须避开已有的道路、建筑、入口、管线、铁路和安全区。

### Stage 8：全局自检和修复

对整张地图进行最终验证。验证失败时：

```text
可局部修复 → Repair
可以换候选 → Retry
没有合法方案 → Reject/Delete
```

---

## 4. 数据结构

```rust
pub struct Block {
    pub polygon: Polygon2,
    pub road_edges: Vec<RoadId>,
    pub avg_slope: f32,
    pub area: f32,
    pub neighbors: Vec<BlockId>,
}

pub struct Lot {
    pub polygon: Polygon2,
    pub street_sides: Vec<StreetSide>,
    pub entrance_candidates: Vec<EntranceCandidate>,
    pub area: f32,
    pub owner_kind: LotKind,
}

pub struct PlacedItem {
    pub id: ItemId,
    pub kind: String,
    pub transform: Transform,
    pub footprint: Polygon2,
    pub clearance: Polygon2,
    pub bounds_3d: Aabb3,
    pub entrance_zones: Vec<EntranceZone>,
    pub tags: Vec<String>,
}

pub struct EnvironmentQuery {
    pub height: f32,
    pub water_surface: Option<f32>,
    pub biome: Biome,
    pub slope: f32,
    pub hazard: HazardFlags,
}

pub struct Reservation {
    pub owner: ReservationOwner,
    pub shape: Polygon2,
    pub hard: bool,
    pub phase: PlacementPhase,
}
```

### 4.1 空间索引必须支持几何查询

`SpatialIndex` 至少需要支持：

```text
query_shape(shape)                 查询几何相交
query_kind_shape(kind, shape)      查询指定类型相交
query_clearance(shape)              查询安全区冲突
query_reservation(shape)           查询预留区冲突
query_height_volume(bounds_3d)      查询三维范围冲突
```

不要只实现 `queryPoint(x, z)` 或“中心点距离检查”。

---

## 5. 增加新物体时，描述哪些内容？

新物体不应该只写一个名称和模型路径。应描述以下五类内容：

### 5.1 身份和资源

```yaml
id: street_bench
kind: street_furniture
asset: assets/props/street_bench.glb
category: functional
```

### 5.2 几何和朝向

```yaml
geometry:
  footprint: [1.8, 0.7]
  height: 0.9
  pivot: center
  rotation: any
  clearance: 0.5
```

含义：

- `footprint`：物体实际占地；
- `height`：三维碰撞高度；
- `pivot`：模型原点位置；
- `rotation`：允许的朝向；
- `clearance`：维护和安全间距。

### 5.3 环境限制

```yaml
environment:
  on_ground: true
  not_in_water: true
  max_slope: 0.25
  allowed_biomes: [grassland, urban_green]
  forbidden_hazards: [cliff, floodplain]
```

### 5.4 与其他物体的关系

```yaml
relations:
  avoid:
    - kind: road_surface
      distance: 0.3
    - kind: building
      distance: 1.0
  prefer:
    - kind: sidewalk
      distance: 0.5
  require:
    - kind: sidewalk
      distance: 2.0
```

### 5.5 功能区和连接区

有门、窗口、车道、服务口或朝向要求的物体，必须定义锚点：

```yaml
anchors:
  - id: front
    side: north
    type: public_access
    clear_radius: 1.5
    must_face: sidewalk
  - id: service
    side: south
    type: maintenance
    clear_radius: 1.0
```

### 5.6 生成策略和失败策略

```yaml
placement:
  phase: functional
  priority: 60
  attempts: 12
  allow_scale: false
  allow_rotate: true
  fallback: skip
  repair:
    - rotate_to_nearest_sidewalk
    - move_to_nearest_valid_candidate
```

一个新物体的完整描述应当能回答：

```text
它是什么？
占多大地方？
需要什么地面？
不能靠近什么？
喜欢靠近什么？
门/入口在哪里？
什么时候生成？
失败时怎么处理？
```

---

## 6. 新物体的推荐描述格式

推荐使用 **TOML 或 JSON** 保存规则。早期开发推荐 TOML，便于人工阅读；引擎内部统一转换成 Rust 类型。

### 6.1 示例：街边长椅

文件：

```text
assets/objects/street_bench.object.toml
```

内容：

```toml
id = "street_bench"
kind = "street_furniture"
asset = "assets/props/street_bench.glb"
category = "functional"

[geometry]
footprint = [1.8, 0.7]
height = 0.9
clearance = 0.5
pivot = "center"
rotations = "align_to_target"

[environment]
on_ground = true
not_in_water = true
max_slope = 0.20
allowed_biomes = ["urban", "park", "urban_green"]
forbidden_hazards = ["cliff", "floodplain"]

[[relations.avoid]]
kind = "road_surface"
distance = 0.3

[[relations.avoid]]
kind = "building"
distance = 1.0

[[relations.require]]
kind = "sidewalk"
distance = 0.8

[[anchors]]
id = "seat_front"
side = "front"
type = "public_access"
clear_radius = 1.2
must_face = "sidewalk"

[placement]
phase = "functional"
priority = 60
attempts = 12
allow_rotate = true
fallback = "skip"
```

### 6.2 规则中必须区分“实体类型”和“语义标签”

例如：

```text
kind = building
kind = tree
kind = road_surface
```

是实体类型；

```text
tags = storefront, public_access, flammable, tall
```

是语义标签。

规则既可以写：

```yaml
avoid_kind: building
```

也可以写：

```yaml
avoid_tag: flammable
```

这样新增“木棚”和新增“商铺”时，不需要修改所有旧规则。

---

## 7. 文本描述如何变成 Rust 代码？

### 7.1 不建议让 Rust 直接解析自然语言

以下描述不能直接可靠地交给引擎：

```text
这个长椅应该自然地放在路边，不要挡住行人。
```

应该先转换成结构化规则：

```text
需要人行道；远离道路表面；入口方向面向人行道；保留 1.2m 净空。
```

再写成 TOML/JSON。自然语言可以作为注释或编辑器辅助输入，但不能作为运行时唯一依据。

### 7.2 Rust 中定义强类型 Schema

文件建议：

```text
bevy/crates/core/src/rules/schema.rs
```

示例：

```rust
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct ObjectDescriptor {
    pub id: String,
    pub kind: ItemKind,
    pub asset: String,
    pub category: ItemCategory,
    pub geometry: GeometrySpec,
    pub environment: EnvironmentSpec,
    #[serde(default)]
    pub relations: RelationSpec,
    #[serde(default)]
    pub anchors: Vec<AnchorSpec>,
    pub placement: PlacementSpec,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GeometrySpec {
    pub footprint: [f32; 2],
    pub height: f32,
    pub clearance: f32,
    pub pivot: Pivot,
    pub rotations: RotationMode,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EnvironmentSpec {
    pub on_ground: bool,
    pub not_in_water: bool,
    pub max_slope: f32,
    #[serde(default)]
    pub allowed_biomes: Vec<Biome>,
    #[serde(default)]
    pub forbidden_hazards: Vec<HazardKind>,
}
```

### 7.3 加载、校验和编译

运行时流程：

```text
读取 *.object.toml
    ↓
serde 反序列化
    ↓
Schema 校验
    ↓
ObjectDescriptor
    ↓
编译成 RuntimeRule
    ↓
注册到 ObjectRegistry
    ↓
生成器按阶段调用
```

示例接口：

```rust
pub struct ObjectRegistry {
    descriptors: HashMap<String, ObjectDescriptor>,
}

impl ObjectRegistry {
    pub fn load_dir(path: &Path) -> Result<Self, RuleLoadError> {
        // 读取目录、解析 TOML、校验 id 和资源路径、建立索引
        todo!()
    }

    pub fn get(&self, id: &str) -> Option<&ObjectDescriptor> {
        self.descriptors.get(id)
    }
}
```

### 7.4 将描述编译为统一的候选验证器

```rust
pub struct Candidate<'a> {
    pub transform: Transform,
    pub footprint: Polygon2,
    pub clearance: Polygon2,
    pub bounds_3d: Aabb3,
    pub anchors: Vec<WorldAnchor>,
    pub target: &'a TargetSurface,
}

pub struct PlacementContext<'a> {
    pub environment: &'a EnvironmentLayer,
    pub reservations: &'a ReservationLayer,
    pub spatial_index: &'a SpatialIndex,
    pub road_graph: &'a RoadGraph,
    pub nav_graph: &'a NavigationGraph,
}

pub enum RejectReason {
    OutOfBounds,
    InWater,
    ForbiddenHazard,
    SlopeTooHigh,
    ReservationConflict,
    GeometryCollision,
    MissingRequiredRelation,
    BlockedEntrance,
    NotGrounded,
    DisconnectedAccess,
}

pub fn validate_candidate(
    item: &ObjectDescriptor,
    candidate: &Candidate,
    ctx: &PlacementContext,
) -> Result<(), RejectReason> {
    // 1. 地图边界
    // 2. footprint/clearance 与 ReservationLayer 相交检查
    // 3. 水体、洪泛区、生物群系、坡度检查
    // 4. 与 SpatialIndex 中已有实体的几何碰撞检查
    // 5. 锚点、入口净空和道路/人行道连接检查
    // 6. 地形接触或地基检查
    todo!()
}
```

重点是：

> **TOML/JSON 不直接执行代码，而是被 Rust 解析成强类型描述，再由通用验证器执行。**

### 7.5 什么时候需要新增 Rust 代码？

新增物体分为三类：

#### A. 纯数据型物体：只需新增描述文件

例如：

```text
树、石头、长椅、路灯、垃圾桶、普通小卖部、广告牌
```

它们只需要不同的尺寸、规则、资源和标签。

#### B. 参数化物体：新增描述文件 + 通用生成器参数

例如：

```text
住宅、商铺、仓库、停车场、围墙、农田
```

需要指定：

```text
建筑模板
层数范围
入口数量
屋顶类型
地块适配方式
```

但仍然不一定要增加 Rust 类型。

#### C. 行为型物体：需要 Rust 生成器

例如：

```text
道路、桥梁、隧道、河流、铁路、复杂电线网络
```

因为它们会改变地形、连接图或影响多个对象，不能只靠单个 TOML 规则解决。

推荐接口：

```rust
pub trait ProceduralGenerator {
    fn generate(
        &self,
        descriptor: &ObjectDescriptor,
        context: &mut GenerationContext,
    ) -> Result<Vec<PlacedItem>, GenerationError>;
}
```

注册方式：

```rust
registry.register_generator("road", Box::new(RoadGenerator::default()));
registry.register_generator("bridge", Box::new(BridgeGenerator::default()));
registry.register_generator("building", Box::new(BuildingGenerator::default()));
```

---

## 8. 新增物体的完整工作流

以后增加一个新物体，按以下流程执行：

### 第一步：准备资源

```text
模型/实例资源
碰撞包围盒或 footprint
原点和朝向约定
缩放单位
```

必须明确：

```text
1 个引擎单位等于多少米
模型正面朝哪个方向
原点是在中心、底部还是入口处
```

### 第二步：创建描述文件

例如：

```text
assets/objects/bus_stop.object.toml
```

填写：

```text
身份、资源、尺寸、环境限制、关系、锚点、生成阶段、失败策略
```

### 第三步：运行规则校验器

```bash
cargo run -p glyphweave -- rules validate assets/objects/bus_stop.object.toml
```

应检查：

- `id` 是否重复；
- 资源路径是否存在；
- footprint 和 height 是否大于零；
- 距离是否为非负数；
- `must_face` 是否引用有效类型或标签；
- 生成阶段和优先级是否有效；
- 入口是否有净空；
- 规则是否存在自相矛盾。

### 第四步：加入场景和分布规则

描述文件只说明“这个物体怎样才合法”，还需要在场景配置中说明“它在哪里出现、出现多少”。

例如：

```toml
[distribution.bus_stop]
allowed_zones = ["commercial", "residential"]
near = ["road_primary", "road_secondary"]
min_road_spacing = 250.0
max_per_region = 12
```

### 第五步：运行固定种子测试

```bash
cargo test -p glyphweave -- object::bus_stop
cargo run -p glyphweave -- generate --seed 12345 --validation-report
```

### 第六步：查看调试输出

生成器应支持输出：

```text
地图文件
验证报告 JSON
被拒绝候选列表
规则命中统计
道路/地块/建筑 debug layer
```

### 第七步：加入回归测试

新物体至少加入：

```text
一个正常生成案例
一个靠近障碍物的拒绝案例
一个坡度/水体边界案例
一个入口被阻挡案例（如适用）
```

---

## 9. 统一放置算法

```text
place(descriptor, candidate_positions, context):
  1. 按 PlacementPhase 和 priority 排序
  2. 为每个候选位置生成实际 footprint、clearance、入口区和三维 bounds
  3. 查询 EnvironmentLayer：高度、水面、坡度、生物群系、危险区
  4. 查询 ReservationLayer：道路、铁路、河流、公园、管线等预留区
  5. 查询 SpatialIndex：几何碰撞和安全距离
  6. 检查硬约束：越界、水体、坡度、碰撞、地面接触
  7. 检查功能约束：入口朝向、道路/人行道关系、必要邻居
  8. 为剩余候选计算软约束评分
  9. 尝试 Repair：旋转、平移、地基、局部 cut/fill
 10. 通过后以事务方式 commit：实体、占用区、入口、连接边一起登记
 11. 所有候选都失败时记录 RejectReason，不强行放置
```

### 9.1 事务式提交

不能出现“物体已经加入实体列表，但空间索引没有登记”的半完成状态。

```rust
pub struct PlacementTransaction {
    item: PlacedItem,
    reservations: Vec<Reservation>,
    nav_edges: Vec<NavEdge>,
}

impl PlacementTransaction {
    pub fn validate(&self, ctx: &PlacementContext) -> Result<(), RejectReason>;
    pub fn commit(self, ctx: &mut GenerationContext);
    pub fn rollback(self);
}
```

---

## 10. 自检系统：如何知道生成结果是否合格？

自检必须分成四层，而不是只做一个“有没有重叠”的检查。

### 10.1 Schema 自检

在加载描述文件时检查：

```text
字段完整
类型正确
枚举值有效
路径存在
尺寸合理
规则无矛盾
id 唯一
```

### 10.2 单物体自检

验证每个物体自身：

```text
footprint 非空
height > 0
pivot 正确
底部没有悬空
没有穿入水体
入口区没有和自己的主体重叠
```

### 10.3 关系自检

验证物体之间：

```text
建筑与道路
树与建筑
树与入口
岩石与道路
商铺与人行道
桥与道路
铁路与建筑
```

每次拒绝都必须带有机器可读原因：

```json
{
  "item": "house_042",
  "candidate": [381.5, 122.0],
  "reason": "ReservationConflict",
  "conflict_with": "road_primary_07",
  "rule": "avoid.road_clearance"
}
```

### 10.4 全局自检

地图生成结束后检查：

```text
所有道路是否连通
主路是否连接城市核心或地图边界
所有建筑是否有合法入口
所有地块是否有道路接入
是否存在孤立建筑
是否存在重叠或穿插
是否存在悬空或埋入过深
是否存在水下对象
是否有物体越界
是否有道路进入不可通过区域
是否有大量对象因为规则冲突全部失败
```

### 10.5 不变量（Invariant）

每个有效地图必须满足以下不变量：

```text
I1: 所有 PlacedItem 都在地图边界内
I2: 所有硬性禁区不与非法对象相交
I3: 建筑 clearance 不与 RoadClearance 相交
I4: 入口净空区不被树、石头、灯和其他设施阻挡
I5: 建筑要么接触地形，要么具有已登记地基
I6: 所有要求道路接入的建筑都连接到 RoadGraph/NavGraph
I7: 所有桥梁两端都连接道路
I8: 所有实体、空间索引、导航图和 debug 数据保持一致
```

### 10.6 固定种子回归测试

必须支持确定性生成：

```text
同一个版本 + 同一个 seed + 同一套资源 = 相同的规划结果
```

每次修改规则或算法后，至少测试：

```text
平地
丘陵
山区
河流穿越
湖泊边缘
狭窄地块
高密度商业区
极端随机种子
```

可以保存摘要而不是保存完整二进制地图：

```json
{
  "seed": 12345,
  "buildings": 184,
  "roads": 62,
  "floating_items": 0,
  "submerged_items": 0,
  "blocked_entrances": 0,
  "geometry_collisions": 0,
  "disconnected_roads": 0
}
```

### 10.7 属性测试和随机压力测试

Rust 侧建议使用属性测试，验证大量 seed：

```rust
proptest! {
    #[test]
    fn generated_map_preserves_invariants(seed in any::<u64>()) {
        let map = generate(seed);
        assert!(map.validation.floating_items == 0);
        assert!(map.validation.submerged_items == 0);
        assert!(map.validation.geometry_collisions == 0);
    }
}
```

对于某些随机种子无法满足全部城市规模要求时，可以允许“少生成对象”，但不能允许硬约束错误。

### 10.8 Debug Layer 和可视化自检

即使暂时不考虑最终渲染，也要输出调试数据：

```text
道路中心线
道路实际走廊
建筑 footprint
建筑 clearance
入口净空区
水体和洪泛区
禁建区
被拒绝的候选点
碰撞位置
悬空位置
```

建议支持：

```text
--debug-layer roads
--debug-layer reservations
--debug-layer footprints
--debug-layer rejected
--validation-report report.json
```

“不考虑渲染”不等于“不需要调试可视化”。没有 debug layer，很难定位规则究竟哪里出错。

---

## 11. 性能和 6 km × 10 km 地图限制

最大地图约为 60 km²，不应把所有对象都放在一个全局列表中进行 O(n²) 检查。

建议：

```text
地图分成 region/chunk
每个 chunk 有局部空间索引
跨 chunk 查询使用边界缓存
道路和河流等长对象使用分段索引
规则校验只查询 footprint 的包围盒覆盖区域
```

生成顺序可以是：

```text
先完成宏观规划
再按 chunk 生成街区、地块和对象
最后进行跨 chunk 连通性和边界验证
```

必须保留：

```text
固定 seed
确定性排序
稳定的对象 id
可重复的候选点顺序
```

否则同一地图在不同线程顺序下可能出现不同结果，导致难以复现的 bug。

---

## 12. 推荐目录结构

```text
bevy/crates/core/src/planning/
  city.rs
  road.rs
  block.rs
  lot.rs
  terrain_edit.rs

bevy/crates/core/src/rules/
  schema.rs
  loader.rs
  validator.rs
  compiler.rs
  placement.rs
  registry.rs
  errors.rs

bevy/crates/core/src/spatial/
  index.rs
  geometry.rs
  collision.rs

bevy/crates/core/src/validation/
  report.rs
  invariants.rs
  connectivity.rs
  regression.rs

assets/objects/
  tree.object.toml
  rock.object.toml
  street_bench.object.toml
  storefront.object.toml
  bus_stop.object.toml

schemas/
  object-descriptor.schema.json
  distribution.schema.json
```

规则优先采用数据驱动；道路、桥梁、地形修改和导航连接等复杂系统使用 Rust 生成器。

---

## 13. 实施路线图

### Phase A：先实现统一规则引擎和自检基础

这一阶段不是为每个物体分别写一套逻辑，而是先建立所有对象共用的运行时基础：

1. `ObjectDescriptor` 强类型结构；
2. TOML/JSON 加载器和 schema 校验器；
3. 通用 `Constraint` 约束解释器；
4. `EnvironmentLayer`、`ReservationLayer`；
5. 支持 footprint/clearance 的 `SpatialIndex`；
6. 统一候选生成、硬约束验证、软约束评分、修复、重试、提交流程；
7. `ValidationReport` 和机器可读 `RejectReason`；
8. `ObjectRegistry`，支持通过配置文件注册新对象；
9. 先接入三个基准对象：道路、建筑、树木；
10. 再接入岩石、路灯、小卖部，验证“新增普通对象无需修改核心 Rust 代码”。

验收目标：解决门前有树、房在水里、房悬空、物体重叠、石头上路等问题，并证明新增加的普通对象可以通过描述文件接入。

### Phase B：道路和地形工程

1. 道路走廊；
2. 道路坡度和路口验证；
3. 道路 cut/fill；
4. 桥梁、涵洞、隧道占位；
5. 道路完成后重新计算地形和可建设层。

### Phase C：街区、地块和入口

1. 道路提取街区；
2. 街区切分地块；
3. 建筑退界和入口候选；
4. 人行道/车道连接；
5. 建筑、商铺和公共设施按地块生成。

### Phase D：全局验证和随机压力测试

1. 固定种子回归测试；
2. 多地形属性测试；
3. 连通性测试；
4. 大地图分块测试；
5. 生成报告和 debug layer；
6. 统计规则拒绝率和空地率。

### Phase E：宏观自然性和丰富度

1. 兴趣地图；
2. 聚落种子；
3. 张量场或 L-system 道路；
4. 功能区密度梯度；
5. 树木、灌木、岩石和季节化内容。

---

## 14. 验收标准

在没有最终渲染的情况下，也应该可以通过几何和数据测试验收：

### 硬性标准

```text
随机测试种子中，非法水下建筑 = 0
随机测试种子中，未处理的悬空建筑 = 0
随机测试种子中，道路与建筑 clearance 非法相交 = 0
随机测试种子中，入口被硬性物体阻挡 = 0
随机测试种子中，未登记的空间索引实体 = 0
```

### 软性标准

```text
大多数住宅有道路或人行道接入
商业区附近有足够的商铺聚集
道路断头率在目标范围内
地块废弃率在目标范围内
树木和岩石不集中堆积在功能区入口
```

软性标准应配置成可调参数，不能写死为所有地图相同的数字。

---

## 15. 阶段性结论

原 v0.1 的核心思路可以保留，但必须补上四个层面：

```text
1. 新物体的结构化描述格式
2. 描述文件到 Rust 强类型运行时的转换
3. 事务式放置和空间/导航登记
4. 单物体、关系、全局、回归四层自检
```

最终架构不是：

```text
随机生成物体 + 每个物体写一点 avoid 规则
```

而应该是：

```text
环境图层
+ 预留图层
+ 道路/街区/地块规划
+ 数据驱动对象描述
+ Rust 通用规则编译器
+ 事务式放置
+ 地形工程
+ 全局验证与可复现测试
```

这样以后新增一个物体时，通常只需要：

```text
准备资源
→ 编写 .object.toml
→ 通过 schema 校验
→ 配置分布规则
→ 运行固定种子和压力测试
→ 加入回归测试
```

只有道路、桥梁、隧道、河流等会改变地形、连接图或多个对象关系的复杂物体，才需要新增 Rust 生成器代码。

---

## 16. 统一规则引擎和 Agent 扩展协议

### 16.1 结论：规则可以近似统一，但不能把所有对象强行做成同一种生成器

推荐采用“三层架构”：

```text
对象描述层：TOML/JSON，声明对象的资源、尺寸和规则
        ↓
通用规则层：Rust，实现环境、碰撞、距离、入口和连接性约束
        ↓
特殊生成器层：Rust，仅处理道路、桥梁、隧道、复杂建筑等行为型对象
```

因此：

| 对象类别 | 新增对象的主要工作 | 是否需要新增 Rust 生成器 |
|---|---|---:|
| 静态普通物体：树、石头、长椅、路灯、垃圾桶 | 新增资源和 `.object.toml` | 否 |
| 参数化物体：住宅、商铺、仓库、停车场 | 新增描述，选择已有模板/生成器 | 通常否 |
| 系统型物体：道路、桥梁、隧道、铁路、管线 | 描述规则 + 专用生成算法 | 是 |

目标不是“所有对象零代码”，而是：

> **大多数对象只需要数据；少数会改变地形、道路图或导航图的对象才需要代码。**

### 16.2 通用约束类型

所有对象都应尽量使用统一的约束枚举，而不是为每个对象写独立的 `if/else`：

```rust
pub enum Constraint {
    InsideBounds,
    OnGround,
    NotInWater,
    MaxSlope(f32),
    AllowedBiome(Vec<Biome>),
    ForbiddenHazard(Vec<HazardKind>),
    AvoidKind { kind: ItemKind, distance: f32 },
    AvoidTag { tag: String, distance: f32 },
    RequireNear { kind: ItemKind, distance: f32 },
    PreferNear { kind: ItemKind, distance: f32, weight: f32 },
    FrontOn { kind: ItemKind },
    ConnectTo { graph: GraphKind },
    ClearAnchor { anchor: String },
}
```

建议把规则按用途分成四组：

```text
环境约束：OnGround、NotInWater、MaxSlope、AllowedBiome
几何约束：InsideBounds、AvoidKind、AvoidTag、Clearance
功能约束：FrontOn、RequireNear、ClearAnchor、ConnectTo
偏好评分：PreferNear、PreferAway、Density、Orientation
```

前面三组可以作为硬约束，最后一组只用于合法候选之间的排序。任何偏好都不能覆盖硬约束。

### 16.3 统一放置流程

```text
读取对象描述
    ↓
根据生成阶段产生候选位置
    ↓
根据模型尺寸计算 footprint、clearance、入口区和三维范围
    ↓
执行硬约束检查
    ↓
执行关系和可达性检查
    ↓
对通过的候选进行软评分
    ↓
尝试旋转、平移、地基或局部修复
    ↓
事务式提交实体、占用区和导航连接
    ↓
提交后再次执行局部验证
```

所有普通对象都走同一条流程。不同对象的差异主要来自：

```text
描述文件中的参数
候选位置来源
对象的 footprint/anchor
是否绑定专用生成器
```

### 16.4 新增普通物体的 Agent 工作流

以后新增树、石头、长椅、路灯、垃圾桶等对象时，Agent 应执行下面的固定流程：

```text
1. 检查对象资源和模型坐标
2. 确认 footprint、height、pivot 和正面方向
3. 创建 `.object.toml`
4. 选择环境、几何、关系和入口规则
5. 配置对象分布区域和数量限制
6. 运行 schema 校验
7. 运行规则冲突检查
8. 生成固定 seed 测试
9. 运行多地形压力测试
10. 通过后加入 ObjectRegistry
```

Agent 不应直接把自然语言当成运行时规则。例如：

```text
“长椅自然地放在路边，不要挡住行人”
```

必须具体化为：

```text
要求靠近 sidewalk；
避开 road_surface；
入口/使用侧保留 1.2m 净空；
不得进入 building clearance；
不得位于水体和危险坡面。
```

### 16.5 Agent 生成的最小文件集合

静态普通物体至少需要：

```text
assets/objects/<id>.object.toml
assets/objects/<id>.test.toml
```

其中：

- `.object.toml` 描述对象本身的合法性；
- `.test.toml` 描述至少一个成功案例和多个失败案例；
- 分布规则可以放在场景配置中，不要把“出现数量”硬编码在对象规则里。

示例：

```toml
# street_bench.test.toml

[[cases]]
name = "near_sidewalk"
scene = "flat_urban"
expect = "placed"

[[cases]]
name = "overlap_road"
scene = "bench_on_road"
expect = "rejected"
reason = "GeometryCollision"

[[cases]]
name = "blocked_usage_side"
scene = "bench_blocked_by_tree"
expect = "rejected"
reason = "BlockedEntrance"
```

### 16.6 什么时候必须新增 Rust 代码

满足下面任一条件时，不能只新增 TOML：

```text
对象会修改高度图或水文
对象会创建、拆分或连接道路/导航图
对象需要多个阶段协同生成
对象的形状不是固定 footprint，而是由环境计算出来
对象需要搜索路径、拓扑、交通或流体关系
对象具有复杂的地基、桥墩、隧道或边坡结构
```

典型专用生成器包括：

```rust
RoadGenerator
BridgeGenerator
TunnelGenerator
RailwayGenerator
BuildingGenerator
TerrainEditGenerator
```

专用生成器只负责“如何生成”，合法性检查仍然应该调用统一规则引擎。

### 16.7 规则系统的边界

统一规则引擎可以判断：

```text
物体能不能放在这里
物体会不会碰撞
物体是否符合环境
物体是否有入口
物体是否连到道路或人行道
```

但它不能单独决定：

```text
城市核心应该在哪里
道路应该怎么连接
哪里应该形成商业街
街区应当多大
建筑密度如何变化
```

这些属于规划器：

```text
CityPlanner
RoadPlanner
BlockExtractor
LotSubdivider
DensityPlanner
```

因此不能把所有城市逻辑都塞进对象规则文件。

---

## 17. 推荐的最小可行实现（MVP）

为了避免一开始过度设计，第一版只实现以下能力：

```text
1. TOML 对象描述
2. Rust serde 加载和 schema 校验
3. InsideBounds
4. OnGround
5. NotInWater
6. MaxSlope
7. AvoidKind
8. NoGeometryCollision
9. ClearAnchor
10. ObjectRegistry
11. RejectReason
12. 固定 seed 测试
13. JSON 验证报告
```

第一批对象：

```text
道路、普通住宅、树木、石头、小卖部
```

第一批验收场景：

```text
房子放在水里        → 拒绝
房子放在陡坡上      → 拒绝或生成地基
房子与道路相交      → 拒绝
树木位于小卖部门口  → 拒绝
石头位于道路表面    → 拒绝
合法候选位置存在    → 成功放置
没有合法位置        → 跳过并记录原因
```

先让这套闭环稳定：

```text
描述 → 加载 → 候选 → 验证 → 放置 → 登记 → 自检
```

然后再增加街区、地块、道路张量场和复杂地形工程。

---

## 18. 对原实施路线的最终调整

推荐实际执行顺序：

```text
Phase A：统一规则引擎 + 描述文件 + 自检
Phase B：道路走廊 + 道路地形工程
Phase C：街区 + 地块 + 建筑入口
Phase D：全局连通性 + 固定种子 + 压力测试
Phase E：宏观自然性 + 更多对象类型
```

不要先为几十种物体分别编写规则代码。正确顺序是：

```text
先写一套通用规则解释器
→ 用道路/建筑/树木验证
→ 把普通对象迁移到配置文件
→ 让 Agent 按协议新增对象
→ 通过自动测试后注册
```

这套方式能够控制代码规模，也能避免每新增一个物体就引入一套互相不兼容的判断逻辑。


---

## 19. 生成编排：不是简单排序，而是依赖图 + 可回退流程

### 19.1 为什么需要编排器

生成器不能只写成：

```text
先生成所有树
再生成所有房子
最后生成道路
```

也不能简单地认为“道路永远沿着原始地形，建筑永远适应道路”。现代社会中的道路、铁路、桥梁、隧道和大型建筑可能会改变地形，因此需要一个负责安排阶段、依赖关系、回退和重新生成的 `GenerationOrchestrator`。

编排器要解决四个问题：

```text
什么必须先生成？
什么可以并行生成？
什么变化会使已经生成的结果失效？
失败后应该重试、改路线、改地形，还是放弃？
```

### 19.2 生成顺序应由依赖关系决定

推荐的依赖图如下：

```text
原始地形、水文、危险区
          ↓
城市需求、聚落种子、功能区
          ↓
道路/铁路候选走廊
          ↓
道路工程方案：沿地形、挖方填方、桥梁、隧道
          ↓
工程后最终地形和更新后的水文/可建设性
          ↓
街区和地块
          ↓
建筑、地基、入口、人行道
          ↓
小卖部、公交站、路灯、停车位等设施
          ↓
树木、岩石、灌木和装饰物
          ↓
全局验证、修复和统计
```

这里的“先后”不是所有对象的绝对先后，而是数据依赖：

- 树木依赖道路和建筑的最终位置；
- 建筑依赖地块和道路完成后的地形；
- 地块依赖道路走廊；
- 道路方案依赖城市核心、地形和工程规则；
- 道路工程会修改地形，因此会使后续地块和建筑重新计算。

### 19.3 固定阶段和可迭代阶段

建议将流程分成两类：

#### 固定阶段

这些阶段完成后，通常不应被低优先级对象推翻：

```text
基础水文和硬禁建区
城市核心和重要聚落
主路/铁路的连接目标
道路工程方案
街区边界和地块边界
```

#### 可迭代阶段

这些阶段允许失败后重新尝试：

```text
次级道路
地块切分
建筑选型
建筑朝向
商铺和公共设施
树木、石头和装饰物
```

例如一个住宅地块放不下建筑时，不应立即修改主路；应该按以下顺序处理：

```text
换建筑模板
→ 旋转建筑
→ 调整地块退界
→ 尝试有限地基处理
→ 换地块
→ 放弃该建筑
```

只有道路、桥梁等高层结构确实无法满足连接目标时，才允许回退到道路规划阶段。

### 19.4 变化影响和失效传播

每个阶段的输出都应该声明会影响哪些下游数据：

```rust
pub struct GenerationArtifact {
    pub name: String,
    pub version: u64,
    pub invalidates: Vec<ArtifactKind>,
}
```

例如：

| 修改内容 | 必须重新计算 |
|---|---|
| 主路路线变化 | 道路走廊、街区、地块、建筑、设施、植被 |
| 道路 cut/fill 变化 | 最终地形、坡度、水文、地块、建筑 |
| 地块边界变化 | 建筑、入口、设施、植被 |
| 建筑位置变化 | 入口、人行道、设施、周边植被 |
| 树木位置变化 | 通常只影响局部验证 |

这意味着道路不能生成完后随意“补一刀”修改高度图。任何地形工程修改都必须使受影响的下游对象失效并重生成，或者通过事务回滚。

### 19.5 编排器接口示例

```rust
pub trait GenerationStage {
    fn name(&self) -> &'static str;
    fn dependencies(&self) -> &'static [ArtifactKind];
    fn run(&self, ctx: &mut GenerationContext) -> Result<StageOutput, StageError>;
}

pub struct GenerationOrchestrator {
    stages: Vec<Box<dyn GenerationStage>>,
}

impl GenerationOrchestrator {
    pub fn run(&mut self, ctx: &mut GenerationContext) -> Result<GenerationReport, StageError> {
        // 根据依赖关系排序，执行阶段，记录版本和失效范围
        // 失败时执行 retry / fallback / rollback
        todo!()
    }
}
```

初期不需要做复杂的通用工作流引擎，只要做到：

```text
阶段有明确输入和输出
阶段顺序固定且可追踪
阶段失败可记录
下游失效可重新生成
所有结果可用 seed 复现
```

---

## 20. 现代工程化地形：道路不必永远绕山

### 20.1 “沿地形”应该是偏好，不应该是绝对规则

原方案中“道路沿等高线”只能作为默认偏好，不能作为硬约束。

现代道路可能采用：

```text
顺应地形
挖方
填方
削坡
挡土墙
桥梁
涵洞
隧道
```

正确的问题不是：

```text
道路能不能穿过山？
```

而是：

```text
当前道路等级和工程预算，是否允许以某种工程方式穿过山？
```

### 20.2 地形区域应有不同的可修改等级

建议将地形和区域分为三类：

#### 硬禁建/硬禁改区域

除非生成特殊基础设施，否则不能修改：

```text
深水区
保护区
极端不稳定地质区
不可穿越的危险区
地图边界外
```

桥梁或隧道可以通过显式例外规则进入部分区域，但必须生成对应结构，不能直接把道路贴在水面上。

#### 可工程改造区域

可以在预算和阈值内修改：

```text
普通山坡
低矮丘陵
裸土地
普通农田
浅沟和小溪
```

#### 可自由调整区域

可以进行较小规模的平整和填挖：

```text
城市建设区
道路用地
建筑地块
停车场
工业用地
```

### 20.3 工程能力应由道路等级决定

不同道路不能使用同一套地形策略：

| 道路类型 | 默认策略 | 工程能力 |
|---|---|---|
| 高速/快速路 | 保持连续和较小坡度 | 允许较大挖填、桥梁和隧道 |
| 主干道 | 连接城市核心和重要区域 | 允许中等挖填、桥梁和挡土墙 |
| 次干道 | 服务街区 | 尽量顺应地形，可有限挖填 |
| 支路/住宅路 | 服务地块入口 | 优先绕行，不值得大规模工程 |
| 土路/林道 | 低成本连接 | 基本沿地形，尽量不改造 |

因此，道路描述中应增加工程策略：

```toml
[engineering]
can_cut = true
max_cut_depth = 18.0
can_fill = true
max_fill_height = 8.0
can_bridge = true
can_tunnel = true
max_grade = 0.07
construction_budget = 100.0
environmental_cost_limit = 40.0
```

### 20.4 道路路线应比较多个方案

对于两个城市核心之间的连接，不要只生成一条路线。至少应比较：

```text
方案 A：沿山谷和等高线绕行
方案 B：直接挖山
方案 C：修建隧道
方案 D：桥梁跨越沟谷或河流
```

每个方案都计算成本：

```text
总成本 = 路程成本
       + 坡度惩罚
       + 挖方成本
       + 填方成本
       + 桥梁成本
       + 隧道成本
       + 洪水/地质风险成本
       + 生态或保护区成本
```

路线选择可以使用：

```rust
pub struct RouteOption {
    pub path: Polyline2,
    pub road_kind: RoadKind,
    pub cut_volume: f32,
    pub fill_volume: f32,
    pub bridge_length: f32,
    pub tunnel_length: f32,
    pub construction_cost: f32,
    pub hazard_cost: f32,
    pub score: f32,
}
```

选择最低成本的合法方案，而不是简单地选择最短路线或完全绕开山体。

### 20.5 地形工程必须产生可验证的结果

道路挖山之后，必须生成工程记录：

```rust
pub enum TerrainEdit {
    Cut { area: Polygon2, depth: f32 },
    Fill { area: Polygon2, height: f32 },
    Flatten { area: Polygon2, target_height: f32 },
    RetainingWall { line: Polyline2, height: f32 },
    Bridge { span: Polyline2 },
    Tunnel { entrance_a: Vec3, entrance_b: Vec3 },
}
```

并且验证：

```text
道路表面连续
道路坡度不超限
挖方深度不超限
填方高度不超限
边坡有稳定处理
桥梁两端接路
隧道两端接路
水流没有被无故截断
道路工程没有让已有建筑悬空
```

### 20.6 现代化不等于无限制改造

如果所有道路都可以无限挖山，地图会变得不自然，也会失去地形意义。因此工程能力还应受到以下因素影响：

```text
道路等级
城市规模
交通重要性
技术水平
建设预算
环境保护程度
地质稳定性
随机世界设定
```

可以为世界设置参数：

```toml
[world.infrastructure]
technology_level = 0.8
construction_budget = 0.7
environmental_protection = 0.5
mountain_engineering = 0.6
```

同一座山在不同世界设定中可能产生不同结果：

```text
低技术、低预算世界 → 道路绕山
现代中等预算世界 → 局部挖方和挡土墙
高技术、高预算世界 → 隧道、桥梁和大规模削坡
```

---

## 21. 编排和工程化地形的验收标准

新增以下验证项：

```text
所有生成阶段的依赖关系无环
道路工程发生后，下游地块和建筑使用的是新地形版本
道路方案符合道路等级和工程预算
道路没有无理由穿越硬禁建区
允许穿越山体时，存在对应的挖方、隧道或桥梁记录
挖方、填方、桥梁和隧道参数没有超过上限
水体跨越使用桥梁、涵洞或合法道路结构
任何下游对象都不会引用已经失效的地形版本
```

最低要求不是“道路永远避开山”，而是：

> **道路可以改造地形，但每次改造必须有原因、有能力限制、有成本记录，并触发受影响对象的重新验证。**

---

## 22. 更新后的总体决策

最终采用以下策略：

```text
先规划需求和连接目标
→ 生成多条道路候选路线
→ 根据道路等级和工程能力选择方案
→ 提交挖方、填方、桥梁或隧道
→ 生成工程后的最终地形
→ 在最终地形上生成街区、地块和建筑
→ 生成设施和自然物体
→ 全局验证，必要时回退到最近的上游阶段
```

所以，生成顺序不是简单的“自然物体先后表”，而是：

```text
依赖关系
+ 优先级
+ 工程预算
+ 地形可修改性
+ 失败回退
+ 失效传播
```

这才适合现代世界随机地图，而不是只适合“物体不能改变地形”的静态场景。
