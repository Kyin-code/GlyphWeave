# GlyphWeave Agent 完整交接（2026-10-24）

状态：**当前 canonical Agent 交接稿**

基础代码基线：`refactor/authoritative-baked-audit` / `777c29f`

这份文档面向下一位接手实现的 Agent，不是产品宣传，也不是按日期追加的历史日志。
目标是让接手者在**不重读全部历史文档、不根据预览猜架构**的前提下，知道：

1. 项目为什么这样分层；
2. 当前真实生成链是什么；
3. 哪些历史里程碑仍然有效，哪些已被后续要求修正；
4. 当前硬规则在哪里、哪些已经完全阻断发布、哪些仍有缺口；
5. 下一位应从哪一项开始，哪些事绝不能先做；
6. 每一步应输出什么、怎样证明通过。

给项目负责人看的现状、数据、资源和硬规则清单在：

[`CAPABILITY-INVENTORY-AND-STAGED-GENERATION-20261023.zh-CN.md`](CAPABILITY-INVENTORY-AND-STAGED-GENERATION-20261023.zh-CN.md)

本文与该审核稿的分工：

| 文档 | 读者 | 用途 |
|---|---|---|
| 本文 | 下一位 Agent | 架构理解、实施顺序、工程边界、操作手册 |
| `CAPABILITY-INVENTORY...` | 项目负责人 | 现状清单、48 城数据、已确认规则、待审核选项 |
| `requirements-and-progress-master...` | 全体 | 需求与历史进度总账 |
| `HANDOVER.zh-CN.md` | 全体 | 旧版按主题/日期累积的交接日志 |
| `docs/acceptance/` | 检查者 | 历史验收证据 |

**冲突处理顺序：**

1. 当前用户明确指令；
2. 仓库 `AGENTS.md`；
3. 本文的当前架构与工作边界；
4. 用户审核稿中的硬规则和待确认项；
5. `requirements-and-progress-master` 的需求定义；
6. 旧 `HANDOVER` 和轮次记录中的历史事实。

历史文档中的数字只能作为证据，不能自动成为当前通过结论。

---

## 0. 接手第一天必须做的事

### 0.1 阅读顺序

按顺序阅读：

1. 本文全部；
2. `CAPABILITY-INVENTORY-AND-STAGED-GENERATION-20261023.zh-CN.md` 第 8 节“硬性规则”；
3. `requirements-and-progress-master.zh-CN.md` 第 2、3、5.5–5.14 节；
4. `bevy/crates/core/src/worldgen.rs` 中 `generate_entities_with_profile_report`；
5. `bevy/crates/core/src/worldgen/{route_evolution,settlement_evolution,road_network,frontage}.rs`；
6. `bevy/crates/core/src/worldgen/final_scene_audit.rs`；
7. `bevy/crates/core/src/worldgen/access.rs`；
8. `bevy/crates/core/src/rules/`；
9. 三城 Manifest 和 `THREE-CITY-SLICES-R249-R252-20261023.zh-CN.md`。

### 0.2 不要先做什么

未经审核确认，不要先做：

- 增加更多建筑外观；
- 继续调高建筑数量；
- 继续增加随机植被或圆形斑块；
- 在 HTML 里补生成 Rust 缺失的道路/建筑；
- 为了让测试通过而放宽碰撞、坡度、入口或水体门限；
- 把 48 城统计中位数写成硬建筑配额；
- 把旧 `frontageProgram.count` 重新作为普通城市生成目标。

当前最优先事项是：

```text
统一主生成器
→ 固定阶段依赖
→ 让每个阶段有冻结产物和失败闸门
→ 再提升自然、道路、聚落和视觉效果
```

### 0.3 工作树检查

当前工作树可能已存在与本次交接无关的未跟踪目录：

```text
bevy/target-*/
examples/modern-riverside-city-512.manifest.json
```

不要擅自删除、提交或覆盖。先确认归属。

---

## 1. 项目目标与产品边界

### 1.1 项目是什么

GlyphWeave 是一个模拟现代社会的地图生成项目：

- 以 Rust Core/CLI 为唯一地图真源；
- 输入是自然语言/结构化 Manifest；
- 输出是完整烘焙的 2D/2.5D 世界数据；
- HTML 是编辑、战略图、2.5D 近景和运行态验收适配器；
- Godot 4.7 是运行时适配器；
- Agent 只负责语义规划、数据合同和 Patch，不能替代 Rust 做几何/拓扑真值。

### 1.2 产品要表达的关系

地图不是“随机地形 + 随机房子”，而是：

```text
自然地理
→ 气候与水文
→ 山脊、谷地、岸线、湿地、可建地
→ 人类路径
→ 聚落
→ 时代叠加
→ 街区、地块、建筑
→ 人口、设施、市场
→ 工程改造自然
→ 生态与街景细节
```

自然决定第一层可行域；人类工程可以改变地形，但必须进入工程账本，并保留自然层与
最终层的差分。

### 1.3 已冻结输出边界

- 默认 `2.5d`，可选 `2d`；
- 当前不提供完整对外开放的 `3d` 生成模式；
- 1m × 1m 水平逻辑网格；
- 默认高度精度 0.25m；Manifest 可声明毫米编码；
- 512m 流式分块；
- 16m Storage Chunk；32×32 Storage Chunk 组成 Region；
- 单场景默认 1km × 1km，最大 6km × 10km；
- 同 seed、同 Manifest、同生成器版本必须可复现；
- 预览不得自行生成地图真值。

### 1.4 当前不做

以下能力曾被登记为 Future，当前不作为 A/B/C 里程碑实现目标：

- 完整人口经济模拟；
- 运行时动态 Agent 行为；
- 完整游戏经营循环；
- 商业资产直接复用；
- 开放式 LLM 驱动地图运行时；
- 完整 3D 生成模式。

`gameplay/` 中旧殖民游戏代码仍存在，但主线已转向现代世界生成。不要把旧 gameplay 状态机
当当前地图生成入口。

---

## 2. 当前真实状态

### 2.1 已有的底座

已具备：

- Manifest 校验与稳定 ID；
- 1m/0.25m Heightfield；
- `.gemap v3` 与 sidecar；
- 512m 流式分块；
- HTML 战略图/2.5D/探索；
- Godot sidecar adapter；
- 确定性重建与 worldRevision；
- scale/quality/final entity/access/rules 等多层审计；
- 最大 6×10km 历史闭环；
- 规则引擎与对象描述符。

### 2.2 已有的自然能力

已具备：

- `naturalGeography` 宏观参数；
- 多尺度噪声；
- 山脊/谷地近似；
- 海岸与湿度控制；
- 水体与岸线合同；
- 自然专用零人工痕模式；
- 32m 采样最陡下降排水图；
- 自然/最终地形差分和工程后排水对比；
- 挡土墙、涵洞等工程实体基础。

