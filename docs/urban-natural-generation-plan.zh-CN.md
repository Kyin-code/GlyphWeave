# 现代城市自然生成方案（调研 + 架构设计）

> 版本：v0.1 草案（2026-09-01）
> 目标：让 GlyphWeave 从"巨型地标 + 粗糙布局"进化到"精细化、合理、自然"的现代城市生成。
> 本文基于对 CityEngine / Parish-Müller / GDMC 竞赛 / 多篇 SIGGRAPH 论文 / Cities:Skylines 验证系统的调研。

---

## 1. 现状诊断（为什么 GIS 路线不够）

| 问题 | 根因 |
|------|------|
| 门被树挡住 | 树与建筑是**独立随机放置**，无"互相感知" |
| 路从房穿过 | 建筑先放、路后放，或路生成不感知已有建筑 |
| 房在水里 / 悬空 / 碰墙 | 放置时没有查地形高度、水面、邻居实体 |
| 巨型建筑可以、精细不行 | GIS 适合定锚大物（楼/桥/公园），但**没有地块/街区的概念** |

**核心结论**：我们缺的不是"更多物品"，而是**两个东西**：
1. **一个分层的生成顺序**（宏观规划 → 微观填充）
2. **一套每类物品自己的放置规则**（约束验证，而不是事后修 bug）

---

## 2. 调研收获：可借鉴的成熟方法

### 2.1 道路生成（三条成熟路线）

| 方法 | 出处 | 核心思想 | 适合我们 |
|------|------|---------|---------|
| **自敏感 L-system** | Parish & Müller 2001（CityEngine 前身） | 主路/次路分级生长，新路感知已存在的路：snap 到交点、避开障碍、沿等高线 | ✅ 主路骨架 |
| **张量场流线** | symbios-tensor (GitHub) | 主路沿**地形等高线**、次路沿**梯度**，RK2 积分出有机网格 | ✅ 山区地形 |
| **交通模拟驱动** | Benes 2014 (捷克) | 用相邻城镇间交通量决定何时修主路，主路交叉口 = 城市生长核心 | 🔄 远期 |

**关键洞察（跟 Kyin 的想法完全一致）**：道路**必须感知地形**。
- `symbios-tensor` 明确做了 `carve_roads`（把路下的地形压平 + 边坡融合）——**这就是"修路后回头调整地形"**！
- CityEngine 有 `adapt to elevation`：主路沿等高线走，保证坡度 ≤ 临界值。

### 2.2 建筑放置（约束系统）

| 方法 | 出处 | 核心思想 |
|------|------|---------|
| **约束建筑选择** | Scharl 2010 (CESCG) | 每个地块选"最合适"的建筑：占地不超地块、**有门的边必须临街**、无门的边不能临街、坡度超限则弃地 |
| **GDMC 去中心迭代规划** | 2022 竞赛冠军 | 建筑逐个放置，每次决策都重新评估全局 → 适应随机地形 |
| **Cities:Skylines 验证系统** | 游戏引擎 | `ValidationHelpers`：查碰撞（quadtree）、查水面（InWater）、查地形（Floating/OnGround/Shoreline）、查边界 |

### 2.3 宏观↔微观配合（层级生成）

| 方法 | 出处 | 层级 |
|------|------|------|
| **三尺度** | "semantically plausible small-scale towns" (ScienceDirect 2023) | world 尺度(200m 采样) → town 尺度(5-10m) → human 尺度(房间门窗) |
| **街区→地块→建筑** | CityEngine 标准管线 | 路围成**街区(block)** → 街区切成**地块(lot)** → 地块上生成建筑 |
| **有机村落生长** | "Procedural villages" (LIRIS 2012) | 兴趣地图(interest maps) 渐进生成：路吸引定居者，房又扩展路，交替迭代 |

### 2.4 每类物品的放置规则（Kyin 的插件思路——找到了成熟范本）

**`Meep` 引擎（GitHub）** 的架构和我们想要的一模一样：
- 每个物品 = 一个 `MarkerNode`（带 type/tags/position/size/priority）
- 放置 = 一组 `Rule`（matcher 匹配地形/邻居 → action 决定放不放、放哪）
- 规则可以**读取前一轮写好的数据层**（高度图、biome、障碍层）→ 天然支持"道路先规划，树后避让"
- 支持 `overlap rejection`（重叠拒绝）、`priority`（重要物体先放）

