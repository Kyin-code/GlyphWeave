# GlyphWeave 现代社会地图生成架构（Rust Core + Agent）

## 目标

将 GlyphWeave 从“可生成且不碰撞的对象集合”推进为可解释的现代社会地图：自然基底先成立，既有自然/人类事实再进入规划，人类建设经过地形工程，最后由人类—自然反馈生成材质、植被和环境影响。Agent 只负责从自然语言提取意图、选择 profile、提出修改；Rust Core 掌握所有空间事实、候选位置、修复和审计。

单场景限制为 512m–6000m × 512m–10000m；渲染器和 Godot/HTML 适配器只能消费 baked 数据，不能重新生成高度、水体或碰撞语义。

## 统一流水线

```text
WorldIntent / Manifest
  -> 输入校验和版本化归一化
  -> L0 自然基底：height / hydrology / landform / climate
  -> L1 自然对象：water / soil / rock / vegetation patches / hazards
  -> L1.5 既有事实：existing road / farm / wall / building / protected feature
  -> L2 聚落规划：settlement nodes / morphology profile / land-use budget
  -> L3 交通和街区：road graph / drainage graph / blocks / parcels
  -> L4 建筑组：building cluster / entrance / parking / yard / service access
  -> L5 地形工程：grade / fill / cut / retaining / drainage
  -> L6 人类—自然反馈：runoff / wetness / disturbance / vegetation response
  -> L7 最终 baked raster + semantic sidecar + entities
  -> Core audit + CLI audit + quality gate
  -> HTML / Godot 表现层
```

生成顺序不是“对象优先级列表”，而是数据依赖关系。后一个阶段只能读取前一个阶段已经提交的事实。

## 空间语义

### TerrainSemantic sidecar

每个 1m baked cell 具有：

- `surface`：水、浅水岸线、泥滩、草地、耕地土、裸土、石面、硬化地面；
- `slope`：最终 baked 高度场推导的坡度；
- `curvature`：局部地形曲率；
- `wetness`：水文和湿度场；
- `disturbance`：道路、建筑 pad、施工或土地转化造成的人类扰动。

高度仍由 height payload 独立保存；sidecar 只能影响材质、植被和分析，不能位移几何。

### ExistingFeature

自然石头、现有树、旧道路、农田、围墙和旧建筑不是普通 generated entity，而是具有来源、永久性、保护状态和转化策略的既有事实。默认规则：河道/悬崖/保护区不可移动；旧农田可被城市转化但必须产生 impact；旧道路优先成为新道路骨架。

## 现代城市形态

不用“中式/欧洲/美国”直接映射建筑尺寸，而使用 `UrbanMorphologyProfile`：

- block width/depth；
- street width 和交叉口密度；
- lot coverage、front/side/rear setback；
- building height/floor distribution；
- parking、tree canopy、public space；
- mixed-use、transit proximity、industrial separation。

生成单位为 `Settlement -> RoadGraph -> Block -> Parcel -> BuildingCluster -> Entrance`，而不是孤立建筑。

## 质量约束

### 硬约束

- 任何对象不得越界、进水、穿透最终地形或与不可兼容实体相交；
- 建筑必须有可用入口、清空区和道路/人行路径；
- 道路必须连通并具有排水出口；
- authored entity 不能被为通过门禁而移动、缩小或删除；
- 不能用 renderer 的近似判断替代 Core 的最终事实。

### 软目标

- land-use 面积误差；
- 城市连续性和街区尺寸分布；
- 绿地连通性；
- 工业与住宅隔离；
- 建筑高度梯度；
- 道路 frontage 覆盖率。

软目标只能用于候选评分和质量报告，不能覆盖硬约束。

## 本次落地

1. 新增 `BakedTerrain`，bake/CLI 共用严格的 chunk 高度读取；
2. 新增 `audit_baked_scene`，统一最终 audit；
3. 新增连续水体 footprint 查询，接入 placement 和 audit；
4. 新增 `engineered_placement`，在最终 terrain carve 之后对 generated 细节重新选址；
5. `scale-audit` 优先使用 `asset:<assetId>` authored contract；
6. `scale-audit` 每个 scene 一次性加载 heightfield，chunk hash 改为增量 BLAKE3；
7. land-use region 阈值根据 urban target 调整；
8. 本轮增加 chunk-local terrain carve 过滤，减少大地图每 cell 扫描无关 carve 的开销；
9. SceneIndex 持久化 `AccessGraph`，记录建筑入口到最近道路的连接距离和断开状态；
10. HTML terrain 已消费 slope/curvature/wetness/disturbance sidecar，不自行改变高度。