尚未完成：

- 构造地貌 → 汇水 → 河流 → 侵蚀沉积的完整自然链；
- 排水图驱动地形，而不是地形完成后再生成排水图；
- 山脊、谷地、冲积扇、阶地、三角洲、岩性的一等对象化；
- 生境场作为全城市生态的唯一上游；
- 水位、河床、护岸和最终工程的完全同源。

### 2.3 已有的道路能力

已具备：

- 自然地形上的路线搜索；
- 坡度、曲率、水体和桥成本；
- 8% 默认规划纵坡；
- 6m 默认填方；
- 分级道路横断面；
- 道路独立 OBB；
- `routeGrade` / road profile；
- 桥、高架、跨线实体；
- 道路 carve 和工程账本；
- 平地直线 / 地形拟合 alignment 选择。

尚未完成：

- 完整路口并集多边形；
- 转盘/渠化岛/互通模板；
- 隧道截面与洞门；
- 桥头挡墙、桥面排水、纵向碰撞插值；
- 统一判断“平地主干路必须直”的全局决策器。

### 2.4 已有的聚落能力

已具备：

- 需求驱动的建筑数量；
- 人口/岗位/零售/设施推导；
- 地块/建筑原型；
- FAR、层数和覆盖率；
- 旧城/更新/新区时代预算；
- 保留/修改/拆除/替换父对象；
- 学校、医院、教堂、社区/体育设施请求；
- 重规划权重；
- 入口、楼梯和后侧服务入口；
- 生态、树、灌木、草、岩石和街具。

尚未完成：

- 所有城市场景只有一条主生成链；
- 时代阶段枚举统一；
- 街区先于地块、地块先于建筑的完整 stage contract；
- 大型设施与市场统一保留机制；
- 中心建筑体量和覆盖率仍需提高；
- 城市边缘、旅游、放牧、种植、保护区尚未全部成为严格独立 profile。

### 2.5 已有的 48 城能力

已具备：

- 48 个核心切面先验；
- 48 个当前 1km² OSM 测量窗口；
- 81 个来源条目；
- 历史与土地压力先验；
- `referenceData.caseIds` 自动组合 style；
- `pnpm check:urban-cases`。

不等于：

- 48 座城市已经复刻；
- 48 套官方完整统计；
- 48 个世界已烘焙；
- 48 套视觉验收。

详细数据和逐案数字看审核稿第 4 节。

---

## 3. 仓库结构与职责

```text
GlyphWeave/
├── bevy/
│   ├── Cargo.toml
│   ├── crates/core/              # Rust 地图真源
│   └── crates/cli/               # 生成、审计、预览 CLI
├── adapters/html/                # Three.js 近景、战略图、渲染
├── server/                       # 静态预览/API 服务
├── src/                          # React/Konva 编辑器
├── assets/
│   ├── data/                     # 48 城与地球理先验
│   ├── objects/*.object.toml     # 规则对象合同
│   ├── third_party/              # 外部模型与许可证
│   └── glyphweave.registry.json  # assetId → 模型
├── examples/*.manifest.json      # 可复现输入矩阵
├── docs/
│   ├── collaboration/            # 需求、交接、审核
│   └── collaboration/rk-x/       # 当前实施证据
├── scripts/                      # 数据审计、镜像、复算
└── generated/ 或 bevy/work/      # 运行产物，通常不入库
```

### 3.1 Rust Core

核心文件：

| 文件 | 职责 |
|---|---|
| `bevy/crates/core/src/world.rs` | 实体/世界基础类型 |
| `bevy/crates/core/src/worldgen.rs` | Manifest 校验、场景生成总编排、高度场、实体材料化 |
| `bevy/crates/core/src/baked_terrain.rs` | 烘焙地形读取与完整性 |
| `bevy/crates/core/src/height_payload.rs` | 高度编码 |
| `bevy/crates/core/src/natural.rs` | 自然 baseline、32m 排水图、审计 |
| `bevy/crates/core/src/gemap.rs` / `storage/` | gemap v3 与分块 |
| `bevy/crates/core/src/rules/` | 描述符、约束、候选放置、审计 |

### 3.2 Worldgen 子模块

| 模块 | 职责 | 当前角色 |
|---|---|---|
| `worldgen/natural_geography.rs` | 自然地理参数派生 | 生产 |
| `worldgen/natural_baseline.rs` | 冻结自然基线 | 生产 |
| `worldgen/natural_only.rs` | 零人工痕 allow-list | 生产审计 |
| `worldgen/natural_geography.rs` | 地质/气候/水文的控制量 | 生产 |
| `worldgen/route_evolution.rs` | 自然地形路径与纵坡 | 生产 |
| `worldgen/settlement_evolution.rs` | 需求驱动聚落 | 生产 |
| `worldgen/epoch_evolution.rs` | 时代与拆改父链 | 生产 |
| `worldgen/planning_history.rs` | 网络中心、拓宽、公园 | 生产 |
| `worldgen/market_catchment.rs` | 市场容量/竞争/覆盖 | 生产 |
| `worldgen/engineering_scale.rs` | 宏观工程授权与预算 | 生产 |
| `worldgen/road_network.rs` | 旧 road-first 道路规划 | 并存风险 |
| `worldgen/frontage.rs` / `joint_frontage.rs` | 旧临街模板 | 并存风险 |
| `worldgen/auto_roads.rs` | 自动道路扩展 | 旧路径 |
| `worldgen/final_scene_audit.rs` | 最终 OBB/贴地/水体审计 | 生产 |
| `worldgen/access.rs` | 步行入口搜索 | 生产 |
| `worldgen/bounded_cut.rs` | 有界挖方账本 | 生产可选 |
| `worldgen/engineered_placement.rs` | 工程化放置 | 生产 |
| `worldgen/shore_band.rs` | 岸线公共带 | 生产 |

### 3.3 Settlement 子模块

| 模块 | 职责 |
|---|---|
| `settlement/building_archetypes.rs` | 建筑形态、尺寸、层数、覆盖率 |
| `settlement/morphology.rs` / `morphology_compile.rs` | 聚落形态参数编译 |
| `settlement/development.rs` | 道路开发与地块归属 |
| `settlement/landcover.rs` | 地表实现 |
| `settlement/agriculture.rs` | 农田/农牧地表 |
| `settlement/ecology.rs` | 生态实例和预算 |
| `settlement/ecology_habitat.rs` | 坡度/水体/高程生境接口 |
| `settlement/service_coverage.rs` | 服务范围 |
| `settlement/district_program.rs` | 时代/区域程序 |

### 3.4 Rules

`rules/` 是声明式约束层：