**CityEngine CGA shape grammar** 也是：每个建筑样式一个 `.cga` 规则文件，规则描述"怎么把地块变成立体"。

---

## 3. 总体架构方案

### 3.1 生成顺序（宏观 → 微观，道路感知地形，建筑感知道路）

```
Stage 0: 自然地形（已有：ridge 骨架 + 水文 + biome）
         ↓
Stage 1: 宏观规划（新）
   - 用兴趣地图/张量场 决定"哪里能住人"（坡度、水面、植被、耕地）
   - 选择城市核心/聚落种子（临近水源、平地、交通要道）
   - 生成主路骨架：沿等高线 / 连接聚落种子（L-system 或张量场）
         ↓
Stage 2: 微观规划（新）
   - 主路围出街区(block)
   - 街区按坡度/面积切成地块(lot)，太陡的弃掉
   - 每个地块选建筑类型（靠商业核心=商业，边缘=住宅，滨水=特殊）
         ↓
Stage 3: 建筑生成（已有 CGA 引擎增强）
   - 每个地块用 CGA 规则生成建筑
   - 建筑必须：底面贴合地形(applySlopeFoundation已有) + 临街面开门 + 不越地块
         ↓
Stage 4: 配套填充（新——核心是"每类物品自己的规则"）
   - 树木/岩石/路灯/小卖部/公园，每个一个"物品规则插件"
   - 按规则查：碰撞、距离、地形、邻居 → 决定放不放
         ↓
Stage 5: 反向修正地形（新——修路/修地概念）
   - 路下的地形压平 + 边坡融合（symbios-tensor 的 carve_roads）
   - 建筑地基压平（已有 applySlopeFoundation）
   - 桥墩处抬高/引桥（已有部分）
```

### 3.2 物品规则插件系统（Kyin 的核心诉求）

每个物品一个文件夹/文件，描述它的"放置规则"。放不下就不放（绝不硬塞）。

```js
// rules/storefront.js   —— 小卖部
export default {
  kind: 'storefront',
  // 1. 物理/碰撞（基本要求）——不满足直接不放
  physics: {
    onGround: true,          // 必须贴地，不能悬空
    notInWater: true,        // 不能在水里
    slopeMax: 0.15,          // 地面坡度上限
    clearance: {              // 需要留出的空当
      front: 3,              // 正门前 3m 不放任何东西
      sides: 1,              // 两侧各 1m
    },
    avoid: [                 // 必须避开的实体
      { kind: 'road', pad: 1 },      // 不压马路
      { kind: 'building', pad: 0 },  // 不跟建筑重叠
      { kind: 'water', pad: 2 },     // 离水 2m
      { kind: 'tree', pad: 0.5 },    // 树不插门里
    ],
  },
  // 2. 应用要求（功能/语义）
  functional: {
    needsFrontOn: 'road',    // 正门必须临街（Street Access Side）
    noBlock: [               // 门前禁止出现的东西
      'tree', 'rock', 'lamp', 'bus_stop',
    ],
    nearLike: [              // 喜欢挨着
      { kind: 'road', within: 8 },
    ],
  },
  // 3. 季节/环境变化（可选）
  season: { summer: {}, winter: {} },
}
```

**放置算法**（每类物品共用同一套判定流程）：
1. 候选位置 = 满足 `physics` 所有硬性检查的位置（不满足就跳过，**宁缺毋滥**）
2. 从候选里按 `functional` 打分排序（有门临街的优先）
3. 放置后**登记进空间索引**（quadtree/网格），后续物品可查
4. 每个物品有一个 `priority`，高优先级的（路、桥、核心建筑）先放

### 3.3 关键数据结构

```
Block（街区）    { polygon, roadEdges[], avgSlope, area, neighbors }
Lot（地块）      { polygon, streetSides[], setbackFront/Side/Rear, area, ownerKind }
PlacedItem       { kind, worldX/Z, footprint, frontDir, priority, tags }
SpatialIndex     { quadtree / grid map, queryRect(), queryKind() }
EnvironmentLayer { height, water, biome, obstacle, roadMask, population }  ← 多层可叠加
```

---

## 4. 实施路线图（建议顺序）

