# 任意角度道路土方与审计统一

日期：2026-10-21
状态：Core 几何、入口支撑、bounded-cut 与道路审计已接入；1km 重烘仍待执行。

## 1. 问题

自然地形路径会生成 0°、45°、90°、135° 及其他方向的直路段，但普通道路的
`TerrainCarve` 只保存世界轴半宽/半深。45°/135° 因而被当成未旋转的 X 向长条：

- 土方 flat top 与实际路面方向不一致；
- support/cut window 可能覆盖相邻入口；
- chunk 索引可能漏块或把道路膨成无关 AABB；
- 道路审计采样到 AABB 角点，可能把非路面格点误判为设计面错误。

只改渲染或只改 `carve_geometry` 会继续制造“生成、账本、审计各算一套”的漂移。

## 2. 统一几何

`TerrainCarve` 现在保存：

```text
cx/cz                 世界中心
half_w/half_d         道路局部沿向/横向半长
rotation_y_deg        与 EntityInstance 相同的 yaw
```

格点归属统一由 `oriented_outside_m` 计算：

```text
along  =  dx*cos + dz*sin
across = -dx*sin + dz*cos
outside = max(|along|-half_w, |across|-half_d)
```

世界轴 AABB 仅用于：

- `CarveIndex` 粗筛；
- chunk 相交与扫描范围；
- 生成世界坐标化的审计候选格。

最终归属仍必须回到局部矩形，不能以 AABB 直接判定道路所有权。

## 3. 已接入消费者

| 消费者 | 接入结果 |
|---|---|
| `plan_terrain_carves` | 普通道路与 graded road 均保存真实 yaw 和局部半长 |
| `apply_terrain_carves` | flat top、fill ring、road support 和 road cut 使用同一局部距离 |
| `CarveIndex` / chunk filter | AABB 只做覆盖粗筛，不漏旋转矩形 |
| `terrain_disturbance` | 道路/建筑扰动按真实朝向判定 |
| `bounded_cut` | `RoadFlatTop`、`RoadCutFootprint`、`SupportWindow` 保存 yaw |
| road transport audit | 只采样真实运输带格点，AABB 角点不再要求设计标高 |
| water-crossing audit | 桥面宽度与湿格采样按局部路面方向 |
| frontage / pad 归因 | 诊断和冲突窗口改用统一 carve 距离 |

轴对齐 0°/90° 保留原有整数取整规则，避免扩大一格造成旧合同回归。任意角度
才使用投影 AABB 覆盖局部矩形。

## 4. 新增门禁

- 45° 道路中心必须属于 flat top；
- 道路局部转角内点必须属于 flat top；
- 横向 0.25m 外邻点必须不属于 flat top；
- 旧 X 向近似下的 `(cx+17,cz)` 必须已经离开真实道路；
- 135° 道路实体进入 carve 后必须保留 135° yaw；
- 0°/45°/90°/135°/217.5° 共用同一局部矩形规则。

定向复跑已通过：

```text
oriented_road_flat_top_uses_local_axes_at_every_cardinal_diagonal
diagonal_road_carve_no_longer_owns_its_world_aabb
planned_route_road_carries_rotation_and_local_half_extents
graded_road_audit_accepts_legal_engineering_and_rejects_real_damage
g1b_preflight_acceptance
g10_preflight_identity_binding
g11_preflight_rule_identity_invariants
```

## 5. 全量回归基线

最新一次全量 Core 回归在修正审查辅助断言前为 `871 passed / 8 failed /
34 ignored`。随后两个 `auto_roads` 辅助断言已单独复跑通过；剩余 6 个失败
均位于本轮之前已改动但尚未闭合的 bounded-cut / frontage-supply /
assigned-road 场景：

- `a_forged_ledger_is_refused_by_name_and_protected_and_water_cells_never_cut`
- `a_graded_road_cut_clears_the_overfill_above_it_and_the_ledger_books_the_depth`
- `the_ledger_measures_the_cut_the_plan_dictains_and_verifies_from_rasters`
- `diagnosis_of_a_published_world_reproduces_its_frontage_report`
- `a_road_that_still_overfills_after_its_cut_is_refused_by_name`
- `a_recomputed_graph_reaches_every_assigned_street_and_the_refused_lot_is_gone`

这些失败不应被“斜向 carve 已完成”掩盖。它们需要独立核对工程账本基线，
不能通过放宽入口或土方门限消除。

## 6. 剩余工作

1. 重烘香港中环 1km 切面，比较拒绝入口、道路土方面积与入口高差；
2. 检查 HTML 道路/地形贴合与 skyline/street/waterfront；
3. 追查上述 6 个旧基线失败，区分规格过期与真实实现缺陷；
4. 把普通演化道路纳入可直接切坡的道路 profile，而不只是 fill-only carve。