- `schema.rs`：资产/实体/环境/关系/锚点；
- `loader.rs`：TOML 严格读取；
- `constraint.rs`：编译为硬/软约束；
- `validator.rs`：执行约束；
- `placement.rs`：候选搜索、旋转、移动和事务提交；
- `audit.rs`：对成品世界重跑规则。

当前 22 个 `assets/objects/*.object.toml`，不是“只有 5 个对象”的早期 MVP 状态。

### 3.5 CLI

`bevy/crates/cli/src/main.rs` 负责：

- Manifest 校验；
- 世界生成；
- scale audit；
- quality report；
- world quality gate；
- rules validate/audit；
- preview;
- 其他诊断。

### 3.6 HTML/React

- `adapters/html/app.js`：加载 sidecar、战略图、近景/探索、相机；
- `adapters/html/render/terrain.js`：连续道路面、材质；
- `adapters/html/render/grass.js`：实例化草；
- `adapters/html/render/vegetation.js`：树/灌木/岩石；
- `adapters/html/render/water.js`：水面；
- `adapters/html/render/buildings.js`：建筑体量与立面；
- `adapters/html/render/ecology.js`：生态实例；
- `adapters/html/render/presets/materials.js`：材质和季节参数。

HTML 不能作为道路/建筑生成真源。可见几何必须服从 Rust 发布的 OBB、标高和 profile。

---

## 4. 架构分层与设计理由

### 4.1 为什么 Rust Core 是唯一真源

如果把地图生成放在 HTML：

- 浏览器刷新、窗口裁剪、LOD 会影响结果；
- Godot 与 HTML 会形成两套地图；
- 碰撞与预览无法同源；
- 生成不可确定性重建；
- Agent 难以验证最终事实。

所以：

```text
Manifest + seed
→ Rust 规划/验证/烘焙
→ sidecar/gemap
→ HTML/Godot 只读适配
```

### 4.2 为什么不是“全 3D”

- 512m–6×10km 规模下，逐格完整 3D 资产成本不可控；
- 2.5D 更适合战略图与近景共享同一高度场；
- 允许近景有可读建筑体量，远景用 LOD；
- 未来可扩展真实 3D，但当前不接受以 3D 视觉掩盖数据缺失。

### 4.3 为什么是 1m 高度场 + 0.25m 精度

- 城市入口、路缘、水岸和台阶需要米级位置；
- 0.25m 精度足够大多数地形表现；
- 工程和实体可另带毫米 offset；
- 不是每格都生成 GC node，而是数组 + 合并 Mesh。

### 4.4 为什么区分 512m、16m、Region

- 512m：流式加载和 LOD 边界；
- 16m：gemap 存储 Chunk；
- 32×32 16m Chunk：Region 聚合；
- 三者不是同一个“块”，混用会制造接缝、内存和索引错误。

### 4.5 为什么自然先于人类

- 山体、谷地、汇水、海岸和坡度决定道路与聚落可行域；
- 人类工程是自然之上的差分，不是直接随机覆盖；
- 先冻结自然基线，才能知道挖填、挡墙、填海、桥隧改变了什么；
- 生态、树、草、湿地必须消费地形/湿度/坡度/水体，而不是独立随机。

### 4.6 为什么人口/需求先于建筑数量

建筑数量如果作者写死：

- 无法表达城市中心与外缘的差别；
- 会为了目标数把房子塞进错误地块；
- 会和道路、入口、市场、设施容量冲突。

当前合同：

```text
residents/workers → households/jobs/retail/facility demand
→ eligible lots/capacity
→ buildings
```

数量是结果。`recommendedCount` 只用于形态和密度核对。

### 4.7 为什么规则引擎与生成器并存

不是所有对象都能用同一种算法生成：

- 自然地形和道路需要图/场/路径；
- 建筑需要地块/体量；
- 市场需要网络/容量；
- 街具只需要候选约束。

规则引擎提供统一“能不能放在这里”的最后硬门；每个生成器负责“候选从哪来”。

### 4.8 为什么审计不止一种

- `scale-audit`：尺寸、水体、树木等比例；
- `quality-report`：汇总结构/服务/市场/入口；
- `final-entity-audit`：最终实体 OBB、水体、贴地；
- `rules audit`：合同逐项重放；
- AccessGraph：真实步行入口；
- market radiation/deficit：服务守恒；
- world quality gate：发布判定。

任何一个通过都不能替代另一个。

---

## 5. 当前实际生成链

### 5.1 总入口

主入口是：

```text
generate_entities_with_profile_report(...)
```

它并不是一个单纯“从模板挑实体”的函数，而是当前多个规划阶段的汇合点。

### 5.2 当前顺序（重要）

当前 `worldgen.rs` 的实际顺序接近：

1. 自然 heightfield bake；
2. natural drainage 派生；
3. natural baseline freeze；
4. route evolution plan；
5. settlement evolution plan；
6. epoch evolution；
7. planning history；
8. market catchment；
9. road network plan；
10. development plan；
11. 旧模板/land-use 生成实体；
12. 设施、路线道路、建筑、楼梯、入口、市场材料化；
13. 规划历史的道路拓宽、中心、公园；
14. 自然细节清理；
15. 工程 carve 与差分账本；
16. final audit、sidecar、gemap、报告、适配器输出。

### 5.3 为什么这仍是过渡架构

因为：

- `routeEvolution` 和 `roadNetwork` 都可能成为道路来源；
- `settlementEvolution` 和旧 `frontage/settlementProgram` 都可能成为建筑来源；
- 模板生成器可能在自然/路线/聚落之后加入自己的道路、建筑和细节；
- 目前没有“新路径只能消费旧路径”的 schema 级互斥；
- 当前用多个后置 retain/sanitize 校正冲突，而不是由阶段契约阻止冲突产生。

**下一位不要继续扩展这一格局。**

### 5.4 旧的 road-first 链是什么

```text
roadNetwork
→ development zones
→ frontageProgram / settlementProgram
→ roadside sites
→ buildings/approaches
```

它解决过：

- 滨水 road-first；
- 自动道路扩展；
- 临街地块；
- 道路/入口/地表闭合；
- B-01/B-02 补给控制。

仍然有价值：

- 其中多个约束和审计应保留；
- 地形 carve、临街入口、道路横断面经验必须迁移；
- 但不应继续作为第二套城市主链。

### 5.5 新的 evolution 链是什么

```text
naturalGeography
→ naturalHeightfield
→ routeEvolution
→ settlementEvolution
→ epochEvolution
→ planningHistory
→ marketCatchment
→ materialize
→ engineering/access/audit
```

它解决：

- 自然先行的路线；
- 非直线地形道路；
- 需求驱动建筑数量；
- 新旧城；
- 市场网络；
- 宏观工程预算。

它仍需：

- 排水进入地形发生机制；
- 街区和地块正式成为独立阶段；
- 统一时代枚举；
- 完全消费自然生境场；
- 删除/降级旧模板主链。

