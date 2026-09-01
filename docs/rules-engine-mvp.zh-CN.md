# Rust 规则引擎 MVP 实现文档（待审查）

> 版本：v0.3 实现稿（2026-09-01）
> 关联方案：`docs/urban-natural-generation-plan.zh-CN.md` 第 17 节 MVP
> 本文档描述**要实现的代码结构、接口、算法和测试**，供其他 agent 审查。
> 审查重点见文末「审查清单」。

---

## 1. 目标与范围（MVP，不做全量）

本轮只实现方案第 17 节的 MVP：

```text
1. TOML 对象描述
2. Rust serde 加载 + schema 校验
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

第一批对象：**道路、普通住宅、树木、石头、小卖部**。
验收场景：房子在水里/陡坡/压路 → 拒绝；树在小卖部门口 → 拒绝；石头在路上 → 拒绝；合法位置 → 放置。

**明确不做**（本轮）：街区/地块切分、张量场道路、工程预算、失效传播、季节化、渲染。

---

## 2. 目录结构与模块

```text
bevy/crates/core/src/rules/
  mod.rs          # pub mod 汇总 + 公共 re-export
  schema.rs       # ObjectDescriptor / GeometrySpec / EnvironmentSpec / RelationSpec 等强类型
  loader.rs       # 读 *.object.toml → serde → schema 校验 → ObjectDescriptor
  registry.rs     # ObjectRegistry：id → descriptor 索引 + 资源路径校验
  constraint.rs   # Constraint 枚举 + 从 descriptor 编译成约束列表
  validator.rs    # validate_candidate(item, candidate, ctx) -> Result<(), RejectReason>
  placement.rs    # place(descriptor, candidates, ctx) 统一放置流程（硬约束→软评分→commit）
  errors.rs       # RuleLoadError / PlacementError / RejectReason

assets/objects/
  tree.object.toml
  rock.object.toml
  building.object.toml
  storefront.object.toml
  road.object.toml        # 道路走专用生成器，但 MVP 先给一个占位描述（见 §9）
```

---

## 3. 数据结构（schema.rs）

### 3.1 顶层描述

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct ObjectDescriptor {
    pub id: String,               // 唯一 id，如 "tree_cedar"
    pub kind: ItemKind,           // building / tree / rock / storefront / road ...
    #[serde(default)]
    pub tags: Vec<String>,        // 语义标签：storefront, public_access, flammable ...
    #[serde(default)]
    pub asset: String,            // 资源路径（MVP 可空）
    pub geometry: GeometrySpec,
    pub environment: EnvironmentSpec,
    #[serde(default)]
    pub relations: RelationSpec,
    #[serde(default)]
    pub anchors: Vec<AnchorSpec>,
    pub placement: PlacementSpec,
}
```

### 3.2 几何

```rust
pub struct GeometrySpec {
    pub footprint: [f32; 2],      // [width, depth]，米
    pub height: f32,              // 三维碰撞高度
    #[serde(default = "default_clearance")]
    pub clearance: f32,           // 维护/安全间距（叠在 footprint 外）
    #[serde(default = "default_pivot")]
    pub pivot: Pivot,             // center / bottom
    #[serde(default)]
    pub rotations: RotationMode,  // any / none / align_to_target
}
```

`clearance` 的作用：实际占用区 = footprint 向外扩 clearance，用于碰撞检测。入口锚点另有自己的 `clear_radius`。

### 3.3 环境

```rust
pub struct EnvironmentSpec {
    pub on_ground: bool,          // 底部必须贴地（四角高度差校验）
    pub not_in_water: bool,       // footprint 不得进入水体/洪泛区
    pub max_slope: f32,           // 坡度上限（米/100m 或比率，见 §6.1）
    #[serde(default)]
    pub allowed_biomes: Vec<Biome>,       // 空 = 不限
    #[serde(default)]
    pub forbidden_hazards: Vec<HazardKind>, // cliff, floodplain ...
}
```

### 3.4 关系

```rust
pub struct RelationSpec {
    #[serde(default)]
    pub avoid: Vec<RelationAvoid>,    // 硬约束：避开某类实体，间距 distance
    #[serde(default)]
    pub require: Vec<RelationNear>,   // 硬约束：必须附近有某类实体
    #[serde(default)]
    pub prefer: Vec<RelationNear>,    // 软偏好：评分用，不拒绝
}

pub struct RelationAvoid { pub kind: ItemKind, #[serde(default)] pub distance: f32 }
pub struct RelationNear  { pub kind: ItemKind, pub distance: f32 }
```

### 3.5 锚点（入口/服务口）

