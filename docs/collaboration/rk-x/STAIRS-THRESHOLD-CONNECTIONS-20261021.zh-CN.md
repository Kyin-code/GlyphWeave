# 门前静态楼梯与 AccessGraph 连接

日期：2026-10-21  
控制世界：`bevy/work/rk-x/hk-central-natural-evolution-1km-r2h/`

## 目标

真实坡地建筑的门口不能靠放宽 0.30m 台阶合同通过。先为 0.30m–2.40m
的局部门口高差生成短静态楼梯；若入口仍无道路路线，再提升为从 facade
到 frontage road point 的长直梯。AccessGraph 只使用实际楼梯实体的地面端
作为步行路线起点。

## 数据合同

`GroundingSpec.stairConnection` 记录：

- `stairId`：实际静态楼梯实体的 ID；
- `from` / `to`：门侧与地面侧世界坐标；
- `fromMm` / `toMm`：两端毫米标高；
- `lowMm` / `highMm`：楼梯总高差；
- `stepCount`：按 0.18m 目标踏步高计算的级数；
- `widthM`：楼梯宽度，本轮固定 2.4m。

门侧高差小于 0.30m 时不生成短梯；长梯总高差上限为 8m，且必须通过
实体/水体/跨度检查。仍不可达时保留具名拒绝，等待 switchback/挡墙工程。

## AccessGraph

入口审计现在：

1. 验证建筑引用的楼梯实体真实存在；
2. 验证楼梯门端与建筑 facade 距离不超过 2m；
3. 验证直梯或长直梯的水平长度不超过 24m；
4. 从楼梯地面端开始执行与普通路线完全相同的道路搜索；
5. 连通时状态写为 `walkable_route_with_stairs`；
6. 楼梯引用缺失或几何不合法时写 `stair_connection_invalid`。

楼梯不是 AccessGraph 中的“跳过门”标记，而是显式静态实体和路径点。

## r2h 结果

| 项 | r2a | r2b |
|---|---:|---:|
| 最终聚落建筑 | 28 | 34 |
| 门前楼梯 | 0 | 26 |
| AccessGraph 连通 | 35 | 41 |
| AccessGraph 断点 | 0 | 0 |
| 严格入口拒绝 | 7 | 1 |

41 个连通节点中，15 个为普通地面路线，26 个为
`walkable_route_with_stairs`。

剩余 1 个拒绝：

- `stair_connection_invalid`：长直梯到道路的路径仍被现有实体/水体/廊道合同拒绝。

短梯不足以到达道路时，入口拒绝回合会先尝试从 facade 到规划 frontage
road point 的长直梯，再回退到稳定的 facade 外推落点；任一目标仍必须通过
实体、水体和跨度检查。剩余情况需要真正的 L 形/switchback 路径与平台分段。

## 验证

- `linked_stairs_connect_a_stepped_threshold` 单测通过；
- `scale-audit pass`；
- `quality-report pass`；
- `visual-feedback pass`；
- 26 个楼梯实体通过 `stairs` 语义契约；
- 市场入口后需求重算为 34 栋最终需求，覆盖约 100%。

## 仍开放

- 剩余长梯的 switchback 路径和平台分段；
- 平台化坡面、挡墙和排水沟；
- 4 个剩余入口拒绝；
- 45°/135° 普通道路 oriented carve；
- 楼梯与道路 owned band 冲突时的更细所有权账本。

