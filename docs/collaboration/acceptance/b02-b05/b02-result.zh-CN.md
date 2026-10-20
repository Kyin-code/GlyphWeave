# B-02 结果：需求驱动道路层级与可达性

状态：`partial`。当前详细审计见
[`rk-x/B02-CURRENT-AUDIT-20261020.zh-CN.md`](../../rk-x/B02-CURRENT-AUDIT-20261020.zh-CN.md)。

## 已实现

- `automaticRoadExtensions` v1/v2/v3；v3 使用真实 frontage parcel realization 测量供给。
- collector/local/farm_access 层级与 8/12/16m 宽度链进入 road profile、entity、development 和独立道路审计。
- `supply_states` 现在枚举一个 purpose 的全部授权 tier；新增单测
  `supply_states_consider_every_authorized_tier`，证明 urban 同时产生 12m local 和 16m collector 候选。

## 当前实测

- B-01 双 urban 网格控制：9/24 frontage（homes 6/16、shops 3/8），自动路 3 条 8m。
- B-02 双 urban 网格 v3 控制：baseline 16m collector + 8m farm_access，无 12m local；
  v3 sweep 已含 12m/16m，但 accepted 0，frontage 0/24。
- 1km v3 控制触发 `invalid millimetre chunk dimensions` panic，保留为负例。

## 结论

道路层级状态和宽度工程链已存在；真实两级拓扑仍被 B-01 临街供给、tier provenance 缺失和
final audit gate 未接入阻塞。下一步先修 B-01 CLI 供给，再补 tier 持久化与最终审计门禁。
