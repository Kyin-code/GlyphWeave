# B-03 结果：用途分区、地表执行与缺额闭合

状态：`partial_content`。当前 residual 审计见
[`rk-x/B03-RESIDUAL-AUDIT-20261020.zh-CN.md`](../../rk-x/B03-RESIDUAL-AUDIT-20261020.zh-CN.md)。

## 已实现

- 显式 `residualLandProgram` v1：城市开放空间比例 0..0.25、农村牧草地比例 0..0.50；缺省关闭。
- `LandcoverPlan` 记录 scene/source policy；residual 只从 `FutureDevelopment`/`AgricultureCandidate` 来源选真实 1m 地表，避水、障碍和坡度，并按比例停止。
- `PublicOpenSpace`/`PastureGround` 材质与 ledger 物理字段。
- `LandUseLedger.residual` 按 `sourceZoneId + subUse` 输出 candidate/target/realized/policy remainder/rejected/unresolved；realized 从 mesh 复算。
- 新单测 `residual_audit_splits_policy_remainder_and_rejections_from_unresolved`；ledger 全组 9/9 通过。

## 当前实测

- 旧 `land-execution-2`：ledger realized 138,384m²、unresolved 123,760m²（历史 worldRevision）。
- 2026-10-20 当前 HEAD 控制：`b03-residual-20261020`，generate exit 0，58 entities，worldRevision `ddc5d27d...`；
  ledger realized **133,602m²**、unresolved **128,542m²**；
  residual candidate **229,568m²**、target **87,314m²**、realized **101,026m²**、
  policy remainder **142,254m²**、rejected attempts **2,242m²**。
- nature 已全部实现；urban/rural 仍是主要未实现量。
- frontage 仍受 B-01/B-02 供给缺口影响。

## 结论

residual 缺额现在可解释、可逐 source zone/sub-use 复算，但显式子用途、农村公共空间、
共享农路/等高条带、development↔landcover 映射和全局 1m 重叠审计仍未完成。B-03 仍为 partial_content。