### Phase A：放置验证框架（最快见效，解决"屡屡犯错"）
1. 建 `SpatialIndex`（quadtree 或 512m chunk 网格，查询已有实体）
2. 建 `EnvironmentLayer`（height/water/biome/roadMask 可查询）
3. 建规则 schema + 判定器
4. **把现有每类物品接上规则**（树/建筑/岩石/路灯/小卖部/桥）——先解决：门前有树、路压房、房在水里、房悬空

### Phase B：街区/地块规划（精细化关键）
1. 道路网 → 提取街区（左转最小角遍历出有界面，参照 symbios-tensor `extract_blocks`）
2. 街区 → 地块（递归沿最长边切分，参照 CityEngine `Block Subdivision`）
3. 地块 → 建筑（约束选择：街面访问面 / 坡度 / 面积）
4. 反向修地形：`carve_roads` + `carve_lots`

### Phase C：宏观规划（自然性关键）
1. 兴趣地图（坡度/水源/植被/交通 → "宜居度"热力）
2. 聚落种子选择（兴趣地图卷积选核心）
3. 主路骨架（张量场 或 L-system，沿等高线 + 连接聚落）

### Phase D：配套填充（丰富度关键）
1. 树木/灌木：Poisson disk 撒点 + 规则避让（门前、路上、水里不种）
2. 小卖部/公园/路灯：按 `functional` 规则（临街、门前留空）
3. 季节化：复用 presets 系统

---

## 5. 参考资源清单

| 资源 | 内容 | 地址 |
|------|------|------|
| Parish & Müller 2001 | 自敏感 L-system 城市生成 | SIGGRAPH 2001 |
| symbios-tensor | 张量场道路 + 地块 + 地形雕刻（Rust，最贴近我们） | github.com/TheJanusStream/symbios-tensor |
| CityEngine docs | 道路生长/街区/地块/CGA 规则 | doc.arcgis.com |
| GDMC 2022 冠军 | 去中心迭代规划（适应随机地形） | arxiv 2309.10871 |
| procedural villages | 兴趣地图渐进村落 | perso.liris.cnrs.fr 2012 |
| Meep 引擎 | Marker + Rule 放置系统（我们的插件范本） | meep.company-named.com |
| Cities:Skylines 验证 | quadtree 碰撞/水面/地形验证 | ps1ke.github.io |
| Ogun | 潜在博弈 + logit 动态放置 | docs.rs/crate/ogun |
| 三尺度小镇 | world/town/human 层级 | ScienceDirect 2023 |

---

## 6. 与现有代码的关系

- **复用**：`makeMCTerrain`/CGA 建筑引擎/`applySlopeFoundation`/`WaterGeometry`/presets 系统
- **新增目录建议**：
  ```
  bevy/crates/core/src/planning/      ← 街区/地块/宏观规划（Rust）
  bevy/crates/core/src/rules/         ← 物品规则插件（Rust 或 JSON 数据）
  schemas/placement-rule.schema.json  ← 规则 schema
  ```
- 规则优先用**数据驱动**（JSON/JS 对象），方便 Kyin 调参、不用改代码

---

## 7. 待 Kyin 决策的点

1. **生成顺序确认**：走"地形→宏观→微观→配套→反向修地"五阶段？还是先只做 Phase A（验证框架）解燃眉之急？
2. **规则放哪**：Rust（生成器内）还是 JSON 数据文件（生成器读取）？建议后者（可调不改代码）。
3. **要不要参考 symbios-tensor**：它和我们的目标高度重合（Rust 生成 + 地形雕刻），可 clone 细读。

---

## 8. 物品规则插件完整示例（对应我们犯过的错）

把 Kyin 提到的每个错误场景 → 一条规则。这是 Phase A 的实现范本。

### 8.1 已犯错误 → 规则映射

| 错误场景 | 归属 | 规则怎么写 |
|---------|------|-----------|
| 门口有树 | tree | `avoid: [{kind:'storefront', pad:3, at:'front'}]` + `notIn.frontOf: ['storefront']` |
| 道路上有房 | building | `avoid: [{kind:'road', pad:0}]` + `needsFrontOn: 'road'`（只许贴边，不许压路） |
| 水里有房 | building | `physics.notInWater: true`（放置前查水面高度 vs 地形高度） |
| 房子悬空 | building | `physics.onGround: true`（四角高度差 < 阈值，否则下移或弃置） |
| 房子碰墙/重叠 | building | `avoid: [{kind:'building', pad:0}]` + 空间索引重叠检查 |
| 铁轨边有建筑/树 | 建筑+树 | `avoid: [{kind:'railway', pad:20}]`（安全距离） |
| 小卖部正门放东西 | storefront | `functional.frontClearZone: 3m` |

