# B Review R6 结果：完整支撑与非零持久化收口

状态：`partial`。完整道路支撑审核、非零工程独立落盘重读和固定节点身份接线已完成；原自然案例仍无 24 见证。

## 分类结果

| 项目 | 状态 | 说明 |
| --- | --- | --- |
| engineeringNegatives | pass | 中心、非中心、端部和扩展设计破坏均拒绝 |
| syntheticControl | pass | 平地 24/24/24 保留 |
| nonzeroEngineeringControl | pass | natural/final 差 100mm，24/24/24 |
| persistedNonzeroPositive | pass | 完整最小输入落盘并重读审核 |
| originalNaturalCase | unresolved | 原自然固定入口下仍无合法 24 见证 |
| naturalJointEvaluation | executed-unresolved | 4 个真实候选状态全部 plan_rejected |
| cliIntegration | not_proven | 未运行原自然 bake/gate |

## 完整道路支撑审核

`audit_joint_plan` 现在对每条 base/extension road entity 的完整支撑矩形执行：

- 以 natural + 候选 plan 的生产 `plan_terrain_carves`/`apply_terrain_carves` 重建 expected final；
- 逐支撑格比较 actual final 与 expected final；
- 继续核对 natural fill envelope、road profile、固定 base edge 身份和 extension 设计重建。

测试覆盖：

- 固定 base 中心 `[64,160]`、纵向 `[65,160]`、横向 `[64,161]`、端部/边界点 `[96,160]`、`[28,160]`、`[100,160]` 的 +1mm 无来源扰动均拒绝；
- 支持范围外 `[0,0]` 的合法无关地形变化不拒绝；
- extension.roadMesh 顶点 +1m 仍由生产 road-alignment verify 拒绝；
- 删除固定 base edge 仍由 base 身份合同拒绝。

## 非零工程持久化

`bevy/work/collaboration/ds-b-r6/persisted-nonzero-r6-final/` 保存：

- scene、zones、water；
- base plan、candidate plan；
- natural 与 final 高度 payload；
- development plan、landcover plan、entities、surface realization。

文件写入使用 create-new，拒绝覆盖。重新读取时从文件恢复 natural/final/base/candidate，再通过生产 `trusted_base_contract`、`audit_joint_plan` 和 `landcover::verify` 审核。结果保持 24/24/24、yardUnresolved 0、12m/16m 两级路网。

该包是 `integration_fixture`，不是原自然案例或 `generate-world` CLI 证书。

## 固定节点与需求接线

- `TrustedBaseContract` 保存 base edge 身份和固定 node map。
- 新增 `apply_fixed_endpoint_levels`，按坐标对所有候选边应用固定端点，不依赖数组索引。
- 测试覆盖数组重排和 `[224,96]`、`[288,224]` 两个固定接入根。
- `evaluate_style_candidate` 与自然 runner 现在都解析并传递 settlementProgram，不再一个传 None。

## 原自然评估

实际执行 4 个状态，固定入口 8000mm 保持，只改变新增内部候选高程：

- `attemptedStates=4`
- `extensionCount=null`，因为均在 plan 接入阶段失败
- 8000：station45 requiredElevation 7120 > allowed 7040
- 7000/6000/5000：内部候选接入/走廊约束拒绝
- `bestLegalCandidate=null`
- status=`unresolved`

没有重复 R5 的四档全网扫描以外的无界候选；也没有得到 24 见证，因此未 bake。

## 历史证据

- R4 `natural-joint-evaluation.json` 已保持原内容，1143 bytes / SHA256 `67666193c600a0adcbcb6120f86e9c3907dbd5c2fc6dcd7022ec39923e1f6694`。
- R5 `natural-joint-evaluation-r5.json` 保持原 SHA256 `a240b0c5b76592c6e6198376edf5c7981a4d26ecefdbc476b2a619fab11dbd4e`。
- R6 输出使用新的 `ds-b-r6` 路径和 create-new。

## 验证

- `cargo test -p glyphweave-core worldgen::road_parcel_feasibility`：8 passed / 2 ignored / 0 failed。
- ignored persisted-nonzero 与 natural-joint-evaluation 分别独立执行通过。
- `pnpm doc-tree:check`：PASS。
- `git diff --check`：exit 0，仅既有 CRLF 提示。
- 未运行完整 core/workspace、512m generate、gate、最大图或浏览器。

## 未完成

- 原自然案例仍没有 24 个工程合格、完整可达的 parcel 见证。
- CLI 不能表达合成 raster/zone，原自然 24/24 未在 `generate-world` 中证明。
