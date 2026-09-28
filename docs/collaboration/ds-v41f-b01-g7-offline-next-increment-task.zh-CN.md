# V4.1 B01-G7：利用已验收证据形成首个增量的离线实验提案

## 1. 当前裁定与本轮目标

G6-R1技术修复已接受，预算台账已归档，process仍budget-noncompliant。不要再修改台账、旧回执或围绕aggregate标签往返。旧标志初始为true，不能以标志值证明历史运行阶段；继续按既定来源限定读取，无需单独修订。

**本轮推进的是B阶段内部的离线决策，不是新的自然运行授权。** 使用现成AB/BC完整profile、四份raw trace、32行正确支撑扫描与源码，回答：哪个既有窗口最值得用最小变化验证第一个真实新增parcel？

目标不是一次清除全部28个拒绝，更不是改成要求少于原16住宅+8店铺。首个增量只是下一实验门槛，原需求不变。B partial，C不推进。

先读：
- G:/difyllmwiki/GlyphWeave/docs/collaboration/b01-g6-r1-diagnostic-repair-inspection.zh-CN.md
- G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g6-r1-diagnostic-repair/run.json
- 同目录raw-trace-*.json与完整profiles
- G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g6-20260919/support-review.json

## 2. 工作A：按现有八个几何窗口而非32条尝试排优先级

两侧×station18/42/66/90为8个不同几何窗口；两请求与两态重复观测不能重复计容量。

逐窗列：中心/用途/候选building+2m范围、现有设计值、全支撑自然最大值及坐标、必要pad高程下限与填方上限、已观测的首拒绝、未进入的后续审核。no-cut之后仍可能有support/access/yard问题，不能标为已通过。

首先考虑station18等必要修正量较小的窗口，但按证据排序，不直接将first-point差值当目标。区分：
- 直接抬高一个pad的几何必要量；
- 改B/C后生产最小profile的实际响应；
- 最终完整审核是否通过。
三者不能互相代替。

只做针对选定站点的端点坡度传播不等式：从已有源码说明9m平台后的B/C影响如何衰减，哪些内点可能由natural下界支配。可以列有来源的必要B/C条件，但不得运行生产solver/复制完整求解器/枚举新高程。

最多保留2个目标窗口、形成1个首选参数提案；若证据不足就列明确缺口，不为交付强造可行性结论。每个拟议值都标proposed_not_executed，不冒充观察值。H1的uniform+696及B+1002不自动恢复授权。

## 3. 工作B：解释两处support-change样本的来源层与影响范围

现成4条记录实际是两个位置被两请求重复触发，冲突实体均为本态新增BC，不是未验证的历史35路。

重点读取既有数据和源码，不执行carve重放：
- (196,171)：before5074→after1960；
- (196,188)：before4265→after1960；
- 同态BC profile在x196对应station92为2120；site在station90设计1960。

区分profile设计值、现有carve合成采样before、加入建筑后的after。说明production既有支撑检查扫描的是何种范围，以及两点相对BC中心线的横向位置；不要将所有值都叫路面高程或认定拓扑冲突。

沿代码定位对应数据来源、support检查、road profile/flat-top/footprint/transition和carve组合的调用关系，可引用行号。回答：这是所有窗口都必然触发的已证问题，还是目前仅在station90两个点观察到的后续风险？没有证据就写unknown。

不根据before与profile不同就改审核；不同范围可能承担不同合同。不得把这一步变成全场carve诊断、算法重写或“设计B”实施。若最终仍需某个点的生产采样重放才能定因，把它作为后续提案中的单独实验项，不本轮执行。

## 4. 工作C：交付可批准或拒绝的最小实验方案

一页给出：
1. 首选目标窗口与选它的定量理由；保留原需求/顺序/几何合同。
2. 拟议变量到底是B高程、C高程或其它输入，最多1个主变化方向；现有模型能否表达，不改生产合同。
3. 必要条件依据、尚未验证环节、预期能区分的两个结果，不说必成功。
4. 对照使用已有G5/G6证据，不建议为对照再跑旧两态。
5. 拟议执行上限建议最多2个新状态、1次harness、一次baseline与按需最多2次candidate realization；具体是否授权留主检查员决定。
6. 在任何自然调用前，拟议harness的序列化、profile点数、raw trace、单位、预算/早停都应能用现存JSON或合成fixture验证。自然运行失败即停，禁止自发重启补字段。
7. 工程拒绝、测量零收益、实现拒绝与真实增量分别怎么报告，如何保证完整证据一次输出。

若选定方案要求改carve/审核/no-cut/fill/需求/地形，明确标超出现授权并说明原因，不把它塞进本轮实施。允许结论为需要一个更窄的诊断提案，而不是强行给新高程。

## 5. 严格边界与交付

本轮cargo/test/natural harness/design/append/infer/measure/realize/candidate_final_terrain/carve重放/bake全部0；不改Rust/CLI生产或测试源码，不新建运行harness，不启动服务/agent，不Git写操作、不F/A变更。

允许：只读已有JSON/栅格/源码，小脚本核算既有8窗的几何/算术/来源一致性；脚本累计<=60s，不全场枚举、不扫描参数组合。不能以“离线”名义运行另一套road solver或合成新世界。

新文档：G:/difyllmwiki/GlyphWeave/docs/collaboration/b01-g7-offline-next-increment-proposal.zh-CN.md
新证据目录：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g7-offline-next-increment/
保存窗口对照表、两点来源说明、一个首选实验提案、输入/源码hash、实际离线命令/时间与回执；状态proposal_ready_for_review。旧台账和已验收证据不修改。

doc-tree/diff检查后停止。**本任务书不授权执行该提案。**
