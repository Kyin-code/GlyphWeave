# B-02 real frontage supply closure (2026-10-20)

状态：**B-02 的 24 个沿街需求已在真实 512m 世界中全部 realized-with-access**。
这关闭了长期存在的 “planned 24 / road audit fail / realized 0” 缺口。

## 1. 控制

基础输入：

`examples/morphology/b04-c1-compact-mixed-block.manifest.json`

控制覆盖：

- `automaticRoadExtensions.scenes[0].segmentLengthsM = [256, 128, 64]`
- `frontageProgram.requests[*].stationSpacingM = 16`
- `frontageProgram.requests[*].setbackM = 8`
- `morphologyCoverage = "audit"`

覆盖由 CLI 回归测试在内存中应用，不改写已入库的基础 Manifest：

`bevy/crates/cli/tests/service_coverage.rs::compact_supply_control_realizes_24_frontage_with_a_mixed_hierarchy`

未提高 `reachDistanceM`，未放宽 6m 填方、8% 纵坡、25% 可通行面、2m
入口、4m 岸线净距或 2m 切方上限。

## 2. 实测

独立 `quality-report`：

| 项 | 值 |
|---|---:|
| frontage requested / planned | 24 / 24 |
| frontage realized with access | **24 / 24** |
| homes / shops | 16 / 8 |
| frontageAudit | **pass** |
| roadNetworkAudit | **pass** |
| roadTierAudit | **pass** |
| accessGraph disconnectedEntities | **0** |

自动路：

| 项 | 值 |
|---|---:|
| acceptedSegments | 3 |
| 实际 edge tier roles | collector, local, local |
| profile collector width | 16m |
| profile local width | 12m |
| parent chain | 由 `roadTierAudit` 独立验证并通过 |

工作证据：

- world：`bevy/work/rk-x/b02-road-supply-s8-20261020/world/`
- manifest（由测试覆盖生成）：`bevy/work/rk-x/b02-road-supply-complete-20261020/control.manifest.json`
- 回归测试同时执行 `quality-report`，直接断言上述五个审计值。

## 3. 为什么能闭合

原控制有两个独立问题：

1. 64m 站距候选与 24m station spacing 只提供很少可建站位；
2. 12m 退界让部分 shop 的入口检查落到相邻道路，形成
   `frontage building missing/changed/disconnected`。

加入 256m 道路候选后，legacy road stage 能产生一条 collector 并保留
本地街；把 station spacing 收紧到 16m 增加合法站位；8m 仍满足合同最小退界，
同时使入口回到被指派道路。这里改变的是输入工程选择，不是验收阈值。

## 4. 边界

- 该 B-02 控制仍有 compact profile 的 landcover/开发尾差警告，因此不是
  “整张世界 gate pass”；B-03 另由 complete-ground 控制闭合。
- `unserved target` 仍会在有限道路预算/不可行候选时显式记录，不把道路中心
  接近冒充整片开发。
- 本控制证明 B-02 的道路供给、层级身份和临街访问链同时成立。
