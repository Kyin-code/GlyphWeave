# B-04 morphology + service coverage (2026-10-20)

状态：`serviceRadiusM` 与 `coverageRatio` 均已有真实消费者；B-04 profile-driven
核心完成，几何生成仍以作者尺寸优先，coverage 是独立审计而不是静默缩放。

## 1. 控制

- manifest：`examples/morphology/b04-village-field-coverage.manifest.json`
- output：`bevy/work/rk-x/b04-service-coverage-20261020b/world/`
- 生成：exit 0，debug 约 17s
- 开关：`style.serviceCoverage = "audit"`（旧 morphology 世界不带该键时不新增文件）
- coverage 控制：同一 compact 供应控制使用 `morphologyCoverage="audit"`、
  `widthM=18`、`stationSpacingM=16`、`setbackM=8`

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
- `coverageRatio`：由 opt-in `morphologyCoverage="audit"` 消费，输出
  `morphology-coverage-audit.json`。它测量真实 parcel/building footprint，
  按 profile 范围逐地块命名 `outOfRangeParcelIds` 与 `unrealizedParcelIds`；
  不悄悄缩小已写明的建筑尺寸。

## 4. 代码

- `bevy/crates/core/src/settlement/service_coverage.rs`
- `bevy/crates/core/src/worldgen.rs` 写 `service-coverage-audit.json`
- `bevy/crates/cli/tests/service_coverage.rs`
- `morphology_compile.rs` 的 source report 与 digest 更新

## 5. 未完成

1. coverageRatio 当前是审计消费者，不是生成器强制缩放；若要让 profile 直接
   改建筑密度，需单独设计“作者尺寸 vs profile 建议”的冲突合同。
2. serviceRadiusM 目前是几何覆盖审计，不是人口/通勤/容量模型。
3. compact-mixed 供应控制使用作者尺寸 18×10m 后，
   `weightedCoverageRatio` 落入 profile 的 `0.45..0.75` 范围，24 个地块
   全部在范围内且 `status=pass`；village-field 控制有 18/18 户 + 4/4 服务。
4. 尚未在 1km / 最大图上重跑 B-04 coverage。
