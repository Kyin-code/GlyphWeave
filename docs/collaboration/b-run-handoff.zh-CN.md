# Milestone B 统一交接报告

状态：`partial`。未签收 B，未推进 C。

## 基线

- WORK_BASE：`fb36dc4f9fdd38e6aa9c5a89676a65b96e5001b1`
- A SOURCE_BASE：`42cb4973fe6fdf985d21ee2a1f4be3c0ce10382b`，确认是 HEAD 祖先。
- 未执行 Git 写操作；F 盘未改；A 冻结世界/runtime/证据未改。

## 阶段结论

| 阶段 | codeReady | contentStatus | 关键结果 |
| --- | --- | --- | --- |
| B-01 | pass | partial | 20×26/520m² parcel、真实 yard/ledger、纯 Rust 24/24；CLI 5/24 |
| B-02 | pass | partial | 8/12/16m 层级接口与选择测试；小场景实际仅 8m、frontage 4/24 |
| B-03 | pass | partial_content | public open space 1293m²、pasture 32386m²；landcover unresolved 123760m² |
| B-04 | partial | partial | 六 profiles、服务关系、48 条案例索引；参数未完整展开 |
| B-05 | not_run | partial | workspace/Node/前端回归通过；最大图/浏览器未准入 |

## 关键产物

- 代码：`parcel_geometry.rs`、`morphology.rs`、`residual_land.rs`、`service_relations.rs` 及 frontage/road/development/landcover/ledger 增量。
- 样例：`examples/b01-parcel-program.manifest.json`、`b02-road-supply.manifest.json`、`b03-land-execution.manifest.json`、`morphology/b04-village-field.manifest.json`。
- 结果：`b01-result`～`b05-result`、`b04-case-register`。
- 证据：`bevy/work/collaboration/ds-b-run/`。

## 未解决

- 24 个 B-01 frontage 目标在低地 CLI 控制只实现 5（B-02/B-03 中为 4）。
- B-02 没有在真实小场景形成 collector/local 两级实网；882 最大图目标未验证。
- B-03 城市/农村仍分别有 11999/111761m² unresolved（1km 样例口径）。
- B-04 profile 数值尚未完整驱动 frontage/services/道路展开；案例资料仍有 heuristic/source_pending。
- 最大图、独立 gate、release runtime、240 块浏览器巡检均 not_run。
- 历史 Quaternius 与 workerd 许可未解析项原样保留。

## 建议下一步

先修复低地 profiled 道路供给/临街站与地块冲突，使一个 ≤1km 控制达到 B-01 24/24 和 B-02 两级实网，再展开 B-04 profile 参数；准入满足后才运行最大图和浏览器。