## 验证门禁

```text
cargo fmt --manifest-path bevy/Cargo.toml --all
cargo test --manifest-path bevy/Cargo.toml --workspace
cargo check --manifest-path bevy/Cargo.toml --workspace
glyphweave generate-world MANIFEST OUTPUT
glyphweave scale-audit OUTPUT
glyphweave rules audit OUTPUT --rules assets/objects
glyphweave world-quality-gate OUTPUT
```

必须分别验证：Rust 单元测试、真实 baked 产物、独立 CLI 读取、损坏 payload、极限尺寸、至少三个 seed。`validation=true` 或单个 preview 截图不能替代完整门禁。

## 后续边界

下一阶段再实现完整 `ExistingFeature` schema、RoadGraph/DrainageGraph 的显式序列化，以及按 land-use budget 的 parcel 数量/面积闭环。当前代码先确保这些概念不再由 Agent 或 renderer 擅自推导，并把最容易造成错误结果的最终地形、完整 footprint、baked audit 和 authored contract 收拢到 Rust Core。


## 2026-09-05 第八次修正：入口可达性图

新增 `SceneIndex.accessGraph`。Rust Core 会为住宅、商业、公共建筑和工业建筑的每个入口 anchor 查找最近道路/人行道/boardwalk，记录 `distanceM`、`roadId` 和 `connected`。质量报告会显示 scene 的入口连接统计，并对断开入口发出 warning。这个图是后续生成步道、消防通道、排水和公交关系的基础；它不会由渲染器猜测，也不会把“建筑没有碰撞”误认为“建筑可用”。


## 验证更新（2026-09-05）

新增 AccessGraph 后，workspace 测试为 app 44、cli 5、集成 1、core 211 全部通过；cargo check --workspace 和 pnpm build 通过。Shenzhen Bay authored fixture 的 scale-audit 为 pass、CLI rules audit 为 14/14；world-quality-gate 对特殊 water kind
iver 归类为 special_unruled，不再把它误报为普通规则缺口。



## 验收更正：AccessGraph v2（2026-09-05）

**此前“入口距离道路不超过120m就已连接”和“authored 可自动暂缓连接”的实现不成立，已替换。**

- `worldgen/access.rs`：1.5m 通道、2m搜索网格、120m长度预算、最多6000个展开节点。
- 每段对旋转障碍物做连续相交检查；水体通过连续走廊查询；最终高度场通过双线性插值采样检查中心和左右边缘的纵坡。
- 当前25%最大路径纵坡、0.30m入口台阶是游戏可行走路径的工程默认值，不是无障碍设计或真实道路规范认证。
- 首选直连，失败后有界 A* 绕行；预算内找不到路线返回明确失败，不能解释为证明全局无路。
- authored、generated 一律要求真实入口。缺入口、入口不在建筑边界、入口被挡、台阶超限均有独立状态码。
- distanceM 变为可空有限数值，失败不再序列化 Infinity；输出完整 path 折线。building计数与anchor计数分离。
- 只在生成阶段为合成 `generated.* / front` 选择立面；不移动建筑、不删除障碍、不调整用户指定门位。审计只读，不能替用户修复。
- 规则placement移动/旋转时同步变换原有anchor与bounds；自动前门按同一旋转公式定位。
- quality-report 从最终heightfield和entities重算，不信任scene.json中的accessGraph计数；strict gate因未验证路线失败，不能只报告warning后判成功。

### 本轮证据与未完成事项

旧 r5 6×10km 产物（305栋建筑）按新检查重算：20栋具备可行走路线，285栋未验证通过，含13栋旧anchor错位。**旧的绿色gate不能证明新可达性要求完成。**

此模块当前只证明到一个道路/人行道对象的地面可行走路线，不代表道路网络整体连通，不自动生成桥面/阶梯/挡墙/排水/铺装实体。不把这些缺口包装成已经落地的RoadGraph/DrainageGraph。后续必须将道路与地块、地形工程、真实入口路径联合规划，并让渲染器呈现同一份路径后再做完整验收。

### 分块优化修正

chunk-local carve 过滤必须包含1m有限差分halo；只检查chunk内部会漏掉改变边缘坡度/曲率的外侧pad。本轮增加全量carve与过滤carve四种payload逐字节一致的测试（包括负原点）。