```rust
pub struct AnchorSpec {
    pub id: String,               // "front" / "service"
    #[serde(default)] pub side: String,   // front / back / left / right（相对模型朝向）
    #[serde(default)] pub kind: AnchorKind, // public_access / maintenance
    #[serde(default)] pub clear_radius: f32,  // 该锚点外清空半径（门前净空）
    #[serde(default)] pub must_face: Option<String>, // 必须朝向的实体类型/标签
}
```

### 3.6 放置策略

```rust
pub struct PlacementSpec {
    pub phase: PlacementPhase,    // 见方案 §2.3：Road/Lot/Building/Functional/Vegetation...
    #[serde(default = "default_priority")] pub priority: u32, // 同阶段内，越小越优先
    #[serde(default = "default_attempts")] pub attempts: u32, // 候选尝试上限
    #[serde(default)] pub allow_rotate: bool,
    #[serde(default)] pub allow_scale: bool,
    #[serde(default = "default_fallback")] pub fallback: Fallback, // skip / move / shrink
}
```

---

## 4. 环境查询与空间索引（ctx 由现有 worldgen 提供）

MVP 复用现有 `worldgen.rs` 的基础设施，不新建大结构：

```rust
pub struct PlacementContext<'a> {
    pub height_at: &'a dyn Fn(i32, i32) -> i16,          // 复用 terrain_height
    pub water_level: f32,                                // 复用 WaterGeometry.surface
    pub slope_at: &'a dyn Fn(i32, i32) -> f32,           // 复用 local_slope
    pub occupancy: &'a OccupancyGrid,                    // 复用现有 10m 网格
    pub bounds: (i32, i32, i32, i32),                    // 地图边界 (min_x, min_z, max_x, max_z)
}
```

`OccupancyGrid`（现有，10m 单元）承担空间索引角色：
- `mark(x, z, half_w, half_d, layer)` — Hard(道路/建筑) / Soft(植被)
- `collides_hard(x, z, half_w, half_d) -> bool`

**局限声明**：10m 网格对 footprint < 10m 的物体精度不足。MVP 接受此精度（基准物体都 ≥10m 或近似），Phase B 再升级为几何索引（`query_shape`）。审查时请确认此取舍是否可接受。

---

## 5. Constraint 枚举（constraint.rs）

```rust
pub enum Constraint {
    InsideBounds,
    OnGround,                       // 需要 height 采样 4 角
    NotInWater,
    MaxSlope(f32),
    AvoidKind { kind: ItemKind, distance: f32 },
    AvoidTag { tag: String, distance: f32 },
    RequireNear { kind: ItemKind, distance: f32 },
    ClearAnchor { anchor: String, radius: f32 },  // 锚点周围清空
    PreferNear { kind: ItemKind, distance: f32, weight: f32 }, // 软
}
```

`from_descriptor(desc) -> Vec<Constraint>`：
- `environment` → OnGround / NotInWater / MaxSlope / (biome/hazard MVP 暂跳过，需 BiomeField 支持)
- `relations.avoid` → AvoidKind（或 AvoidTag，若 kind 是标签）
- `relations.require` → RequireNear
- `anchors` → ClearAnchor
- `relations.prefer` → PreferNear

---

## 6. 验证器（validator.rs）

### 6.1 坡度定义（与现有代码一致）

现有 `local_slope(seed, x, z, width_m, ...)` 返回 **米/100m**（如 12 = 12% 坡度）。MVP 的 `max_slope` 用**米/100m** 单位，与现有建筑检查一致。TOML 里写 `max_slope = 15`（15%）。

### 6.2 水面判定

复用 `footprint_intersects_water(entity, geometry, margin)`。`not_in_water` 时 margin = 0。

### 6.3 地面接触

`on_ground`：在 footprint 四角采样 `height_at`，四角高度差（相对中心）≤ `grounding_tolerance`（默认 0.5m，可配）。超过则：
- 若 `allow_scale` → 缩小 footprint 重试
- 否则拒绝（`NotGrounded`）

### 6.4 碰撞判定

`AvoidKind`：在候选位置的 footprint+clearance 范围内，查 `OccupancyGrid.collides_hard`（Hard 层含道路/建筑）。distance 通过把 footprint 向外扩 distance 实现。

### 6.5 入口净空

`ClearAnchor`：锚点位于 footprint 某边外侧，检查该半径圆内无 Hard 实体（树/石/灯）。`must_face`：锚点朝向的实体类型是否在邻接（先查 occupancy 相邻格子的标记）。

### 6.6 RejectReason

```rust
pub enum RejectReason {
    OutOfBounds,
    InWater,
    ForbiddenHazard,
    SlopeTooHigh { slope: f32, max: f32 },
    ReservationConflict,
    GeometryCollision { conflict_kind: ItemKind },
    MissingRequiredRelation { kind: ItemKind },
    BlockedEntrance { anchor: String },
    NotGrounded,
    DisconnectedAccess,
}
```

