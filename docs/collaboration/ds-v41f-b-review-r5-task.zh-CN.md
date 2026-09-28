# V4.1 B Review R5：真实工程审核接线与合法候选评估

读取 G:/difyllmwiki/GlyphWeave/docs/collaboration/b-review-r4-inspection-20260916.zh-CN.md。

本轮只补现有联合流程，不重新研究城市、不增加profile或新验收平台。保留R4平地/非零工程正例、required统计和实际attemptedStates计数。原自然需求/地形/固定入口不改。

## 1. 首先拒绝三个真实工程反例

针对检查员只读探针 G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b-r4-20260916/persisted-probe.rs 建立对应红测，再接入已有生产校验：

1. 固定base road中心[64,160] final高度+1mm，natural和原plan不变；
2. extension.design.roadMesh一个顶点y+1m，profile/request不变；
3. 删除固定base edge但保留其nodes及其余extensions。

要求明确工程拒绝；不得靠让整个正例报错、修改期待值或统一返回incomplete凑通过。正例24/24与真实yard仍需通过，允许全场其他未开发面积incomplete，但上述破坏必须有独立的engineering failure。

复用road_alignment设计重建/verify、road_extensions合法接入、最终road/site支撑审计与base存量合同；不要新写一套偏松几何检查。可信base保留原node/edge身份、端点及实际高程/profile义务；不能仅记坐标。若只支持未分级的等高base，应在入口显式限定并拒绝其他base，而不是对graded base每个节点强设base.level。

可以把joint汇总器改为内部只接受已验证上下文；但外部入口必须真正做上述审核，不能用换名字/降低可见性逃避反例。

## 2. 接通实际final与完整需求

所有候选采用：冻结自然与原需求/base → 合法道路增量 → 生产工程步骤产生final → 实际实体/入口/yard → 独立来源/工程审核。

自然评估成功分支不能再把natural同时传作final。复用已实现的工程步骤，不复制另一份terrain carve算法；确保保护区/fill/固定支撑约束和生成管线一致，不只调用裸apply函数后就算完整工程验证。

解析并传递原manifest的settlementProgram/housingMinimum、frontage需求与用途等，不再传None悄悄略过设施。

bestLegalCandidate必须基于真实工程合格且所需义务保持：planned、realizedWithAccess、unfilled、requiredUnconnected、yard缺额及道路源审核分别判定。找不到24也可保留供给较好的合法partial witness，但不能因planned==24就标合法完成。输出状态从实际结果推导，不硬编码unresolved/pass。

## 3. 有意义的原自然候选

原8000mm固定父节点入口必须保持。7000/6000/5000整网改低只保留为负例，不再作为推进规划的三个候选。

构造有限候选，只变化新增内部节点的合法高程、非固定端点/合法分段路径或现有可接入位置。每个共享node高程保持一致，固定入口保持8000；每条边须先设计再通过真实append检查，按真实完整parcel供给评分，不只是zone中心接近。

本轮最多16个有意义的新联合状态，利用已有AlignmentContext/自然缓存。记录状态前后增量、拒绝位置、固定入口未改、成功规划/审核阶段、实际住宅/店铺供给；同样状态不重复启动子进程。状态预算与extensionCount分列；未得到部分plan时extensionCount=null及理由，不自动填0。

这不是要求穷举或保证解；合法候选若都失败就记录实际失败约束和最好已验证前缀/供给，不能得出全球无解。至少调用完整评估器而不是只验证已知非法入口。

无联合见证不要bake；本轮不跑最大图、release、浏览器。存在24且工程审核通过的原自然见证后，至多一次512m generate及独立gate，各120s/4096MiB，失败不自动再跑。

## 4. 完整保存已有非零正例

将现有nonzeroEngineeringControl所需scene、zones、冻结请求、base义务、natural/final栅格、road/site/parcel/entity/surface和报告保存到新目录。允许小型测试binary高程，不需要复制完整最大图。

新进程/独立读回路径不得靠重新调用fixture()恢复隐藏输入；由落盘来源恢复并重建审核。除已有删除yard负例，还复验第1节三个破坏。原ds-b-r4/persisted-positive只读，不覆盖。

测试输出路径可通过参数/唯一临时目录选择；不要把必须重跑的测试硬编码为覆盖worker历史证据目录。固定旧文件保留，不在回执里谎称此次覆盖前后是不同历史运行。

## 5. 修改范围及交付

生产修改限 G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/worldgen/road_parcel_feasibility.rs、road_network.rs、road_extensions.rs、road_alignment.rs及必要worldgen.rs工程入口复用/接线；现有frontage/development/landcover可新增与上述问题直接相关的测试，不放宽它们的谓词。需要其他生产修改时列最小理由，不借机扩展C或导入库。

新增 docs/collaboration/b-review-r5-result.zh-CN.md；证据 bevy/work/collaboration/ds-b-r5/，含源码/输入/runtime绑定、定向测试/正负例、新非零落盘包、natural候选实际结果/尝试计数、未解项。

只跑受影响定向测试和小集成，单项120s，首次编译可独立300s。不全core/workspace、不重放历史几百次走廊扫描。完成后停止，状态ready_for_review或partial，不签收B。

不Git add/commit/push/reset/clean/checkout/switch/stash，不启动其他agent，不改F盘/A冻结证据/原自然manifest和需求，不提高fill/坡度/预算。
