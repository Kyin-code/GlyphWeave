# B-03 residual audit（2026-10-20）

状态：`partial_content`。本轮只增加 residual 账本，不改变 residual 几何或旧世界字节。

## 1. 实现

- `LandUseLedger` 新增可选 `residual: ResidualAudit`；无 `residualLandProgram` 的世界不序列化该字段。
- 账本按 `sourceZoneId + subUse` 输出：
  - `candidateM2`
  - `targetM2`
  - `realizedM2`
  - `policyRemainderM2`
  - `rejectedAttemptM2`
  - `unresolvedM2`
  - 逐原因 `rejections`
- `realizedM2` 从真实 `SurfacePatch` 的 `mesh_area` 复算，不信任发布字段 `areaM2`。
- 不变量：`candidate = realized + unresolved`；`rejectedAttempt` 是 unresolved 的子集；`policyRemainder` 不是第二个缺额，可能大于 unresolved（realized 可以超过 target），不能相加。
- 新单测：`residual_audit_splits_policy_remainder_and_rejections_from_unresolved`。

## 2. 当前 b03 控制实测

- manifest：`examples/b03-land-execution.manifest.json`
- 输出：`bevy/work/rk-x/b03-residual-20261020/world/`
- `generate-world` exit 0，2.633s，58 entities，worldRevision `ddc5d27d...`
- ledger：realized **133,602m²** / unresolved **128,542m²**
- residual 总计：
  - candidate **229,568m²**
  - target **87,314m²**
  - realized **101,026m²**
  - policy remainder **142,254m²**
  - rejected attempts **2,242m²**
  - unresolved **128,542m²**

代表性账户：

| source zone / sub-use | candidate | target | realized | unresolved |
|---|---:|---:|---:|---:|
| urban.5 / urban_public_open_space | 13,916 | 1,388 | 1,388 | 12,528 |
| rural.13 / rural_pasture | 13,724 | 2,738 | 8,346 | 5,378 |
| rural.14 / rural_pasture | 15,412 | 3,077 | 6,829 | 8,583 |
| nature.3/12/15 / conservation | 16,384 each | 16,384 each | 16,384 each | 0 |

`policyRemainder` 可以大于 `unresolved`：例如 rural_pasture 的 realized 可以超过 policy target，因为同一 rural cell 内的耕地/田埂/农路也属于农业实现。它不是第二个缺额。

## 3. 口径说明

- 旧 `land-execution-2` 的 123,760m² 是旧 worldRevision 的历史值；当前 HEAD 重烘焙后为 **128,542m²**，两者不能混用。
- 当前 b03 控制仍是 512m，不是 1km。
- nature 已全部实现；urban 和 rural 的未实现量是主要缺口。

## 4. 仍未完成

1. 显式 residual 子用途 v2：`urban building reserve`、`rural village public space`、`rural cropland margin`。
2. 农村公共空间没有真实 `PublicOpenSpace` patch 和 ledger 行。
3. B-03 控制没有启用 `trackNetwork: shared-farm-track-v1` 或 `fieldShapes: contour-elongated`。
4. `developmentAudit.unplannedM2` 与 landcover residual 的映射表仍未输出。
5. residual 分配仍是同一实现重放，不是独立算法；全局 1m patch 两两重叠审计仍缺。
6. `unassigned` 在当前 512m 控制没有形成独立分区。

## 5. 证据

- `bevy/work/rk-x/b03-residual-20261020/bake.log`
- `bevy/work/rk-x/b03-residual-20261020/world/scenes/scene-0/land-use-ledger.json`
- `bevy/work/rk-x/b03-residual-20261020/world/scenes/scene-0/landcover-audit.json`
