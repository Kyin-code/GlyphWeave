# B-04 morphology + service coverage (2026-10-20)

状态：`serviceRadiusM` 已成为真实消费者；B-04 仍未签收（coverageRatio 仍无消费者）。

## 1. 控制

- manifest：`examples/morphology/b04-village-field-coverage.manifest.json`
- output：`bevy/work/rk-x/b04-service-coverage-20261020b/world/`
- 生成：exit 0，debug 约 17s
- 开关：`style.serviceCoverage = "audit"`（旧 morphology 世界不带该键时不新增文件）

## 2. 实测

`service-coverage-audit.json`：

| 项 | 值 |
|---|---:|
| radiusM | **800** |
| services | **4**（school / 2 convenience_store / church） |
| dwellings | **18** |
| coveredDwellings | **18** |
| uncoveredEntityIds | **[]** |
| status | **pass** |

每个服务记录自己的 `dwellingsWithinRadius`；当前 4 个服务都在 800m 内覆盖
全部 18 户。若某户不在任何服务半径内，会以 `uncoveredEntityIds[]` 具名，
不会静默通过。

## 3. 编译报告

- `heightM`：已驱动未显式声明的 frontage 建筑高度；
- `serviceRadiusM`：已从 `unsupported` 移到 `derived`，路径
  `service-coverage-audit.json.radiusM`；
- parcel/inset/setback/road hierarchy/publicSpaceRatio/village field 仍由
  profile 编译；
- `coverageRatio`：**仍是 unsupported**，因为当前没有不破坏既有
  parcel/footprint 合同的实际消费者。

## 4. 代码

- `bevy/crates/core/src/settlement/service_coverage.rs`
- `bevy/crates/core/src/worldgen.rs` 写 `service-coverage-audit.json`
- `bevy/crates/cli/tests/service_coverage.rs`
- `morphology_compile.rs` 的 source report 与 digest 更新

## 5. 未完成

1. coverageRatio 还没有生产者/消费者；不能为了“字段已接”去悄悄缩小已写明的
   建筑尺寸。
2. serviceRadiusM 目前是几何覆盖审计，不是人口/通勤/容量模型。
3. 尚未在 1km / 最大图上重跑 B-04。
