# B-02 结果：需求驱动道路层级与可达性

状态：`partial`。

## 实现

- `automaticRoadExtensions` 新增 version 2 和显式 `hierarchy`：collector/local/farm-access，宽度仅 8/12/16m，旧 version 1 与 8m 语义不变。
- 候选优先按真实未满足目标、zone 进入、中心距离收益、服务数量、层级排序；预算仍为有限候选/段数。
- `RoadProfile`、road entity、开发占地、保护/水体/支撑和独立审核统一消费显式宽度；没有另建脱离工程高程的路网。
- 新样例 `examples/b02-road-supply.manifest.json` 选择 `maxNewSegments=64`、`maxCandidateAttempts=16384`，原 A manifest 未改。

## 测试

- `auto_roads`：旧版确定性/预算/保护回归继续通过；新 hierarchy 选择 collector 16m 并落成真实 entity/profile。
- `road_extensions`：8/12/16m 均进入 entity 与 profile；10m 被拒绝。
- 回归修复：自动候选包围盒恢复原纵向 `margin`，只按真实宽度扩大横向支撑；A frontage 测试恢复 5/8 行为。

## ≤1km 生成

- `bevy/work/collaboration/ds-b-run/b02/road-supply-1` generate exit 0，worldRevision `0156ab19...`。
- automaticRoads accepted 5，unservedTargets 0；最终 frontage 4/24 realized，20 unfilled。
- 实际新增段均为 8m farm-access；collector/local 的层级选择已由单元测试证明，但该小场景没有形成两级实网。

## 结论

道路宽度/层级接口与安全链已实现；小场景内容供给仍不足，882 目标覆盖未证明，状态 partial。
