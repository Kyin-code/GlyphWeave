# B-01 结果：独立地块与真实庭院

状态：`partial`（工程接口 ready，CLI 24 目标未全实现）。

## 实现

- 新增 `worldgen/parcel_geometry.rs`：显式 `frontM/sideM/rearM`、legacy 2/2/2、四朝向非对称边界、偶数/上限/128m/setback 校验。
- `frontageProgram.requests[]` 支持可选 `parcelInsets`；`RoadsideSite` 记录来源参数；选址/预检用完整 parcel 检查场景、用途、道路/实体冲突和水体。
- `development::plan/validate` 从同一几何重算 parcel，不再硬编码建筑外框 +2m；`FrontageParcel` 记录 insets，building/yard/sourceZones 继续精确守恒。
- landcover/ledger 继续消费 Yard 单元，真实庭院面积只来自已核验 mesh。

## 正负例

- `parcel_geometry`：12×10 + 4/4/12 在四朝向得到 20×26m、520m²；legacy 与显式 2/2/2 等价；奇数/越界/setback/溢出拒绝。
- `development`：building 120m²、yard 400m²、跨 zone 260+260m²；篡改 boundary 被独立重建拒绝。
- `frontage`：显式 insets 被保留；相邻房屋不碰但 parcel 重叠时以 `occupied_or_road_support` 拒绝；frontM > setbackM 拒绝。
- 纯 Rust 长 profiled-road fixture 实现 16 homes + 8 shops = 24 parcels，全部 520m²。

## 512m CLI 控制

- 成功：`bevy/work/collaboration/ds-b-run/b01/control-e2e-1`，generate exit 0，worldRevision `c6ee653e...`。
- 实际：frontage 5/24 realized，19 unfilled；development 27 buildings，yard unresolved 289m²；ledger 仍 incomplete。
- 失败保留：`control-e2e-2`（显式走廊撞设施预留）、`control-e2e-3`（填方上限 7040mm > 6994mm）。
- 原因：当前低地样例的 profiled 临街供给不足；没有通过换 seed、缩小 12×10 楼体、减少 24 请求或放宽工程约束来通过。

## 结论

纯几何与模块合同已实现并通过；CLI 24 个目标未全实现，B-01 内容状态为 partial。
