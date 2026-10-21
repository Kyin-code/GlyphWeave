# 已知问题：斜向道路的土方 footprint 仍按轴向矩形处理

日期：2026-10-21

状态：**Core 几何已修复并接入审计；1km 重烘与视觉复核待完成。**

## 1. 事实

当前 1km 自然道路的真实方向分布：

| 方向 | 段数 |
|---|---:|
| 0° | 96 |
| 45° | 60 |
| 90° | 69 |
| 135° | 25 |
| 其他斜向 | 13 |

`route_evolution::materialize` 会生成任意角度的道路实体，但
`worldgen::carve_geometry` 对普通道路只区分：

- 0°；
- 90°；
- 其余角度按未旋转的 X 向长条处理。

因此 45° 道路的 terrain flat top、填方过渡和 `road_owns_cell` 判定可能与
实际道路实体朝向不一致。当前画面仍可读，但这不构成几何证明。

## 2. 影响

- 斜向道路的土方矩形可能侵入相邻入口；
- 入口表面可能被错误道路 fill 优先占用；
- 道路两侧建筑 support 与道路实体的空间关系可能错误；
- 仅用中心点、仅用截图或仅用 `scale-audit pass` 都不能覆盖该问题。

## 3. 不能采用的修法

- 不能把斜向道路统一转成覆盖整条斜线的超大 AABB；
- 不能继续把 45°/135° 当作 X 向；
- 不能用提高入口容差掩盖道路侵占；
- 不能删除斜向道路而退化成横平竖直路网。

## 4. 最小正确实现切片

1. `TerrainCarve` 增加可审计 orientation，或提供一个统一的 oriented
   footprint 求解函数；
2. `apply_terrain_carves`、`CarveIndex`、support/cut window、bounded-cut
   ledger 和审计全部读取同一 oriented footprint；
3. 0/45/90/135/任意角度分别有门禁测试；
4. 用“入口点不在道路 flat top 内”与“道路中心在 flat top 内”双向断言，
   防止只修中心点；
5. 对 1km 控制重烘，比较失败入口数、道路土方面积和视觉道路/地形贴接；
6. 审计报告记录每个非轴向道路的旋转、实际 polygon 与 carve owner。

在该切片完成前，斜向道路仍是 B/C 生成链的已知几何风险。

## 5. 2026-10-21 更新

上述切片 1–5 已在 Core 中接入：道路 carve 保存 yaw 与局部半长，
`apply_terrain_carves`、`CarveIndex`、bounded-cut、道路运输带和桥面审计
统一使用局部矩形判定。轴对齐 0°/90° 的既有整数边界保持不变。

尚未完成第 6 项：香港中环 1km 尚未在本模块后重新烘焙，因此不能宣称
最终入口数、土方面积和视觉结果已经验证。完整记录见
`ORIENTED-ROAD-CARVE-20261021.zh-CN.md`。
