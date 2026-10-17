# B01-G10 连接设计独立审查（2026-09-19）

## 结论

**归属收口：accepted-subtask（限已核验范围）。连接设计与H-G10-1：needs-changes，不批准生产语义变更或自然实验。**

目前的主要阻断不是缺一次执行，而是把入口校验误用到路径内部、把局部高差推成全域不可达，再据此跳到合成合同变更。修正设计论证和接口范围即可；不重跑G9、不重开已验收自然证据。

## 1. 已完成的独立检查

- receipt 8886bytes，SHA256 9baa3d338c010eb5d5ce565e19db5ac6ea14b68c3d8d1ed280a2eebd8aa67c47。
- 9项回执bytes/hash、16个source-hashes条目匹配；G9的11项非文档证据、台账7项绑定保持；生产前缀81685bytes/ba6deb95…一致。
- attribution三个决定性owner/component/几何与检查员离线恢复一致；旧G9标签作废范围及BLAKE3输入编码说明保留。
- 独立--exact运行g10_preflight_identity_binding与g9_preflight_provenance_schema，2 passed/0 failed；无自然输入/诊断/terrain重放。
- 当前映射确实能拒绝测试里的同优先级、不同几何实体换位，比G9 priority-only有实质加强。但profile只比较is_some，不是完整profile签名，见§4。

## 2. P1：0.30m门槛适用位置错误，无法支撑“必要矛盾”

access_for_entity在access.rs:334附近仅检查**exit_point返回的start与closest得到的facade_point**，二者最终地面与建筑ground_height差<=0.30m。这不是整条路径各点相对建筑高程的限制。

按原12×10建筑及现exit_point公式离线算得：

| 位置 | 北候选 | 南候选 |
|---|---|---|
|authored anchor|(270,163)|(270,197)|
|实际route start|(270,162.85)|(270,197.15)|
|facade point|(270,162)|(270,198)|
|直达BC最近边缘目标|(270,174)|(270,186)|
|已观测冲突点|(268,171)|(268,188)|

所以0.30m入口门槛并不直接检查(268,171)/(268,188)。也不能把authored anchor本身与经过调整后的exit_point混作同一点。

实际路径合同还包括：
- segment_clear：中心及左右0.75m，沿段<=0.5m间隔双线性height采样，邻采样梯度<=0.25（约1:4），同时避实体障碍和水；
- route：先试直连，再在2m网格8邻域搜索，MAX_LENGTH=120m、MAX_EXPANSIONS=6000；
- frontage最后要求连接到指定site.road_id，而非随意一个道路。

保留的4点确实显示x268北171→173这条2m小段变化4.272m、南187→188这条1m小段变化4.757m，远超0.25梯度。但它们只能否定**穿过这些点的对应局部路径段**，不能证明所有候选路线必须经过它们。

直连中心在x270，横向检测线约x269.25/270/270.75；现有冲突样本x268甚至不在这三条检测线上。不能据此反向宣称直连必能走通，因为还有其它未知栅格、障碍/水/保护约束；正确结论是未验证，而非必要矛盾已成立。

因此撤回：B2必然牺牲连通、保持旧支撑+真实连通必然矛盾、真正决策只剩C或拒绝。也不能用一次未来C-policy试验access失败证明所有B几何路线不可行。

## 3. P1：首选合成提案没有保证完整审核不变量

现frontage的既有硬支撑检查是**before==after**，不是只要求after>=before。H-G10-1“只抬高、不丢弃邻接填方”至多防下降，仍可能把旧支撑抬高而触发同一个拒绝。

前检还有独立的候选完整pad精确高程检查engineered_platform_not_flat，之后才调用access；即使四旧点保住，若保留的邻接填方进入新pad导致pad不等于2390，同样会在access之前失败。四点+link测试不能代替该链。

本轮没有一个明确到可验收的合成规则：作用域、与old priority/full_target关系、如何同时保旧硬支撑等值和新pad等值均未定义。因而不批准改apply_terrain_carves后“看看access是否通过”。

## 4. P2：所谓完整身份签名仍漏profile内容

verify_carve_identity Expected仅存has_profile:bool，比较项为几何/priority/blend/target及profile是否存在；不比较profile起终点、逐米elevations/maxFill/version等内容。当前负例交换两个几何不同的building owner，可以通过预期拒绝，但不能覆盖“同样外形与中心高程，内部profile改变”的错误。

静态反例：对synthetic_bc_road某非中心/非端点采样单变量+1mm，保持carve geometry、target_h_m和is_some不变；当前比较式中无任何项能察觉。应补profile逐值/稳定内容hash比较与该小负例，不运行自然数据。不能用profile presence代替完整profile身份。

## 5. P2：内部TerrainCarve字段不是已有公开输入能力

TerrainCarve是worldgen.rs内部结构，connection中心/half由入口到frontage_road_point派生，padding/priority/blend/target在plan_terrain_carves中生成。文中“half/blend/target已是输入，所以现数据能表达B2/B3”没有指到可作者配置的schema/解析/consumer链。

区分内部结构可表示与当前manifest已可声明。若改变连接端点、曲线或作用域，需要明确新增/改动的输入与消费者，不能称无需接口变化即可落地，也不能直接操作内部carve绕开全链来源绑定。

## 6. 下一步（不自然执行）

任务ds-v41f-b01-g10-r1-access-contract-design-task.zh-CN.md：
- 接受归属成果，不再修旧标签/重跑旧自然；补profile-content小回归。
- 纠正入口与路径合同适用范围，给准确坐标和路径允许域/未知项，保留“局部很陡”而不声称整体不可能。
- 以旧支撑等值、新pad等值、入口<=0.30、路径坡度/宽度/指定道路为硬不变量，重写一个首选连接设计/最小接口说明；现证据不足则设计一个只读、无策略变更的缺口验证提案。
- 先提交可审查方案，不实现任何生产补丁，不启动自然harness/carve/access重放/realize/bake。

本次检查员仅源码/已有数据/坐标算术与两项合成测试，无生产自然调用、源码修改、agent、Git写或F盘写入。B partial/C不推进。
