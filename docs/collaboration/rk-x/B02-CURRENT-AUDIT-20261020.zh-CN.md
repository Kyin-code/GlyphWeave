# B-02 当前审计（2026-10-20）

状态：`supply-closed`。2026-10-20 后续控制已实现 24/24 realized-with-access；
见 `B02-SUPPLY-COMPLETE-20261020.zh-CN.md`。本文件保留此前的只读审计和失败路径。

## 1. 已实现

- `RoadHierarchy`：collector/local/farm_access，宽度 8/12/16m，显式授权。
- `automaticRoadExtensions` v1/v2：按 zone 目标、距离收益、服务数量选路；旧 v1 语义保持不变。
- `automaticRoadExtensions` v3：`demand-led-realized-parcel-supply-v3`，按真实 frontage parcel realization 测量供给。
- 8/12/16m 工程宽度链已进入 `RoadProfile`、road entity、development 占地和最终道路审计。
- `supply_states` 现在枚举一个 outstanding purpose 的**全部**授权 tier，而不是只取第一个 tier；新增单测
  `supply_states_consider_every_authorized_tier`。

## 2. 两个可复现控制

### B-01 双 urban 网格控制

- manifest：`examples/b01-parcel-program-512-two-district.manifest.json`
- 输出：`bevy/work/rk-x/b01-512-two-20261020/world/`
- 2 个 urban 网格；自动路 3 条 8m。
- frontage：homes 6/16、shops 3/8，合计 **9/24**。
- 结论：比单 urban 网格的 5/24 好，但仍未达到 B-01 的 24/24；自动路只服务 zone 中心，不按临街供给。

### B-02 双 urban 网格 v3 控制

- manifest：`examples/b02-road-supply-512-two-district.manifest.json`
- 输出：`bevy/work/rk-x/b02-512-two-20261020/world/`
- 2 个 urban 网格；baseline 生成 16m collector + 8m farm_access，没有 12m local。
- v3 sweep 已同时出现 12m 和 16m 候选（修复前只有 12m），但 128 个状态内没有真实 parcel supply，
  v3 accepted 0。
- frontage 仍为 **0/24**；拒绝项包含 `wrong_purpose_or_protected`、`existing_support_changed`、
  `natural_support_or_fill_limit`、`occupied_or_road_support`。
- 结论：层级状态可见性已修复，但 B-01 临街供给和最终道路审计仍是上游阻塞。

## 3. 1km 负例

- 本地负例 manifest（未入库，SHA-256 `8C893F9CAAADA5FED37778912B6F34BB8C84068F2F9C87939B21FE950511B56B`）
- 现象：`generate-world` 在 `worldgen.rs:4357` panic：
  `bounded metre source produces valid millimetre payload: invalid millimetre chunk dimensions`。
- 这不是道路宽度错误，而是 1km + v3/控制组合触发的 chunk 编码边界；保留为负例，修复前不得作为生产控制。

## 4. 仍未完成

1. `RoadExtension`/`RoadEdge` 没有持久化 tier role/id；产物只能看到宽度，不能证明 collector/local/farm 的父子拓扑。
2. v2 自动路没有把 frontage/农田真实需求作为 target；仍只按 zone 进入/中心距离停止。
3. v3 的 128 状态上限与排序虽已加入 tier rank，但真实 parcel supply 仍被 B-01 供给不足卡住。
4. 16m collector 的最终 fill audit 仍未作为候选提交前的硬门禁；当前可被计划接受后在最终 audit 失败。
5. 缺少 `bevy/crates/cli/tests/road_supply.rs` 的 CLI 级父链/收益/身份回归。

## 5. 下一步

先修 B-01 CLI 临街供给（station/parcel/支撑/用途冲突），让至少一个控制从 9/24 提升；
再把 tier provenance + final audit gate 做成独立小提交；最后用 v3 append 证明
collector → local → farm_access 的真实父链和至少一个原未满足地块的可达性增量。

## 6. 证据

- B-01 控制：`bevy/work/rk-x/b01-512-two-20261020/bake.log`
- B-02 v3 控制：`bevy/work/rk-x/b02-512-two-20261020/bake.log`
- B-02 v3 单区对照：`bevy/work/rk-x/b02-v3-20261020/bake.log`
- 1km 负例：`bevy/work/rk-x/b02-1km-v3-20261020/bake-r5.log`
