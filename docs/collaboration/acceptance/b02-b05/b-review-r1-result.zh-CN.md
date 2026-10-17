# B Review R1 整改结果

状态：`partial`。旧 B 报告与失败证据保留，本文件纠正原报告事实并登记本轮增量。

## F1 residual 来源绑定

- 新增 `residual_land::audit_source`：从 `manifest.style + scene` 独立取期望策略，并与 scene plan 的 `residualProgram` 精确比较；缺省与显式值不可互换。
- `development::audit_landcover` 改为显式接收 `expected_residual`，不再 clone 被审核 plan 的策略；生成期另用 `validate_landcover_plan`。
- CLI `quality-report` 先生成 residual 全局校验，并同时核对 scene 内 plan 与发布 `landcover-plan.json`。
- CLI 负例通过：同步改 scene+发布文件为 0/0、删除策略、错 version、增加未知 scene 程序均被拒绝。

## F2 道路层级选择

- priority/purpose/tier 改为原子选择：先确定获胜 priority，再按该 purpose 和总服务数选择授权 tier，不再用“最后写入的 purpose”。
- `tier_for` 不再忽略显式 `purposes`；无可用 tier 时产生 no-candidate，不退化为任意 role。
- 宽路结点半径改为 `max(junctionLength, roadWidth/2+3)`；8m 旧行为仍为 8m。
- 新测试覆盖 urban/rural 同时受益时选择 urban 授权 tier、无授权 rural tier 时不产生候选、旧 v1/8m 行为保持。

## F3 morphology 类型

- 缺省仍关闭；显式 number/object/null/array 均明确拒绝；未知字符串拒绝；合法六 profile 行为不变。
- 修复悬空 caseId：`us-houston→us-denver`、`hk-mid-levels→hk-central`、`jp-kyoto→jp-kyoto-gion`；新增测试确保所有 source_case_ids 都在 48 条索引中。

## R1-C 24 目标 CLI

两次 512m 尝试均未进入 frontage：

1. `generate-control-1.log`：宽路与共享端点支撑超出旧 8m 结点半径；已修复。
2. `generate-control-2.log`：`corridor infeasible at station 45: required 7120mm exceeds 7040mm cap`；按两次预算停止，未第三次生成。

因此本轮 **没有** 完成 16 住宅 + 8 店铺的真实 CLI 24/24，也未形成 CLI 中 collector+local 的实际拓扑。诊断表见
`bevy/work/collaboration/ds-b-r1/diagnostic-table.json`：

- 旧 B01 实际 5/24，住宅 5/16、店铺 0/8；profiled road 中仅部分与 urban 相交。
- 旧 B02 实际 4/24，住宅 4/16、店铺 0/8；5 条新增 profiled road 全部 8m，urban 重叠有限。
- 新 R1 设计如果地形允许，可用两条 12m local 各 6 站/侧和 16m collector/8m farm-access 链路覆盖 24 个 parcel；但原冻结地形在 6000mm 填方约束下拒绝了走廊。

## 原报告纠正

- 历史 workspace 日志复核总数为 **484 passed / 0 failed / 3 ignored**；402 只是 core，不是全 workspace。本轮没有再跑 workspace。
- B03 原“1km”控制实际是 **512×512m**，面积分母 262144m²。
- B04 当前仅是 profile 验证与默认关系表；profile 参数尚未驱动生成，service relations 尚未接 AccessGraph 路径审核。
- 48 条索引不等于 48 个独立核实案例；未核实项仍为 heuristic/source_pending。
- B04 `village-field-2` 没有 quality-report，不能写成独立审核通过。
- 因 CLI 24 目标未闭环，`engineeringStatus` 保持 partial，不沿用上一轮 pass。

## 本轮验证

- `cargo test -p glyphweave-core`：405 passed / 3 ignored / 0 failed。
- `cargo test -p glyphweave-cli --test residual_source`：1 passed。
- A 原 frontage 与 automatic-road CLI 回归定向测试通过。
- `pnpm doc-tree:check`、`git diff --check`：exit 0。
- 新 512m 生成：2/2 尝试失败，未运行 gate、最大图、release 或浏览器。

## 未解决

- CLI requested/planned/realizedWithAccess 未达到 24/24/24，店铺仍未被本轮新 CLI 实现。
- 完整庭院 surface 与新增必需庭院缺额 0 未在本轮新 CLI 证明。
- CLI collector+local 两级实际拓扑未完成。
- residual/profile/service 的后续真正展开与网格压缩风险仍未做。