### 8.2 完整规则示例（tree + building + storefront）

```js
// rules/tree.js
export default {
  kind: 'tree',
  priority: 40,                 // 低优先级：路/楼/小卖部先放，树后填
  physics: {
    onGround: true,
    notInWater: true,
    slopeMax: 0.35,             // 树可以在陡一点的地方，但不能悬崖
    avoid: [
      { kind: 'road', pad: 1.5 },      // 不种马路中间
      { kind: 'building', pad: 1 },    // 不贴墙（落叶/根）
      { kind: 'storefront', pad: 3 },  // 门前留空 3m
      { kind: 'railway', pad: 15 },    // 铁路安全距离
      { kind: 'water', pad: 1 },       // 河岸可种，但不下水
      { kind: 'tree', pad: 2 },        // 树间距
    ],
  },
  functional: {
    prefers: [                         // 打分：喜欢种哪
      { kind: 'park', within: 30 },
      { kind: 'green_space', within: 40 },
    ],
  },
}
```

```js
// rules/building.js
export default {
  kind: 'building',
  priority: 30,                 // 在路之后，在树之前
  physics: {
    onGround: true,             // 四角贴地（超差下移，仍超差弃置）
    notInWater: true,
    slopeMax: 0.2,              // 建筑坡度上限
    avoid: [
      { kind: 'road', pad: 0 },       // 不压路，但可以贴边
      { kind: 'building', pad: 0 },   // 不重叠
      { kind: 'water', pad: 5 },      // 滨水退 5m
      { kind: 'railway', pad: 20 },   // 铁路边不建房
    ],
  },
  functional: {
    needsFrontOn: 'road',       // 门必须临街（Street Access Side）
    minRoadAdjacent: 1,         // 至少 1 条边邻路
    frontClearZone: 3,          // 门前 3m 净空
  },
}
```

```js
// rules/storefront.js  —— 小卖部/沿街商铺
export default {
  kind: 'storefront',
  priority: 25,
  physics: {
    onGround: true,
    notInWater: true,
    slopeMax: 0.1,              // 商铺要更平
    avoid: [
      { kind: 'road', pad: 0 },
      { kind: 'building', pad: 0 },
      { kind: 'water', pad: 8 },
    ],
  },
  functional: {
    needsFrontOn: 'road',       // 正门必须临街
    frontClearZone: 3,          // 门前 3m 净空（树/灯/摊位都进不来）
    noBlock: ['tree', 'rock', 'lamp', 'bus_stop', 'food_stall'],
    prefersNeighbor: [          // 喜欢扎堆成市
      { kind: 'storefront', within: 20 },
      { kind: 'market', within: 60 },
    ],
    avoidNear: [
      { kind: 'industrial', within: 40 },   // 不挨重工业
      { kind: 'railway', within: 25 },
    ],
  },
}
```

### 8.3 放置判定流程（统一，所有物品共用）

```
place(item, candidatePositions):
  1. env = 读 EnvironmentLayer(pos)      // height/water/biome/slope
  2. for each rule in item.physics.avoid:
       if spatialIndex.queryKind(kind, pos, pad) 非空 → 拒绝
  3. if not item.physics.onGround 满足 → 拒绝
  4. if item.physics.notInWater and 地形低于水面 → 拒绝
  5. if slope > item.physics.slopeMax → 拒绝
  6. candidates = 通过硬性检查的位置
  7. if candidates 空 → 放弃（宁缺毋滥），记入 report
  8. 否则按 item.functional 打分，选最高分位置
  9. spatialIndex.add(item)             // 登记，供后续物品查
```

**关键保证**：
- 硬性检查（physics）**不满足就跳过**，绝不硬塞 → 杜绝"门被树挡"
- 功能性（functional）只影响**排序**，不影响合法性
- 优先级（priority）控制**放置顺序**：路 > 楼 > 小卖部 > 树，保证后来者避让先行者