---

## 6. 关键数据合同

### 6.1 Manifest

必须包含：

```json
{
  "format": "glyphweave-world",
  "version": 1,
  "world": {
    "name": "...",
    "seed": 42,
    "renderMode": "2.5d"
  },
  "scenes": [],
  "style": {},
  "landmarks": [],
  "sceneGraph": {"transitions": []}
}
```

当前常见 style 段：

| 字段 | 用途 |
|---|---|
| `naturalGeography` | 自然地理 |
| `naturalOnly` | 零人工痕 |
| `urbanSlice` | 城市规模、crop、关系、中心、工程授权 |
| `referenceData.caseIds` | 48 城先验 |
| `landUseProfile` | 用地比例、城市形态 |
| `routeEvolution` | 自然路线 |
| `settlementEvolution` | 聚落需求/地块/建筑 |
| `populationProgram` | 居民/岗位/设施 |
| `epochProgram` | 时代半径 |
| `planningHistory` | 中心/拓宽/公园 |
| `marketCatchment` | 市场层级/容量 |
| `engineeringScale` | 宏观工程预算 |
| `frontageProgram` | 旧临街模板 |
| `settlementProgram` | 旧显式服务模板 |
| `automaticRoadExtensions` | 旧自动道路扩展 |
| `assetContracts` | 对象语义/尺寸/地表 |
| `rulesMode` / `rulesDir` | 规则引擎 |

### 6.2 实体

每个 `EntityInstance` 至少：

```json
{
  "entityId": "...",
  "assetId": "...",
  "kind": "...",
  "worldX": 0,
  "worldZ": 0,
  "worldY": 0,
  "widthM": 1,
  "depthM": 1,
  "heightM": 1,
  "rotationYDeg": 0,
  "scale": 1,
  "grounding": {},
  "anchors": []
}
```

注意：

- `widthM/depthM/heightM` 是米制真值；
- `scale` 不是尺寸替代；
- 旋转是正 Y yaw；
- 入口 anchor 是空间事实，不应由预览端自行猜；
- 大型跨块实体需要 bounds/裁剪代理，不复制整份实体。

### 6.3 规划报告

常见输出：

```text
natural-terrain.json
natural-terrain-audit.json
natural-baseline.json
route-evolution-plan.json
settlement-evolution-plan.json
settlement-candidates.json
epoch-evolution-plan.json
planning-history-plan.json
market-catchment-plan.json
market-radiation.json
market-deficit.json
road-network-plan.json
engineering-scale-plan.json
engineering-placement.json
bounded-cut?
access-surfaces?
land-use-ledger?
final-entity-audit.json
scale-audit.json
quality-report.json
```

报告是审计依据，不是 UI 装饰。

---

## 7. 资产与数据

### 7.1 外部资产

当前正式注册：

- `prop.tree`
- `prop.bush`
- `prop.rock`
- `prop.building`
- `prop.building.small`
- `prop.building.wide`
- `prop.building.round`

来源、许可证和源文件见：

- `assets/glyphweave.registry.json`
- `assets/third_party/quaternius/README.md`
- `assets/third_party/quaternius/buildings/NOTICE.txt`

注意：

- 建筑 OBJ 几何单位与真实米制不一致；
- 运行时把模型归一化到实体比例；
- 后续应明确 `modelSpace` 与 `sourceBoundsM`，不要让适配器猜。

### 7.2 资产合同

每个新增模型/材质/实体都要先有语义契约：

- allowed surfaces；
- forbidden surfaces；
- placement；
- min/max size；
- ground/water clearance；
- slope；
- footprint/collision/pivot；
- assetId；
- license/provenance。

不能只在 HTML 里放一个模型文件。

### 7.3 48 城数据

数据文件：

```text
assets/data/urban-case-priors-48.json
assets/data/urban-case-evidence.json
assets/data/urban-core-measurements/*.json
assets/data/urban-history-patterns.json
assets/data/urban-pressure-patterns.json
```

使用规则：

- 只用核心切面；
- 只作参数先验/机制证据；
- 不做整城平均复制；
- OSM 建筑数视为下界；
- 自然地理 evidenceTier 当前仍为 mechanism-prior；
- 48 案未全部烘焙。

当前 `guiyang-ahha-wetland-512.manifest.json` 有 caseId/viewpoint 模板残留，未修复前不能
把贵阳作为正式复刻验收。

---

## 8. 旧里程碑与设计演进

这一节保留“为什么架构走到今天”的上下文。数字是历史证据，不等于当前重新跑过。

### 8.1 起源：游戏方向与地图闭环

2026-07 至 08：

- gemap v3 存储、Bevy P1 底座形成；
- 旧殖民挑战游戏 `Flood Fortress` 方向后来被搁置；
- 当前主线转为 `glyphweave-world-todo.zh-CN.md` 的大世界生成闭环；
- 核心需求包括 Manifest、分块、LOD、预览、资产替换、多场景和自动验收。

设计影响：

- 保留 gameplay 代码，不作为当前生成入口；
- 保持 Rust core 对 UI/引擎独立；
- 从结构上避免 HTML 伪生成。

### 8.2 A：稳定底座

A 的目标：

- 世界可以暂时难看，但必须可复现、可追溯；
- 坐标、分块、高精度高度、sidecar、gemap、LOD、哈希、消费者边界；
- 最大 6×10km 完整烘焙；
- 消费者不能绕过 Rust 造物。

A 的历史结果：

- 2026-09-09 签收；
- 后续最大图 complete-ground：240 chunks / 623 entities；
- gate pass；第二次烘焙 worldRevision 相同；
- 1777 个可比文件逐字节一致；
- 生态 342,380/342,380/0；
- 仍保留偶发 `0xC0000005` 风险记录。

A 的设计影响：

- 512m 流式、16m storage、32×32 region 不混用；
- 高度精度 mm/GWHM 与旧 0.25m 兼容；
- 所有消费者读同一 sidecar。

A 不等于：

- 城市已经好；
- land-use/建筑已经完整；
- 视觉已经通过。

### 8.3 B：真实街区/聚落

B 分五批：

#### B-01 独立地块几何

目标：

- 不再把“楼框 + 2m”当地块；
- 真实前院/侧院/后院/临街入口；
- 地块→建筑→入口可追溯。

结果：

- joint-plains 512m：18/18 住宅、4/4 服务、gate pass；
- parcel、insets、yard、ownership、ledger 和回归测试建立。

#### B-02 道路容量与供给

目标：

- 主/次/支路真正有等级；
- 自动路不是无限供给；
- 地块必须接合法道路。

结果：

- 512m 控制 3 条自动路，collector/local 实网；
- frontage 24/24 realized-with-access；
- disconnected 0；
- road/tier audit pass。

