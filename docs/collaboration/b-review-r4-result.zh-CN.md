# B Review R4 结果：联合评估执行与工程边界

状态：`partial`。R3 平地正例保留；新增非零工程、持久化重读和原自然有界联合评估。

## 分类结果

| 项目 | 状态 | 说明 |
| --- | --- | --- |
| syntheticControl | pass | R3 合成 flat/urban 24/24/24 保留 |
| nonzeroEngineeringControl | pass | natural 0m、final 0.1m，24 住宅/店铺、yard 与 access 通过 |
| persistedPositive | pass | 产物落盘后重新读取，生产审核再次通过；破坏 yard 被拒绝 |
| originalNaturalCase | unresolved | 原自然固定节点合同下没有找到 24 个完整 parcel 见证 |
| naturalJointEvaluation | executed-unresolved | 实际执行 4 个联合状态；全部 plan_rejected |
| cliIntegration | not proven | CLI 仍未表达合成 raster/zone 或产生 24 原自然地块 |
| jointPlanner | pass/limited | 受信 base 合同 + 完整下游审核；不是全局优化器 |

## J1/J2/J3 修复

- `TrustedBaseContract` 从可信 base plan 的 nodes、level/profile 和 source natural hash 构建；候选 plan 不能自生成 fixed map。
- 缺失 fixed node、base node 0m 被要求 8000mm、natural hash 错误、同共享节点端点高度冲突均拒绝。
- `JointAudit` 保留 frontage/development/landcover 的 status/failures；required access 以 requiredConnected/requiredUnconnected 和 ID 列表统计，optional connected 不能抵消。
- `extensionCount` 与 attempted state count 分列，不再把道路条数冒充搜索状态数。
- `Ok` 只表示评估完成；本结果不把它写成 engineering pass 或可 bake。

## 非零工程正例

合成 fixture 的 natural 为 0m，候选 road profile 抬高 100mm，再由生产 `plan_terrain_carves` + `apply_terrain_carves` 生成 final terrain。natural/final hash 不同，最大差量约 0.1m；最终仍在 final terrain 上执行 AccessGraph 和 yard。

结果保持：

- 16 住宅 + 8 店铺
- frontage 24/24/24，unfilled 0
- 24 个完整 520m² parcel
- yardUnresolved 0
- 16m collector + 12m local 两级真实 profile/entity
- requiredUnconnected 0

## 持久化正例

`bevy/work/collaboration/ds-b-r4/persisted-positive/` 保存 road plan、development plan、landcover plan、surface 和 entities。测试重新读取这些 JSON，并再次调用生产 `audit_joint_plan`、`landcover::verify`；删除一个 yard patch 后审核拒绝。

## 原自然联合评估

使用原始 `examples/b-r1-control.manifest.json`、已有 natural raster 和固定 base node 合同，不换 seed、不修改需求或 fill：

```text
attemptedStates = 4
maxStates = 256
extensionCount = 0 for every rejected state
bestLegalCandidate = null
status = unresolved
```

状态：

- 8000mm：station45 `requiredRoadElevationMm=7120 > allowedRoadElevationMm=7040`。
- 7000/6000/5000mm：设计可过但接回原始 8000mm 固定父节点时 `junction height mismatch`。

这说明 R2 的“独立 7000mm design”确实不能替代原自然连通条件。完整表见 `natural-joint-evaluation.json`。

原 R2 `diagnostic-table-r2.json` 的 `requiredFillMm` 字段没有修改其字节；本轮明确纠正语义：它是 requiredRoadElevationMm，不是填方厚度。原文档中的 z224“北侧”/z160“南侧”标签也纠正为实际的 **z224 南侧 / z160 北侧**越界行。

## 验证

- `cargo test -p glyphweave-core worldgen::road_parcel_feasibility`：6 passed / 2 ignored / 0 failed。
- 两个 ignored 小集成分别独立执行：persisted-positive 通过；original-natural-joint-evaluation 通过并产出 JSON。
- `pnpm doc-tree:check`：PASS。
- `git diff --check`：exit 0，仅既有 CRLF 提示。
- 未运行完整 workspace/core、release、最大图或浏览器。
- 本轮新 512m CLI 生成 0 次；gate/maximum/browser 未运行。

## 未完成

- 原自然案例仍 unresolved，没有 24 个合法完整 parcel 见证。
- CLI 仍不能表达合成 test raster/zone，也不能证明原自然 24/24。
- 最大图和 240 块浏览器仍须在后续准入满足后进行。