序列化为 JSON 时含：`item / candidate / reason / conflict_with / rule`。

---

## 7. 统一放置流程（placement.rs）

```text
place(desc, candidates, ctx):
  1. 遍历 candidates（按确定性顺序，seed 派生）
  2. 每个候选：计算 footprint/clearance/锚点/三维 bounds
  3. 硬约束检查（§6 全部）：任一失败 → 记 RejectReason，试下一个
  4. 通过硬约束的候选 → 软评分（PreferNear 加权和）
  5. 选最高分候选
  6. repair：若候选可被旋转/平移修正到合法，先尝试（attempts 内）
  7. commit：mark 到 OccupancyGrid + push 到 entities + 记录 reservation
  8. 全失败 → 返回 Reject 列表，不放置
```

确定性：候选顺序由 `hash(seed, candidate_index)` 打乱，同一 seed 结果稳定。

---

## 8. 事务式提交

MVP 的"事务"简化（不建 Transaction 结构体）：

```rust
pub struct PlacementOutcome {
    pub placed: Vec<EntityInstance>,       // 成功放置
    pub rejected: Vec<RejectRecord>,       // 拒绝记录（JSON 可读）
}
pub struct RejectRecord {
    pub item_id: String,
    pub candidate_x: i32,
    pub candidate_z: i32,
    pub reason: RejectReason,
    pub rule: String,
}
```

提交顺序保证：**先 mark 到 OccupancyGrid，再 push entities**，不会出现"实体在列表但索引没有"的半状态。

---

## 9. 基准对象 TOML（assets/objects/）

### 9.1 tree.object.toml

```toml
id = "tree_common"
kind = "tree"
tags = ["vegetation", "flammable"]
asset = "assets/vegetation/tree_common.glb"

[geometry]
footprint = [4.0, 4.0]      # 树冠直径 ~4m
height = 8.0
clearance = 0.5
pivot = "bottom"
rotations = "any"

[environment]
on_ground = true
not_in_water = true
max_slope = 30              # 树可稍陡
allowed_biomes = ["grassland", "forest", "urban_green"]

[[relations.avoid]]
kind = "building"
distance = 1.0

[[relations.avoid]]
kind = "storefront"
distance = 3.0              # 门前净空（防止挡门）

[[relations.avoid]]
kind = "road"
distance = 1.5

[[relations.avoid]]
kind = "railway"
distance = 15.0

[placement]
phase = "vegetation"
priority = 50
attempts = 12
allow_rotate = true
fallback = "skip"
```

### 9.2 building.object.toml

```toml
id = "residential_house"
kind = "building"
tags = ["residential", "has_entrance"]

[geometry]
footprint = [12.0, 10.0]
height = 7.0
clearance = 0.5

[environment]
on_ground = true
not_in_water = true
max_slope = 12

[[relations.avoid]]
kind = "building"
distance = 0.0              # 不重叠即可

[[relations.avoid]]
kind = "road"
distance = 0.0              # 可贴边，不压路

[[relations.avoid]]
kind = "water"
distance = 5.0

[[relations.avoid]]
kind = "railway"
distance = 20.0

[[anchors]]
id = "front"
side = "front"
kind = "public_access"
clear_radius = 3.0
must_face = "road"

[placement]
phase = "building"
priority = 30
attempts = 8
allow_rotate = true
fallback = "skip"
```

### 9.3 storefront.object.toml（小卖部）

```toml
id = "storefront"
kind = "storefront"
tags = ["commercial", "public_access", "has_entrance"]

[geometry]
footprint = [8.0, 6.0]
height = 4.0
clearance = 0.5

[environment]
on_ground = true
not_in_water = true
max_slope = 8

[[relations.avoid]]
kind = "building"
distance = 0.0

[[relations.avoid]]
kind = "railway"
distance = 25.0

[[relations.require]]
kind = "road"
distance = 8.0

[[anchors]]
id = "front"
side = "front"
kind = "public_access"
clear_radius = 3.0          # 门前 3m 净空
must_face = "road"

[[relations.prefer]]
kind = "storefront"
distance = 20.0
weight = 0.4                # 喜欢扎堆成街

[placement]
phase = "functional"
priority = 40
attempts = 10
allow_rotate = true
fallback = "skip"
```

### 9.4 rock.object.toml（石头）