#### B-03 用途与地表闭合

目标：

- 规划面积必须实现为可见地表；
- 城市、院落、农田、林地、牧地、保留地分别记账；
- 不允许“计划了但画面没有”。

结果：

- 512m 某控制 unresolved 128,542→0；
- 最大图 ground 50,538,180m²，ledger 60,000,000/0。

#### B-04 形态与公共服务

目标：

- 紧凑混合、低密住宅、村落/农田有可辨认差异；
- heightM、serviceRadius、coverage 和公共空间进入 profile。

结果：

- 六种显式 profile；
- 48 案例索引；
- axis/node growth；
- 公共空间与退界派生；
- 村庄控制 18/18 户、4/4 服务；
- compact 24/24 地块 coverage pass。

#### B-05 集成与生态尾差

目标：

- 多 seed、非零 origin、最大图；
- 完整 land-use/生态/地表；
- 二次烘焙确定性；
- HTML 最大图运行。

结果：

- 最大图 gate pass；
- 工程版最大图 gate pass；
- 1777 files 逐字节一致；
- HTML 战略/2.5D 无 console error；
- Godot smoke 后续通过。

#### B 的边界

B 的“核心里程碑完成”不等于今天的自然→集群→新旧城→市场全部完成。

后续重新定义了：

- 建筑数量不能固定；
- 自然必须先冻结；
- 道路必须从自然路径来；
- 新旧城是 epoch 过程；
- 市场要容量和竞争；
- 48 城只取核心机制。

所以旧 B 的生产能力必须保留，但主生成链仍需重构。

### 8.4 C：自然—工程—人文

C 分项：

| 项 | 内容 | 结果 |
|---|---|---|
| C-N1 | 自然来源矩阵 | Mapgen4/symbios 只参考，不直接复制 |
| C-N2 | 自然/工程耦合 | 部分近似 |
| C-E1 | 挡墙/涵洞 | 山城 232 挡墙 / 5 涵洞 |
| C-E2 | 工程后排水 | 自然 vs 最终 D8，87 改向 / 5 抬升终端 |
| C-L1 | 许可证 | Quaternius/Workerd 来源已解析 |
| C-V1 | HTML 实测 | 战略/2.5D 已验证 |
| C-G1 | Godot runtime | 4.7.2 headless smoke pass |

C 的设计影响：

- 工程必须实体化；
- 地形改变必须能对比自然/最终；
- 自然来源、语义和视觉审核要形成矩阵。

C 仍不表示：

- 完整水力侵蚀；
- 洪泛模型；
- 完整桥梁/隧道/排水；
- 地貌生态已全部消费。

### 8.5 2026-09-29 ABC 纠偏

新增 R01–R16、FUT-01–06，明确：

- 回到 B 生产链；
- 未来能力登记但不实现；
- 区分节点/土地/建筑/道路/入口真实关系；
- 数据来源和视觉不能替代结构验收。

### 8.6 2026-10-20 A/B/C 快照

项目形成“A 底座 + B 街区 + C 工程”的核心里程碑快照，但仍有：

- 偶发崩溃；
- 1km 供给上限；
- B-04 形态部分；
- C 的排水/洪泛深度不足；
- 最大图 land-use 历史例外。

### 8.7 2026-10-21 自然到城市 V2/V3

负责人明确重排：

```text
自然
→ 山谷/渡口/桥头路径
→ 聚落
→ epoch 新老城
→ 设施与生鲜市场
```

新要求：

- 普通建筑数量不是固定目标；
- 建筑尺寸由地形和地块更新；
- 旧城/更新/新区/郊区/旅游/放牧/种植不同；
- 中心由网络和历史决定；
- 市场使用容量/竞争/网络时距；
- 48 城只取核心切面；
- 局部硬合同不放松；
- 特征区可申请独立宏观工程授权。

### 8.8 2026-10-22 硬占用与市场

修复/验证：

- 任意 yaw 不能退化为 AABB；
- 道路先入占用，建筑不能压路；
- 建筑-道路、建筑-建筑、建筑-细节碰撞为 0；
- 入口真实 AccessGraph；
- 市场辐射与缺口守恒；
- 道路 profile 与实体标高同源。

### 8.9 2026-10-23 近期迭代

主要变化：

- r244 密度曲线；
- r246 人口设施；
- r247 显式道路等级；
- r248 重规划；
- r249–r252 三城切片；
- r253/r254 相机与三城可运行预览；
- r201 前入口全部有验证路径，无法建入口的建筑具名拒绝。

三城 r254：

| 城市 | quality | 实体 | 建筑 | 市场 | 主要缺口 |
|---|---|---:|---:|---|---|
| 旧金山 | warn | 232 | 9 | 无 | structured-water land-use、生态、岸线 |
| 上海 | pass | 455 | 18 | 100% | 仍需高密街墙和真实外滩连续界面 |
| 贵阳 | pass | 906 | 27 | 100% | Manifest case/viewpoint 语义错误 |

### 8.10 当前结论

历史里程碑证明“底座、规则、工程和审计能力存在”。

历史里程碑不证明“当前城市生成顺序正确”。

当前重构的核心是：

```text
把能力放回正确阶段
→ 让后阶段不能重写前阶段
→ 用单一主生成器控制城市
```

---

## 9. 推荐目标生成步骤

下一位应把以下步骤视为目标架构，而不是当前的函数调用顺序。每个阶段都应先产出
冻结计划，再允许下一阶段消费。

### 9.1 Step 0：Source & Intent

职责：

- 规范化 Manifest；
- 验证来源、crop、caseId、场景尺度；
- 确定城市规模、自然地理、人口/岗位基线、工程授权。

产物：

- canonical Manifest；
- `source-audit.json`；
- `world-brief.json`；
- effective programs；
- reference hash。

禁止：

- 真实地点没有来源；
- 512m 窗口冒充完整景区；
- caseId 与地点不一致；
- 未声明工程授权就放宽宏观限制。

通过条件：

- 来源和 crop 可追溯；
- 三城名称/意图一致；
- 所有未知关键输入明确追问或显式 inference。

### 9.2 Step 1：Natural Skeleton

职责：

- 宏观构造/山系/盆地；
- 海岸；
- 汇水/河流/湖泊/湿地；
- 侵蚀/沉积近似；
- 1m natural heightfield；
- 可建性、坡度、湿度、灾害和 habitat field。

产物：

- natural heightfield；
- drainage graph；
- landform/habitat；
- natural baseline/hash；
- 自然审计。

通过条件：

- 零人工对象；
- 排水无非法边；
- 水陆/跨块连续；
- 自然对象尺寸/地表全部通过；
- 同 seed 可复烘。

### 9.3 Step 2：Route Skeleton

职责：

