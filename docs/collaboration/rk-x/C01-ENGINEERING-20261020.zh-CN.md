# C-01 engineering works + engineered drainage (2026-10-20)

状态：**C-E1 首批工程实体与 C-E2 工程后路由反馈已实现并在山城 512m 控制运行**。
C 整体仍未签收；最大图工程版本与浏览器/Godot 消费尚未跑。

## 1. 新增能力

| 能力 | 开关 | 产物 |
|---|---|---|
| 挡墙/挡土结构 | `terrainWorks.retainingWalls = "auto"` | `terrain-works.json` 的 `retainingWalls[]`，并嵌入 `engineered-terrain.json.retainingWorks` |
| 涵洞/跨水排水记录 | `terrainWorks.drainageStructures = "auto"` | `culverts[]`，绑定道路、水体、跨度、湿格、河床与桥面高程 |
| 工程后水文对比 | `engineeringDrainage = "audit"` | `engineered-drainage.json`，自然图 vs 最终地形图 |

结构是数据实体：稳定 id、源 carve operation、起止坐标、高度、长度、厚度、
体积、材料、状态。渲染层可以后接，不需要在 Rust 里伪造视觉。

## 2. 山城控制实测

manifest：`examples/c01-engineering-works-512.manifest.json`
（`hillside-town-512` + `terrainWorks` + `engineeringDrainage` + complete-ground）

| 项 | 值 |
|---|---:|
| generate-world | exit 0，约 3.5s |
| carve operations | 96 |
| retaining walls | **232** |
| culverts | **5** |
| walls status | **pass** |
| engineered drainage status | **pass** |
| natural/engineered nodes | 256 / 256 |
| changed flow nodes | **87** |
| new sinks | 2 |
| lost sinks | 19 |
| raised terminals | **5** |

代表挡墙：`scene-0.carve.0.retaining-wall.0`，起止
`(171,197)->(213,197)`，高 2.414m、长 42m、厚 0.4m、体积 40.555m³。
它不是根据“应该有挡墙”生成的，而是由 carve 自然高度与最终地形高差算出。

## 3. 门禁

`world-quality-gate` 新增：

- `engineeredTerrain.retainingWorks.status != pass` → 具名失败；
- `engineeredTerrain.drainageImpact.status != pass` → 具名失败。

山城控制仍因 frontage 0/24、automatic road targets、landcover 216m²
（yard 未闭合）等 B 问题 fail；工程 works 与工程排水本身没有失败。

## 4. 边界与未完成

- 这是**一阶结构记录**：挡墙是沿 carve 周边的四面/两面墙，涵洞是跨水记录的排水构件；
  还没有挡墙基础、排水沟、竖曲线、路面横坡、洪水演算。
- 工程后排水仍是 32m 采样 steepest-descent 的**路由完整性**对比，不是水量/洪泛模型。
- 还没有把 `retainingWorks` 与 `drainageImpact` 画进 HTML；按用户当前要求，渲染不是
  优先级，但不能出现明显错乱。两者已在权威 JSON 中，消费者可按稳定 id 读取。
- 下一步：把山城 yard 216m² 闭合，或在更平地形的 1km/最大图 C 控制上重跑。

## 5. 证据

- `bevy/work/rk-x/c01-engineering-20261020e/world/scenes/scene-0/terrain-works.json`
- `bevy/work/rk-x/c01-engineering-20261020e/world/scenes/scene-0/engineered-drainage.json`
- `bevy/work/rk-x/c01-engineering-20261020e/world/scenes/scene-0/engineered-terrain.json`
- `bevy/work/rk-x/c01-engineering-20261020e/world/world-quality-gate.json`
