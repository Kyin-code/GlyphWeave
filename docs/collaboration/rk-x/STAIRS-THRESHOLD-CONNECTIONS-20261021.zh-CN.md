# 门前静态楼梯与 AccessGraph 连接

日期：2026-10-21  
控制世界：`bevy/work/rk-x/hk-central-natural-evolution-1km-r2b/`

## 目标

真实坡地建筑的门口不能靠放宽 0.30m 台阶合同通过。对 0.30m–2.40m 的局部
门口高差，生成可持续复用的静态楼梯实体，并让 AccessGraph 使用楼梯上端
作为步行路线起点。

## 数据合同

`GroundingSpec.stairConnection` 记录：

- `stairId`：实际静态楼梯实体的 ID；
- `from` / `to`：门侧与地面侧世界坐标；
- `fromMm` / `toMm`：两端毫米标高；
- `lowMm` / `highMm`：楼梯总高差；
- `stepCount`：按 0.18m 目标踏步高计算的级数；
- `widthM`：楼梯宽度，本轮固定 2.4m。

门侧高差小于 0.30m 时不生成楼梯；超过 2.40m 时仍保持
`entrance_step_requires_engineering`，等待 switchback/挡墙工程。

## AccessGraph

入口审计现在：

1. 验证建筑引用的楼梯实体真实存在；
2. 验证楼梯门端与建筑 facade 距离不超过 2m；
3. 验证楼梯水平长度不超过 4m；
4. 从楼梯地面端开始执行与普通路线完全相同的道路搜索；
5. 连通时状态写为 `walkable_route_with_stairs`；
6. 楼梯引用缺失或几何不合法时写 `stair_connection_invalid`。

楼梯不是 AccessGraph 中的“跳过门”标记，而是显式静态实体和路径点。

## r2b 结果

| 项 | r2a | r2b |
|---|---:|---:|
| 最终聚落建筑 | 28 | 31 |
| 门前楼梯 | 0 | 23 |
| AccessGraph 连通 | 35 | 38 |
| AccessGraph 断点 | 0 | 0 |
| 严格入口拒绝 | 7 | 4 |

38 个连通节点中，15 个为普通地面路线，23 个为
`walkable_route_with_stairs`。

剩余 4 个拒绝：

- 2 个 `no_walkable_route_within_budget`；
- 1 个 `entrance_clearance_blocked`；
- 1 个 `entrance_step_requires_engineering`。

前三类多处于 4–7m 的坡差或超过 4m 的入口引道，需要 switchback 楼梯、
平台分段、挡墙和排水，不适合单跑直梯。

## 验证

- `linked_stairs_connect_a_stepped_threshold` 单测通过；
- `scale-audit pass`；
- `quality-report pass`；
- `visual-feedback pass`；
- 23 个楼梯实体通过 `stairs` 语义契约；
- 市场入口后需求重算仍为 31 栋最终需求，覆盖约 100%。

## 仍开放

- 超过 2.4m 的 switchback 楼梯；
- 平台化坡面、挡墙和排水沟；
- 4 个剩余入口拒绝；
- 45°/135° 普通道路 oriented carve；
- 楼梯与道路 owned band 冲突时的更细所有权账本。

