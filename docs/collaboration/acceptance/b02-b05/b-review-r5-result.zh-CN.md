# B Review R5 结果：真实工程审核接线与合法候选评估

状态：`partial`。三个工程漏检已修复；固定入口自然候选已实际评估但仍 unresolved；非零工程完整落盘包尚未完成。

## 分类结果

| 项目 | 状态 | 说明 |
| --- | --- | --- |
| engineeringNegatives | pass | 三项检查员破坏均独立拒绝 |
| syntheticControl | pass | R4 平地 24/24/24 保留 |
| nonzeroEngineeringControl | pass | natural/final 差 100mm，24/24/24 和真实 yard 通过 |
| persistedNonzeroPositive | partial | `ds-b-r5/persisted-positive-r5` 仍是平地落盘包，未完整保存非零工程输入 |
| originalNaturalCase | unresolved | 原自然固定入口候选仍无合法 24 见证 |
| naturalJointEvaluation | executed-unresolved | 4 个有意义状态全部 plan_rejected |
| cliIntegration | not_proven | 未运行原自然 bake/gate |

## 三项工程反例

新增生产审核并全部拒绝：

1. 固定 base road 中心 `[64,160]` final 高程 +1mm：`fixed base road support differs on final terrain`。
2. `extension.design.roadMesh` 顶点 y +1m：road alignment source rebuild 拒绝。
3. 删除固定 base edge、保留 nodes：可信 base edge 身份合同拒绝。

`TrustedBaseContract` 现在保存固定 base edge 的 id/端点/中心/长度/rotation，并拒绝 graded base 或非等高的“可信 base”，不再只保存坐标到高程的映射。

## 非零工程正例

合成 fixture natural 0m、final 0.1m；使用生产 `plan_terrain_carves` 与 `apply_terrain_carves`，随后在 final 上执行 access 和 yard。结果仍为 24/24/24、yardUnresolved 0、16m collector + 12m local。该正例在内存测试中通过。

## 原自然固定入口评估

本轮实际执行 4 个联合状态，保留原始 8000mm 固定父节点，仅改变新增内部节点候选高程；同时解析并传入原 settlementProgram，成功分支使用 production final 后再做 joint audit。

结果：

- `attemptedStates=4`
- 固定入口 8000mm 均保持
- `extensionCount=null`（均在 plan 阶段失败，不能声称零扩展前缀）
- 8000mm：station45 `requiredRoadElevationMm=7120 > allowedRoadElevationMm=7040`
- 7000/6000/5000mm：固定入口后的内部候选仍在 plan 接入阶段拒绝
- `bestLegalCandidate=null`
- status=`unresolved`

这不是全球无解证明，也没有把 4 个状态扩张为完成容量结论。

## 未完成

- 非零工程正例的 scene/zones/base/natural/final/全部产物尚未作为完整独立包落盘重读；当前 `persisted-positive-r5` 仍来自平地正例。
- 原自然案例没有 24 个工程合格且完整可达的 parcel 见证。
- 没有运行新 512m generate、gate、最大图或浏览器。

## 验证

- `cargo test -p glyphweave-core worldgen::road_parcel_feasibility`：7 passed / 2 ignored / 0 failed。
- ignored persisted-positive 与 original-natural-joint-evaluation 分别独立执行通过。
- `pnpm doc-tree:check`：PASS。
- `git diff --check`：exit 0，仅既有 CRLF 提示。
