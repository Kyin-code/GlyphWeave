# 入口拒绝后的市场需求重算

日期：2026-10-21  
控制世界：`bevy/work/rk-x/hk-central-natural-evolution-1km-r2a/`

## 问题

市场计划发生在建筑最终入口审计之前。香港切面规划了 35 栋建筑，其中 7 栋因
严格入口合同被拒绝；旧报告仍把这 7 栋计为市场需求点，所以市场覆盖率只描述
“计划需求”，不描述最终可进入城市的实际需求。

## 修改

新增 `market_catchment::reconcile_after_access`：

1. 以最终实体 ID 集合过滤规划建筑；
2. 重新把实际建筑映射到 route graph 节点；
3. 固定已经物化的市场，重新计算每个市场的容量分配、负载、最近市场、
   步行时距、局部供给密度和缺口；
4. 以重算结果覆盖运行时 `market-catchment-plan.json`；
5. 原始计划另存为 `market-catchment-plan-pre-access.json`，便于对照；
6. `market-materialization-report.json` 同时记录规划需求建筑数和最终需求建筑数。

市场位置不再重新选择，因为市场已经完成 footprint 去重叠、入口审计和物化；
本模块只校正需求与供给关系，避免在报告阶段偷偷移动已发布市场。

## r2a 结果

| 项 | 入口前计划 | 入口后重算 |
|---|---:|---:|
| 需求建筑 | 35 | 28 |
| 总需求 | 2,834.23 | 1,972.88 |
| 已服务需求 | ≈2,834.23 | ≈1,972.88 |
| 覆盖率 | ≈100% | ≈100% |
| 市场 | 6 | 6 |
| 状态 | pass | pass |

世界级审计：

- `scale-audit pass`；
- `quality-report pass`；
- AccessGraph 35 connected / 0 disconnected；
- `visual-feedback pass`；
- 实体仍为 2,973，市场与建筑位置不变。

## 审计文件

- `scenes/scene-0/market-catchment-plan-pre-access.json`：初始规划口径；
- `scenes/scene-0/market-catchment-plan.json`：最终实际需求口径；
- `scenes/scene-0/market-materialization-report.json`：市场实体和需求建筑对照；
- `quality-report.json`：最终结构、AccessGraph 和视觉门禁。

## 仍开放

市场供给重算解决了报告口径，但没有解决 7 个被拒建筑本身。它们仍需要真实
landing + stairs/switchback + retaining/drainage，或被明确规划为不可建地块。