```toml
id = "rock_round"
kind = "rock"
tags = ["decoration"]

[geometry]
footprint = [2.0, 2.0]
height = 1.2
clearance = 0.2

[environment]
on_ground = true
not_in_water = true
max_slope = 40

[[relations.avoid]]
kind = "road"
distance = 1.0              # 石头不上马路

[[relations.avoid]]
kind = "building"
distance = 0.5

[[relations.avoid]]
kind = "storefront"
distance = 2.0

[placement]
phase = "vegetation"
priority = 60
attempts = 8
allow_rotate = true
fallback = "skip"
```

### 9.5 road.object.toml（占位，走专用生成器）

```toml
# 道路由 RoadGenerator 专用生成（改变地形 + 连接图），
# 此文件仅声明其 footprint/约束供验证器复用。
id = "road"
kind = "road"

[geometry]
footprint = [8.0, 8.0]      # 路面宽 ~8m
height = 0.5
clearance = 2.0             # 道路安全区（路肩 + 净空）

[environment]
on_ground = true
max_slope = 8

[placement]
phase = "road"
priority = 10
fallback = "skip"
```

---

## 10. 与现有 worldgen 的集成点（不破坏现有管线）

```text
现有 generate_entities_with_profile(seed, scene, landmarks, geometry, style)
  产生 entities（含随机放置）
        ↓
[新] rules::validate_all(&entities, &ctx)     ← 独立验证通道
        ↓
输出 ValidationReport（JSON）：拒绝项、原因、统计
        ↓
现有代码先不改，report 用于诊断；规则引擎稳定后，
再让 generate_* 直接调 place() 替代随机放置。
```

这样第一阶段是"审计模式"：**不阻止现有生成，只报告违规**。第二阶段才切换成"规则模式"：由规则引擎主导放置。

---

## 11. 测试计划

### 11.1 单元测试（rules crate）

| 测试 | 场景 | 期望 |
|------|------|------|
| `tree_avoid_building` | 树候选落在建筑 footprint 内 | Reject(GeometryCollision) |
| `tree_blocked_by_storefront` | 树在小卖部 front 锚点净空区 | Reject(BlockedEntrance) |
| `rock_on_road` | 石头在 road 上 | Reject(GeometryCollision) |
| `building_in_water` | 建筑 footprint 进水体 | Reject(InWater) |
| `building_on_steep_slope` | 坡度 > max_slope | Reject(SlopeTooHigh) |
| `building_overlaps_road` | 建筑压路 | Reject(GeometryCollision) |
| `valid_position_placed` | 合法候选 | 放置成功 |
| `no_valid_position_skipped` | 全候选非法 | 跳过 + 记录原因 |

### 11.2 固定 seed 回归

```bash
cargo run -p glyphweave -- generate --seed 12345 --validation-report report.json
```

报告必须满足（MVP 验收）：
```json
{ "in_water": 0, "not_grounded": 0, "geometry_collisions": 0, "blocked_entrances": 0 }
```

### 11.3 压力测试（多 seed）

```bash
cargo test -p glyphweave-core -- rules  # 内部属性测试（proptest 可选，先用手动 seed 列表）
```
至少覆盖：平地 / 丘陵 / 山区 / 河流穿越 / 湖边 / 高密度 / 极端 seed。

---

## 12. 审查清单（给审查 agent）

请重点检查以下设计决策：

1. **复用 OccupancyGrid（10m 网格）作空间索引**是否可接受？footprint < 10m 精度不足。
2. **坡度单位**：用现有 `local_slope` 的"米/100m"（如 12=12%），TOML 写 `max_slope=12`。是否一致、清晰？
3. **Constraint 枚举**是否覆盖 MVP 场景？`AvoidTag` 是否需要 MVP 实现（还是先只 `AvoidKind`）？
4. **OnGround 容差**：默认 0.5m 是否合理？四角采样 vs 网格采样？
5. **放置顺序**：`phase` + `priority`（越小越先）。道路(10) → 建筑(30) → 小卖部(40) → 树(50) → 石(60)。是否合理？
6. **审计模式先行**（先跑 report 不改生成）是否稳妥？
7. **事务简化**：先 mark 再 push，无独立 Transaction 结构——MVP 可接受吗？
8. `fallback = skip`（不放）是默认，`move/shrink` 留 Phase B——OK？

---

## 13. 文件与代码位置

- 实现目录：`bevy/crates/core/src/rules/`（schema/loader/registry/constraint/validator/placement/errors/mod）
- 对象描述：`assets/objects/*.object.toml`（仓库根 assets 或 bevy/assets，待定——建议 `assets/objects/` 与方案一致）
- 集成点：`bevy/crates/core/src/worldgen.rs` 的 `generate_entities_with_profile`（调用 `rules::validate_all`）
- CLI：`bevy/crates/cli/src/main.rs` 增加 `rules validate <path>` 子命令
- 本方案的依据：`docs/urban-natural-generation-plan.zh-CN.md` 第 5/6/7/16/17 节
