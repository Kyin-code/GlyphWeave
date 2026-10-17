# V4.1 B01-G6-R1：修复候选实体绑定与完整取证，不执行H1

## 1. 先读与目标

先读b01-g6-frontage-rejection-diagnosis-inspection.zh-CN.md及inspector-b01-g6-20260919/support-review.json。G6的28/4生产首拒绝分布保留，但辅助building/support数据用了BC道路，H1量化结论失效。

本轮只纠正诊断，不抬高B/C、不新增候选、不改任何生产合同/参数。仍原J9-R1(B3000,C-382)、J9-R2(B3000,C3000)，两新段junction9、原坐标/宽度/shoulder/fill/rise、原自然/zone/需求/旧路不变。

旧G6/G5表、run、receipt不覆盖；原文只追加勘误。B partial/C不推进、不Git/agent/F/A变更。

## 2. T1：用真实生产candidate_site构造候选实体

删除或改正sample_entity_for。它当前忽略sample并返回road实体，不能作为building证据来源。

在现cfg(test)内，用当次生产Request、road、sceneId、station、side调用既有frontage::candidate_site，再取site.entity()；不重写另一套实体/朝向算法，不扩大pub。逐项断言生成site的中心、station/side对应、requestId/roadId、parcel boundary与CandidateSample相符。

用真实site.entity()计算footprint/yaw和与support_check一致的bounds(...,2)。保存building与road两个独立ID/类型，避免变量复用。

先跑小的合成回归（无自然运行）：
- 同一条112×12道路的两个不同station、两个side，候选building均12×10；station18北侧center(270,157)/yaw180/support[262,150,278,164]，南侧center(270,203)/yaw0/support[262,196,278,210]。
- station66南侧support[214,196,230,210]；候选范围不能等于道路[174,172,290,188]。
- 非退化几何下sample/实体绑定不一致明确报diagnostic_incomplete或拒绝，不用road fallback。
- 首违规缺额与全支撑最大缺额分字段，合成两点用例证明第一点差8不意味着完整只需+8。

## 3. T2：原始trace完整保存，不能只重新拼部分字段

四次measure输出按state/request分包保存生产trace：samples、rejected计数、rejection_details全部字段，保留sample.design_level_mm，即使detail无natural/design也保留候选设计高程，字段来源区分。

rejection_details的conflicting_entity、before_mm、after_mm、parcel、at、note等全保留；不要只选natural/design四字段。用正确顺序关联失败sample与detail；有accepted项时不跨数组同下标错配。断言attempt=sample数、拒绝数=detail数、遍历后无多余detail，字段关联错误就显式incomplete。

32行原始输出、派生辅助输出分开标记。完整性至少分traceCountComplete、sampleBindingValid、geometryValid、rawDetailPersisted、profilePersisted，不能单一detailComplete掩盖所有问题。

## 4. T3：补正确全支撑极值，但不据此运行新候选

在真实building+2m支撑闭区间读取冻结natural，输出min/max及其坐标、noCutMaxDeficit=maxNatural-design、首点deficit、实际zone填方约束的必要允许区间。完整parcel极值继续单独命名。

这些都是针对当前候选的离线必要条件，不替代生产审核、不用于私自改变profile。

应能复核检查员反例：st66南侧design147，maxNatural1149@(230,210)，完整缺额1002；首违规仅155-147=8。假设uniform+696仍不足，但1002不是B增加量证书。

撤回H1“B提高等于街面整体提高/696可清除28次”的依据。只读解释现solver的端点坡度传播与内部natural下界分段支配：station42原设计1105，B3696起点传播下界1056，不能推出该站抬高696。**本轮不运行B3696/B4002或任何新值，也不另造求解器。**

记录existing_support_changed的四条原始对象/坐标/before/after；只解释该检查表示建筑carve影响旧硬支撑，不新增专门carve扫描，不把它未经证明叫拓扑冲突。

## 5. T4：真正保存完整两态计划/profile包

新目录中以create-new保存两态新增AB/BC的完整生产CorridorDesign/request/profile_mm，或完整staged RoadNetworkPlan副本；绑定hash和natural/source/base hash。不能只存长度+first+last，也不能将诊断计划包称发布世界。

通过现有profile重建候选site时保存真实设计高度，不用RejectionDetail的null覆盖已知site height。验证所有已保存候选中心与profile station值一致。

## 6. 执行预算与范围

先完成上述合成回归，确认输出schema包含所需字段，再允许**一次原两态诊断补证**。这是本轮明确的诊断重放授权，不是G6历史原运行，不算新高程状态。

- ≤1自然harness、2态、≤4measure/32attempt，sample_cap≥8，仅BC测量，整轮≤120s，编译单列。
- 按需重建AB/BC一次每态，infer交叉核对最多1；准备阶段/候选阶段/叶子内部not_instrumented分列。
- baseline realization=0、candidate realization=0、candidate_final_terrain=0、bake=0。
- 0其它旧自然实验/128/clock80/carve独立扫描/CLI世界/gate/release/browser/max/workspace。
- 只改auto_roads.rs cfg(test)、文档、新证据；frontage/frontage_supply/road_alignment等生产源码只读，不改公开类型/默认/审核/no-cut/fill/地形/需求。
- 一次自然输出不完整就标不完整并停止，不为补日志自动重跑。

## 7. 交付与停止

新ds-b01-g6-r1-diagnostic-repair/：小测试日志、规则表、唯一run、四份完整raw trace、32行正确辅助几何、两态完整profile/plan/hash、真实预算、旧证据/输入源码未变检查。

新b01-g6-r1-diagnostic-repair-result.zh-CN.md与新receipt，G6结果只追加勘误；doc-tree/diff检查。

结果只说明诊断修复和当前拒绝原因，不执行H1、不批准下一高程试验。完成停止等复验，B仍partial/C未推进。