- 从自然节点/山口/渡口/港口/目标中心生成候选路径；
- 比较地形拟合、直线、桥、隧道、高架和填方；
- 生成道路等级与总路权。

产物：

- route-evolution plan；
- road alignment/design；
- engineering-scale plan；
- bridge/tunnel/overpass plan；
- route audit。

通过条件：

- 道路可通行；
- 坡度/填方/桥跨在合同内；
- 可见路面与 OBB 同宽；
- 平地主干路直线规则成立或显式说明例外；
- 没有建筑先于道路。

### 9.4 Step 3：Settlement Seeds

职责：

- 从路线交叉/渡口/站场/港口/桥头/市场和历史锚点选择聚落种子；
- 计算网络中心、中心角色、可建设容量；
- 确定聚落边界和主次中心。

产物：

- settlement seeds；
- centre graph；
- growth capacity；
- focal anchors。

通过条件：

- 每个中心有网络或叙事依据；
- 不落水、不落陡坡、不侵占绝对保护地；
- 大都市只生成切面，不虚构完整城市。

### 9.5 Step 4：Epoch / District

职责：

- 给聚落分配 founding/old-core/expansion/industrial/modern-new/renewal/periphery；
- 分配住宅/岗位/商业/服务预算；
- 记录 preserve/modify/demolish/replace。

产物：

- unified district enum；
- epoch plan；
- district demand；
- building operation parent chain。

通过条件：

- 阶段名只有一套规范枚举；
- 旧城/更新/新区空间形态不同；
- 保护对象不能被普通拆改；
- 未使用预算进入合法回填，不伪造完成。

### 9.6 Step 5：Blocks & Local Roads

职责：

- 主路围合街区；
- 生成集散路、本地路、巷道和步行连接；
- 处理路口、转盘、过街和公共空间。

产物：

- block polygons；
- local road network；
- public space；
- junction/intersection templates。

通过条件：

- 街区由道路围合；
- 平地区块优先规则/网格；
- 主路交叉数量受控；
- 路口几何连续；
- 可见道路与碰撞/设计面一致。

#### 9.6A 关键顺序

必须是：

```text
道路/街廓边界
→ 街区
→ 地块
→ 建筑
```

不能：

```text
先随机建筑
→ 再补道路
→ 最后移动建筑
```

### 9.7 Step 6：Parcels & Land Use

职责：

- 每街区切地块；
- 分配住宅、商业、办公、工业、公共、绿色和混合用途；
- 预留学校、医院、市场、公园、消防等。

产物：

- parcel plan；
- land-use ledger；
- facility reservations；
- frontage/setback/access candidates。

通过条件：

- 地块不跨水/跨陡坡/跨块缝；
- 大型设施先预留地块；
- 公共空间与市场有真实临街/可达关系；
- 面积预算可解释。

### 9.8 Step 7：Demand & Facilities

职责：

- 人口/岗位 → 住宅/就业；
- 人均零售 → 商业面积；
- 人口阈值 → 学校/医院/教堂等；
- 空地块不足时显式重规划；
- 失败设施具名。

通过条件：

- 请求设施不缩尺寸；
- 重规划有父对象和拆除记录；
- 需求/交付/缺口三者守恒。

### 9.9 Step 8：Fresh-food Network

职责：

- 社区菜场、邻里超市、大超市、周期市场、批发市场；
- 真实步行/道路时间；
- 容量与竞争；
- 5/10/15/30 分钟统计带。

通过条件：

- 不使用直线半径；
- 容量负载不超限；
- 总需求 = 服务 + 缺口；
- 逐市场分配可复算。

### 9.10 Step 9：Building Massing & Epoch Operations

职责：

- 按地块、FAR、层数、中心压力、社区原型选择建筑；
- 计算 W/D/H；
- modify/preserve/demolish/replace；
- 生成最终建筑父子链。

通过条件：

- 数量是结果；
- 不允许固定盒子尺寸；
- 同一地块不重复建筑面积；
- 保护对象/设施/市场不被普通建筑覆盖；
- 所有 final buildings 有来源和 epoch。

### 9.11 Step 10：Access & Engineering Closure

职责：

- 建筑入口；
- 门前铺装；
- 台阶/坡道/楼梯；
- pad、挡墙、涵洞、桥、高架；
- 地形 carve；
- 自然/final diff。

通过条件：

- 每个建筑至少有验证入口；
- 楼梯从门槛落到道路边缘；
- 没有悬空、埋地、落水、压路；
- 所有地形改变有工程 ledger；
- 保护区/水岸/水体不得普通削方。

### 9.12 Step 11：Ecology & Street Life

职责：

- 从 habitat field 生成森林/灌木/草甸/湿地/岩生斑块；
- 街树、树池、路灯、长椅、标识；
- 水面、岸线和湿边；
- 建筑立面与道路断面的视觉资源。

通过条件：

- 不使用独立圆斑噪声；
- 树冠/草/灌木不穿墙；
- 树冠尺寸不是树干尺寸；
- 水体有 level/wave/shore；
- 视觉资源不反向定义碰撞和尺寸。

### 9.13 Step 12：Bake & Gate

职责：

- sidecar/gemap/LOD/hash；
- 全套 audits；
- HTML/Godot；
- visual feedback。

通过条件：

- Source/Semantic/Scale/Bake/Continuity/Adapter/Visual/Feedback 门；
- 退出码和 JSON `status` 同时通过；
- 截图非空且可辨识结构；
- 不把缩略图当视觉验收。

---

## 10. 推荐实施顺序与任务拆分

### 10.1 P0-A：统一生成入口与 provenance

目标：

- 结束两套主城市链不可控叠加；
- 让每个最终实体可追溯唯一父计划；
- 修复版本和输入语义。

任务：

1. 在 Manifest/style 中增加城市生成器选择：

```text
cityGenerator = "evolution" | "legacy-road-network"
```

2. schema 层拒绝同场景双主生成器。
3. 旧 frontage/settlementProgram 仅允许 legacy 或非结构性 detail。
4. `EntityInstance.grounding` 或旁路索引增加：

```text
sourceStage
parentPlanId
sourceCandidateId
```

5. `generationProfile`、`generatorRevision` 和程序哈希统一。
6. 修复贵阳 caseId/viewpoint。
7. 建立阶段产物契约文档/schema。

验收：

- 同一 Manifest 只有一套主 road plan；
- 每栋 final building 可追到 parcel/block/epoch；
- 每段 road 可追到 route 或 legacy road plan；
- 三城 manifest source/crop/caseId 一致；
- old controls 不变或显式 legacy。

**必须先做这一项。**

### 10.2 P0-B：自然骨架成为真输入

任务：

