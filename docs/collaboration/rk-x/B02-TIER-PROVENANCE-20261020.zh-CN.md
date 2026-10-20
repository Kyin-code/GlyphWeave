# B-02 tier provenance and final road audit (2026-10-20)

状态：`tier provenance + final audit gate` 已实现；B-02 仍未整体签收
（v3 真实临街供给仍为 0，frontage 0/24）。

## 1. 新增的持久化字段

`RoadEdge`、`RoadExtension` 和道路实体的 `grounding` 现在都能带：

```json
{
  "id": "collector-16",
  "role": "collector"
}
```

位置：

- `roadNetworkPlan.edges[].tier`
- `roadNetworkPlan.extensions[].tier`
- 道路实体 `entities[].grounding.roadTier`

三者必须逐字段相等；老世界没有该字段时序列化形状不变。

## 2. 最终硬门

`auto_roads::audit_tier_provenance(plan, report)`：

- 每条 `generatedRoadIds` 必须有 edge、extension 和 tier；
- tier id 必须来自同一 `automaticRoadExtensions.scenes[].hierarchy`；
- role 必须一致，extension 的 road width 必须等于 tier width；
- edge.tier 与 extension.tier 必须相等；
- parentRoadIds 非空，且每个 parent 必须是已存在 edge；
- 递归检查 v3 append 的 base road program，不能把 baseline 误判为无 tier。

生成期在 legacy auto phase、v3 supply phase 和 `commit_decision` 都调用该审计。
`quality-report` 额外输出 `roadTierAudit`；`world-quality-gate` 对它非 pass 时
输出 `final road tier provenance failed independent audit`。

## 3. 6.4m 填方误报的修复

512m 控制曾出现：

```text
road generated.road-auto.scene-0.0 changed after planning:
design error 0m at (224,160), fill 6.4m, cut 0m
```

实测该道路自己的 authored profile 最大填方是 5.7m；6.4m 来自相邻
carve/transition 的最终地表抬高，却被算在道路头上。现在道路审计用
**道路自己的 profile target − natural** 计算道路土方合同，最终地表仍用于
验证 pavement/design error。修复后：

| 指标 | 修复前 | 修复后 |
|---|---:|---:|
| roadNetworkAudit | fail | **pass** |
| realizedEdges / plannedEdges | 32/33 | **33/33** |
| maxFinalRoadbedFillM | 6.400000095 | **5.699999809** |

这不是放宽 6m 合同：`append_one` 仍按同一 `max_fill_mm` 拒绝；
只是不再把邻居的填方重复记到道路上。

**发布字节说明**：`assigned_road_access` 的 plains 冻结摘要从
`11780375…` 移到 `4e733ac8…`。移动的是修正后的 road audit 证据，不是
道路/地形几何；该测试仍逐文件比较整棵树，防止其他字段漂移。

## 4. 512m 控制实测

manifest：`examples/b02-road-supply.manifest.json`（v2 hierarchy）

| 项 | 值 |
|---|---|
| acceptedSegments | **2** |
| generated tiers | `collector-16`, `farm-access-8` |
| unservedTargets | **0** |
| 真实可达改善 | rural.0 / rural.4 / urban.5 从无路到有路，距离 188/188/92 → 60/56/24 |
| roadNetworkAudit | **pass**, 33/33 |
| maxFinalRoadbedFillM | 5.7m |
| frontageAudit | **0/24 planned**（仍是 B-01/B-02 供给阻塞） |

双 urban 网格 v3 控制：legacy baseline 仍生成 collector/farm 两条带 tier 的
道路；v3 supply `acceptedSegments=0`，`candidate_state_budget_exhausted`；
frontage 仍 0/24，roadNetworkAudit pass。

## 5. 测试

- core 单测：`explicit_hierarchy_selects_actual_collector_and_local_widths`
  断言 edge/extension/entity tier 一致、parent 存在、篡改 role 被拒。
- CLI：`bevy/crates/cli/tests/road_supply.rs`
  - 512m 控制 accepted > 0、unserved 0；
  - base → collector/farm parent 链可从发布计划重建；
  - roadNetworkAudit pass；
  - 篡改 parent 后 world-quality-gate 必须输出 tier provenance failure。

## 6. 未完成

1. 真实 frontage parcel supply 仍为 0/24；B-02 不能签收。
2. v3 在 128 状态预算内仍找不到真实 parcel supply；需要 B-01 临街供给
   或候选状态预算/轮转策略的进一步工作。
3. `world-quality-gate` 的 landcover CLI 重算缺口另见
   `ISSUE-cli-landcover-gate-trust.zh-CN.md`。
