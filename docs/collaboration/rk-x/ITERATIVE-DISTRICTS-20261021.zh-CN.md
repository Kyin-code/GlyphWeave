# 迭代式新老城区控制（2026-10-21）

## 1. 交付范围

这是新老城区要求的**首个可执行垂直切片**，不是完整城市历史、人口或地产经济模拟。

Manifest：

`examples/iterative-new-old-city-512.manifest.json`

世界：

`bevy/work/rk-x/iterative-new-old-city-512-v5-20261021/world`

中心：`(320, 320)`。城区由三个有序建设阶段组成：

| 阶段 | cohort | 半径 | 需求倾向 |
|---|---|---:|---|
| `old-core` | `established` | 0–110m | 商业优先，保留少量住宅与高层 |
| `renewal-ring` | `renewal` | 90–175m | 商业、住宅和更新型公寓混合 |
| `new-ring` | `new` | 155–265m | 住宅与公寓块为主，仅保留邻里商业 |

环带刻意互相重叠。真实城市更新不是硬同心圆，重叠用于让中心商业向更新带自然衰减，而不是在一条规则半径上突然变纯居住。

## 2. Core 实现

新增 `settlement/district_program.rs`：

- 读取 `style.urbanDistrictProgram`；
- 校验 1–8 个有序迭代、场景中心、半径、cohort 和请求数量；
- 将迭代展开到既有 `frontageProgram.requests`；
- 为请求附加 `centerDistanceM`、`centerWorld`、`districtId`、
  `constructionCohort`；
- 生成 `urban-district-audit.json`。

展开后的每个请求仍经过原生产链：

```text
道路候选 -> 中心距离 -> 用途/保护 -> 水体和岸线 -> 场地工程
-> 入口可达 -> 规则引擎 -> 最终 bake
```

中心距离不是第二套放置器，也不会放宽填方、坡度、退界、入口或水域检查。

## 3. 实测结果

最终控制：310 entities。

| 阶段 | 需求 | 实现 | 空额 | 平均中心距 | 实际构成 |
|---|---:|---:|---:|---:|---|
| old-core | 14 | 11 | 3 | 67.51m | 7 店 / 4 住宅 |
| renewal-ring | 18 | 11 | 7 | 128.40m | 6 店 / 4 住宅 / 1 公寓块 |
| new-ring | 24 | 9 | 15 | 189.33m | 4 店 / 4 住宅 / 1 公寓块 |

总计 31 个临街地块；商业从中心向外的数量为 7 → 6 → 4，符合当前控制设置的
中心商业衰减。外围住宅没有绕过道路、入口或工程审计。

审计：

```text
road-network-audit: pass
plannedEdges 29 / realizedEdges 29
plannedSites 31 / realizedSites 31
scale-audit: pass
quality-report: warn
```

`quality-report warn` 的原因是 955m² 庭院坡地未实现面，以及 structured-water
场景尚未接 settlement planner 的既知口径；不是道路、水体、入口或本迭代功能失败。

## 4. 视觉表现

- `constructionCohort` 从 Plan/Entity 传到 `GroundingSpec`；
- HTML 住宅与公寓块按 cohort 使用暖旧砖、混合更新和冷色新城立面；
- 战略视图读取 `urban-district-audit.json`，绘制阶段半径和
  realized/requested 图例（已实现数据入口，当前截图受窄视口影响）；
- 近景截图：
  - `preview-districts-explore.png`
  - `preview-districts-strategic.png`

## 5. 本轮同时修复的一致性问题

1. rules-engine 在 reconcile 阶段使用 `fallback=skip` 删除规划地块后，
   再执行一次有界 pad completion，避免 plan/development/carves/entity 分离。
2. post-reconcile skip 不再错误写入 `refusedEntityIds`；它由
   `inputEntityIds/outputEntityIds/skips` 三角自身证明。
3. 岸线审计只把结构化水 footprint 计为水体；低于 datum 的干燥洼地不再
   被误判为第二片湖。

## 6. 边界

- 当前表达“经过若干阶段后的静态结果”，不做逐年代拆房、产权、地价、人口迁移。
- 中心商业是显式距离分带，不是经济模型或真实城市法规。
- 新城仍有较大未满足供给；报告如实保留 `partial`，没有复制建筑补数。
- 真实转向几何、立交、完整街道断面和共享步行网络仍属于后续工作。