1. 增加/提升汇水与排水分辨率；
2. 让 drainage 先于 final natural heightfield 决定谷地；
3. 形成山脊、谷地、冲积扇、阶地、三角洲、海岸、湿地对象；
4. 统一水体 footprint/level/河床/岸线来源；
5. 输出 habitat field；
6. 自然对象和生态选择消费 habitat。

验收：

- naturalOnly 零人工痕；
- 排水合法；
- 河流/湖泊/岸线跨块连续；
- 水体与所有陆生实体零相交；
- 生态斑块由坡度/高程/湿度/水体边界形成。

### 10.3 P1：路线与工程骨架

任务：

1. 路线只消费冻结自然；
2. 多候选路径评分；
3. 平原主干路 straight/straight-preferred；
4. 桥/隧道/高架作为显式实体；
5. 路宽先按等级，不按资产；
6. 路口/转盘/互通模板；
7. 工程预算与 carve 账本。

验收：

- 所有主路可连通；
- 纵坡/填方/桥跨通过；
- 道路 OBB 与可见断面一致；
- 无固定 worldY；
- 弯道不再碎片化。

### 10.4 P2：聚落种子与时代

任务：

1. 中心从路网/港口/桥头/市场/历史锚点产生；
2. 统一 epoch 枚举；
3. 旧城/更新/新城/外围不同街区/地块/建筑；
4. 重建/保留/拆除/替换父链。

验收：

- 中心不是几何圆；
- 新旧城有街网/地块/用途/高度差异；
- 保护对象不被误拆；
- 所有建筑有 epoch cohort。

### 10.5 P3：街区、地块、建筑

任务：

1. 道路围合街区；
2. 切地块；
3. 预留设施/市场/公共空间；
4. 由人口/岗位需求消费地块容量；
5. 建筑由 archetype 生成 W/D/H；
6. 不固定建筑数量。

验收：

- 建筑数由需求/地块/入口产生；
- 中心密度与边缘差异；
- 单体 footprint 不是统一小盒；
- 相邻 OBB/道路/细节碰撞为 0。

### 10.6 P4：市场与设施

任务：

1. 市场先占真实可达临街；
2. 学校/医院/教堂等后置；
3. 空地块不足时具名重规划；
4. 容量/竞争/服务带。

验收：

- 市场负载/覆盖/缺口守恒；
- 每个设施请求具名交付/失败；
- 不缩小设施；
- 不靠直线距离。

### 10.7 P5：入口、楼梯与工程闭合

任务：

1. 所有可建建筑提供真实入口；
2. 门槛-落点楼梯；
3. 侧/后入口；
4. 台地/挡墙/涵洞/桥；
5. 最终实体审计 fail-closed。

验收：

- 最终建筑入口 100%；
- 无孤立 approach/stairs；
- 无悬空/穿地/落水/压路；
- 地形修改有 ledger。

### 10.8 P6：生态/街景/资产

任务：

1. habitat patch；
2. 街树、路灯、长椅；
3. 水岸湿地；
4. 建筑立面材质；
5. 预览性能与分块裁剪。

验收：

- 不再有圆形草场；
- 楼内无绿植；
- 道路无碎裂；
- 建筑/道路/树/水尺度合理；
- 运行态截图通过。

---

## 11. 规则执行位置索引

| 要改的内容 | 必读/改动位置 | 不得绕过 |
|---|---|---|
| Manifest | `worldgen.rs` schema/validate、`bevy/crates/cli/src/main.rs` | unknown fields/尺寸 |
| 自然地理 | `worldgen/natural_geography.rs` | source/crop |
| 排水 | `natural.rs` | 非法边/封闭盆地 |
| 水体 | `worldgen.rs` water_geometry、scale audit | footprint/level/shore |
| 路线 | `worldgen/route_evolution.rs` | 8%/6m/桥跨 |
| 道路断面 | `RoadDesign`、`road_profile.rs` | OBB = visible width |
| 挖填 | `bounded_cut.rs`、`engineering_scale.rs` | ledger |
| 聚落 | `settlement_evolution.rs` | 需求产生数量 |
| 建筑原型 | `settlement/building_archetypes.rs` | 尺寸/层数/覆盖率 |
| 时代 | `epoch_evolution.rs`、`planning_history.rs` | 父链/保护 |
| 市场 | `market_catchment.rs` | 网络/容量/守恒 |
| 入口 | `access.rs`、`entry_corridor.rs` | 25%/120m/0.75m |
| 楼梯 | `settlement_evolution.rs`、stairs contract | 0.18m/22m/宽度 |
| 最终碰撞 | `final_scene_audit.rs` | OBB/贴地/水 |
| 规则引擎 | `rules/`、`assets/objects/` | hard constraints |
| HTML | `adapters/html/` | 只读 Rust 真值 |

---

## 12. 运行与验收手册

### 12.1 环境

在仓库根目录：

```powershell
git config core.hooksPath .githooks
```

### 12.2 当前数据校验

```powershell
pnpm check:urban-cases
```

### 12.3 Rust 核心测试

短暂检查：

```powershell
cargo check --manifest-path bevy/Cargo.toml --workspace --tests
```

核心测试：

```powershell
cargo test --manifest-path bevy/Cargo.toml -p glyphweave-core --lib
```

完整 workspace：

```powershell
cargo test --manifest-path bevy/Cargo.toml --workspace --no-fail-fast
```

注意历史上存在约低概率 `0xC0000005` 偶发崩溃；若发生，保留日志、重跑并记录，
不能以 RUST_MIN_STACK/catch_unwind/放宽断言掩盖。

### 12.4 生成与审计

```powershell
cargo run --manifest-path bevy/Cargo.toml -p glyphweave-cli --release -- generate-world <manifest> <out>
cargo run --manifest-path bevy/Cargo.toml -p glyphweave-cli --release -- scale-audit <out>
cargo run --manifest-path bevy/Cargo.toml -p glyphweave-cli --release -- quality-report <out>
cargo run --manifest-path bevy/Cargo.toml -p glyphweave-cli --release -- world-quality-gate <out>
```

必须同时看：

- 进程退出码；
- `scale-audit.json.status`；
- `quality-report.json.status`；
- final entity audit；
- 视觉反馈。

### 12.5 预览

```powershell
cargo run --manifest-path bevy/Cargo.toml -p glyphweave-cli -- preview <out> 8080
```

常用 URL：

```text
?view=strategic
?view=street
?view=skyline
?view=explore
?view=natural
```

修改 `adapters/html/` 后必须重新生成世界目录，因为输出会复制适配器；不能用旧
输出目录证明新渲染通过。

### 12.6 前端

```powershell
pnpm lint
pnpm typecheck
pnpm test
pnpm build
```

### 12.7 三城

```text
examples/san-francisco-golden-gate-south-512.manifest.json
examples/shanghai-bund-512.manifest.json
examples/guiyang-ahha-wetland-512.manifest.json
```

