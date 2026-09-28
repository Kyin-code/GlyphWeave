# B01-G17固定G-W1自然批次独立审查（2026-09-20）

## 裁定

**accepted-bounded-access-negative / archival-delivery-partial**。固定G-W1按保存的生产记录通过已执行D1（完整受影响旧support、pad255及既有prechecks），生产有界access无连接、queue_exhausted。该七块候选本次关闭，不调参、不自然重跑；不推论整个西端或所有连续路线无解，不宣布完整工程/audit已通过。A不变，B partial/C不推进。

自然实验按记录完成；交付缺完整批次实体/carve表及中间人工失败日志，不可称A–E所有条款无缺口。技术负结果与交付完整性分开验收，无需补自然运行。

## 独立核验

清单41/41及回执manifest绑定匹配；执行版1130000字节与当前源码逐字节一致，原始BLAKE3重算108ac4a51c9dd27a315f70306ea5223fe77260aa287cbc69dcb3eaf1544b8ef0。生产前缀81685字节/ba6deb959c3e3b913ed2c65a887f4860b1cddf505cfd2ae2fb71758997224d34保持，access未改。

检查员独立读取GWHM面并核对：
- 本次surface-natural-mm.bin与归档chunk-0-0.natural.height.bin **逐字节相同**，BLAKE3均88cca57680e7b7cf6b183abd9217e4fc6e1ed959d905d454a14051ea0e54f08c。这确立当前冻结坐标/版本的样本同值，不外推其它seed/版本同源。
- 候选pad255格全2390；整面没有低于当前natural的格；与C0不同的1391格全部在固定7块影响矩形并集中，域外0；变化格没有最终值高于natural+6000mm的情况。这些是保存面离线检查，不替代水/parcel/其它完整审核。
- 变化最大格(219,157)：7204→349，绝对差6855mm。
- access.json记录connected=false、roadId=null、observerEquivalence=true；routes queue_exhausted、expanded356、queuePeak25，分支符合代码。未独立重跑真实route或自然实验。

检查员脚本首版误取halfWM/halfDM而run字段实际halfW/halfD，导致影响域计算无效；已保留首版并在verify-corrected.cjs及**evidence-corrected.json**修正，后者为本审查权威复核，输入/worker证据未改。检查员cargo/自然/terrain合成/真实route全0。G17测试固定create_new输出故未直接重跑它们。

证据目录G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g17-20260920/。

## D1及资源范围说明（检查员直接限定，不新开勘误包）

1. support使用真实base_entities和全部proposed域，20275格/22331对按代码和运行记录全部过；pad255通过。该结果可保留。
2. g11_contract_prechecks接收site/plan而非新7块几何，因此其waterCorridor/occupiedOverlap等仍主要检查原site/原连接走廊，不能用其pass包揽七块全部新增足迹的所有工程限制。最终面经生产natural-floor/zone clamp，不等于所有water/roadbed/parcel/landcover审核运行。D1应写明各子检查范围，避免“工程合法”总称。
3. 已保存baseline逐格与G12相等且其原文件仍在，引用旧面作为去重存储不影响当前负结果，但应明确未保存第二份，而非假称本包文件存在。
4. 完整批次实体/grounding/carve表任务要求未交；事后archive派生表不能冒充自然批次现场表。中间合成失败日志未保留也记缺口。不能重新运行自然以伪造当时快照，后续若必须使用现场表，应在新的获批动作中保存。
5. run.callCounts.surfaceCellsBuilt=262144只加了candidate；既然baseline和candidate各一张，总完整反事实构造应524288格（另列自然保存与D1局部栅格）。graph44条含2候选条，其他42。不能写成全部工作只有262144格。
6. cellsChangedByBlock是每个变化格落入每块影响盒就++，重叠可计多次。T1=75/T2-5=105/W1=1219/W2=86是**非互斥影响域内变化计数**，不是移除单块后的因果归属。最大6855mm是真实整体差值；没有逐块消融/运行来源trace不能断言“该块单独造成”。
7. 采用shared clock是正确的；真实入口应在最终文件serialization之后再检查deadline（当前run对象由g1b_publish返回后g17_write_new再序列化）。本轮3.699s不构成超时证据，不为此重跑。

## 关闭与后续

固定G-W1 access负结果正式归档关闭；自然真实性缺口已实质解决，是本包有价值的新证据。G16A证明线不重开，西端仍未获一般不可达证明。

不要求DSH再交确认报告、补日志或重跑矩阵。若用户下一次要求推进，优先只读本包natural/GW1/观察trace定位剩余实际grade/obstacle与路径前沿，再决定是否值得提出唯一新候选；不直接把356节点或467拒绝当作全局不可达，也不立即再批自然试同一七块。当前不新增任务/自然额度，不扩大合同或生产集成。