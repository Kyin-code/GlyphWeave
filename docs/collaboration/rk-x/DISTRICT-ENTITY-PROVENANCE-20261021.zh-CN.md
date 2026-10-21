# 新旧城时代从计划进入最终实体

日期：2026-10-21

## 1. 缺口

`settlementEvolution.districtDemand` 已能按 old-core、renewal、new-growth、
periphery 分配需求，`settlement-evolution-plan.json` 也记录了每栋建筑所属
`districtStage`。但香港 1km 最终 `scene.json` 中，所有同源建筑的
`grounding.districtId` 与 `grounding.constructionCohort` 都是 null。

结果是：

- HTML、Godot 和审计只能读计划侧分期，不能从最终实体确认年代；
- 街道墙、拆合、材质更新与入口工程无法稳定绑定同一时代；
- 新老城容易退化成只改颜色或只改楼高。

## 2. 修复

聚落物化时现在把计划字段写入实体：

```text
districtId          = "district." + districtStage
constructionCohort = districtStage
```

该字段与既有 `GroundingSpec` 共用序列化契约，因此 Core、HTML 和 Godot
读取同一份最终实体数据。

HTML 预览在建筑基座增加 0.55m 时代基座：

| cohort | 基座色 |
|---|---|
| old-core | 暗砖红 |
| renewal | 赭黄 |
| new-growth | 冷灰蓝 |
| periphery | 灰绿 |

基座只是可读性提示，不是分期证据；分期证据仍是实体 `districtId/cohort`
与计划文件的对应关系。

## 3. 香港 r3d 验证

```text
bevy/work/rk-x/hk-central-oriented-road-1km-r3d
```

| cohort | entities |
|---|---:|
| old-core | 10 |
| renewal | 15 |
| new-growth | 9 |
| periphery | 0 |

34 栋通过入口并物化的建筑全部具有非空时代字段。`periphery` 没有建筑，
因为当前核心 1km 切面的外围容量/入口没有形成合法落点；这里保留 0，
不以全局回填或伪造实体凑数。

回归：

```text
entities = 3010
AccessGraph = 41 connected / 0 disconnected
stairs = 25
markets = 6
scale-audit = pass
quality-report = pass
```

新增单测将 `district_budgets_reserve_epochs_and_spill_only_unused_demand`
从“计划有 stage”提升为“物化实体逐栋携带同一 districtId/cohort”。

## 4. 边界

本模块没有实现：

- 连续街墙与店面贴线率；
- 同一地块父/子拆合；
- 旧城保护与新区拆建的几何操作；
- TOD/站城公共空间组合；
- 时代材质资产替换。

这些后续模块现在可以直接读取最终实体时代字段，而不需要再次从计划反推。