逐城烘焙、逐城截图，不并行压机器。

---

## 13. 当前已知问题与风险

### P0

#### 13.1 双主生成链

见第 5 节。下一位必须先解决，不能继续叠加。

#### 13.2 生成版本字符串失真

`WORLD_GENERATOR_REVISION` 仍为旧 r51 字符串，不能代表 r254。

#### 13.3 贵阳输入语义错误

caseId 指向昆明；viewpoint/specialFactors 是复制残留。

#### 13.4 自然排水未驱动地形

当前是 32m 采样事后图，非完整自然发生机制。

### P1

#### 13.5 旧金山 structured-water land-use 不完整

r254 quality warn；服务和生态仍缺。

#### 13.6 道路路口并集/互通

当前有盖面和裁断，没有完整路口并集多边形。

#### 13.7 入口 optional anchors

实体级 `disconnectedEntities=0` 不代表每个侧/后入口都通；验收时要按 required
入口和实际街景分别看。

#### 13.8 建筑体量

近期中心覆盖率仍低于高密核心样本；不能靠增加小盒子。

#### 13.9 资产 unit 失真

OBJ 几何尺寸与 registry `unit=meter` 不一致；运行时靠归一化。

### P2

#### 13.10 偶发 Windows 访问违例

历史约低概率，崩溃点在 road-network JSON 序列化附近；已有取证工具，不能用
随机全绿宣告修复。

#### 13.11 Patch 链

合同要求 Patch，但历史世界 `patchChainHash` 为空，生产链未闭合。

#### 13.12 三城仍不是完整城市复刻

它们是结构/机制切片，不是城市全貌。

---

## 14. 不要踩的规则

- 不要直接把 48 城建筑中位数当硬配额。
- 不要从平铺贴图反推车道和道路宽度。
- 不要按“我手上有什么材质”决定道路 class。
- 不要在预览端重算或移动地图实体。
- 不要用缩略图/SVG 代替真实运行态视觉验收。
- 不要让树冠、草、灌木进入建筑/入口/道路。
- 不要只用中心点判断碰撞、水体或入口。
- 不要静态生成 car/pedestrian/boat 等动态对象。
- 不要在建筑失败后缩小实体来蒙混过门。
- 不要让建筑数量由固定 count 完成。
- 不要把局部 6m/8%/25%/2m/4m 等合同为了画面好看调松。
- 不要在没有工程账本的情况下削山、填海、造岛。
- 不要在 chunk 边界只检查中心。
- 不要使用一个 chunk 的主题色画整块 LOD。
- 不要忽略保护区、岸线和绝对水体。
- 不要在旧 world 上验证新源码通过。
- 不要挑 seed 让数字好看。
- 不要用单一 `quality-report=pass` 代替结构+视觉。

---

## 15. 下一位的前三天任务

### 第一天：确认基线

1. 通读本文和审核稿第 8 节；
2. `git status`，识别无关未跟踪目录；
3. `pnpm check:urban-cases`；
4. 跑一次 targeted Rust tests；
5. 读取 r254 三城 quality report；
6. 记录当前 HEAD、分支、测试和输出。

### 第二天：统一入口设计

1. 画当前 `generate_entities_with_profile_report` 调用图；
2. 枚举所有写道路/建筑/入口的路径；
3. 设计 `cityGenerator` 互斥和 parent provenance；
4. 先写 schema/行为测试，不改默认旧世界；
5. 给出迁移计划，不直接全量改。

### 第三天：阶段契约

1. 定义每个阶段产物；
2. 定义阶段 hash 和失效传播；
3. 定义 "后阶段禁止重写前阶段" 的测试；
4. 选定一个最小场景做 dry-run；
5. 记录失败原因，不以视觉补丁掩盖。

---

## 16. 交接完成标准

下一位 Agent 在开始新功能前应能回答：

- 当前主生成器是哪条？
- 道路由哪个 plan 产生？
- 建筑如何从人口需求变成实体？
- 入口如何验证？
- 市场如何计算覆盖和负载？
- 哪些 rules 是 fail-closed？
- 哪些 health check 只是审计？
- 哪些旧里程碑可用，哪些需要修正？
- 哪些文件是 Rust 真源，哪些是消费者？
- 三城为什么还不算完成？

如果还不能回答，先继续读本文和审核稿，不要开始大改。

---

## 17. 文档地图

### 必读

- 本文
- `CAPABILITY-INVENTORY-AND-STAGED-GENERATION-20261023.zh-CN.md`
- `requirements-and-progress-master.zh-CN.md`
- `docs/glyphweave-world-todo.zh-CN.md`
- `docs/urban-natural-generation-plan.zh-CN.md`
- `docs/20260906-natural-terrain-generation-research.zh-CN.md`
- `docs/20260906-human-settlement-morphology-research.zh-CN.md`

### 当前实现证据

- `ROAD-SECTION-OBB-DENSITY-20261023.md`
- `ROAD-HIERARCHY-R247-20261023.zh-CN.md`
- `POPULATION-FACILITIES-R246-20261023.zh-CN.md`
- `REDEVELOPMENT-R248-20261023.zh-CN.md`
- `MARKET-RADIATION-20261022.zh-CN.md`
- `NATURAL-EVOLUTION-W1-W5-20261021.zh-CN.md`
- `THREE-CITY-SLICES-R249-R252-20261023.zh-CN.md`
- `REFERENCE-AUDIT-20261023.zh-CN.md`
- `HARD-OCCUPANCY-GROUNDING-REGRESSION-20261022.zh-CN.md`

### 旧里程碑/历史

- `ABC-STATUS-20261020.zh-CN.md`
- `FINAL-VERIFICATION-20261020.zh-CN.md`
- `ROUND-LOG.zh-CN.md`
- `docs/collaboration/abc-goals-realignment-and-roadmap-20260929.zh-CN.md`
- `docs/collaboration/abc-follow-up-execution-plan-20260929.zh-CN.md`
- `bevy/work/collaboration/rk/`（历史工单证据）

### 规则与数据

- `assets/objects/*.object.toml`
- `assets/glyphweave.registry.json`
- `assets/data/urban-case-priors-48.json`
- `assets/data/urban-case-evidence.json`
- `assets/data/urban-core-measurements/`
- `scripts/check-urban-cases*`

---

## 18. 交接摘要

如果只能记住五件事：

1. **Rust Core 是唯一地图真源，HTML/Godot 只读。**
2. **当前有两条主城市生成链，必须先统一。**
3. **48 城是核心机制先验和 OSM 部分测量，不是 48 个成品城市。**
4. **建筑数量、市场辐射、入口和碰撞必须由需求/网络/几何产生，不能靠 UI 或固定 count。**
5. **下一步是阶段化生成和 provenance，不是继续加随机细节。**
