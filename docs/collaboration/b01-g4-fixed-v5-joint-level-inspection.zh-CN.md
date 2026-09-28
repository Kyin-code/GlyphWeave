# B01-G4 固定V5高程实验独立审查（2026-09-18）

## 结论

**accepted-bounded-negative-with-errata**：接受本次六态负观测及其阶段/预算，不接受从六态上升到固定几何全域不可能、或“只有改路口合同才能推进”的结论。

本轮不重跑G4。文档、诊断单位和端部范围问题可以在下一小任务开头一次修正，不再另开自然复验回合。B仍partial，C不推进，无增量证书。

## 1. 核验动作与结果

本次只读源码/日志/JSON/hash，并以独立脚本读取冻结natural样本和zone几何；没有Rust编译/测试、没有自然harness、没有production planner/design/append/infer/realize/bake。未将worker日志误称为本检查员自然重放。

- 当前receipt SHA256：81931acfce21dbf8143f0e52b4cc61f29cfdf8e6af8d0cd319289f1a809a693c。
- 当前10项bytes/hash绑定、10旧回执、G3-R1六份旧证据、7冻结W1输入、6其它源码全部匹配。
- 内存移除新增G4测试函数后，auto_roads.rs精确回到G3-R1 hash f4fdd474…；确认生产未改、旧测试未改，无实际源码编辑。
- run.json与run.log中JSON逐字段完全一致；6个唯一(B,C)状态、无重复，baseline前后hash一致、best=null/incomplete=false；日志runtime=2.3037681s。
- 对保存的mismatchCells独立构造几何预期集合：B3000的R1/R2各26，B2640的R1/R2各14，集合逐项匹配；均196格/0missing。没有运行生产profile求解，因此这是对保存诊断的独立一致性检查，不是新自然证书。
- 4态为shared-junction匹配拒绝，2态为street station104填方/平台约束拒绝；候选测量/候选realize/基线realize均0。不是“测量零地块”。

## 2. 必须纠正的表述与记录

### A. 结论外推（重要）

结果文档§3“两个8m平台无法满足9m交集”、§5“唯一能改变该结论的是路口/共享支路合同”、receipt unfinished同类句子，超出了本次六点实验的证据。它们与同文“不穷尽/不声称必须修改”相互矛盾。

8m是最低固定平台长度，不是要求第9米必须变高。具体确定性solver在本次输入输出的第9米有高差，是实际失败原因；不能仅凭8<9推出所有输入、所有允许profile都不可能相等。两条更长范围同高的profile在逻辑上完全可以覆盖9m交集。对于本冻结自然与该solver，若要证明全高程域不可能，仍需另给严密包络证明；本轮没有这份证明。

正确结论：**登记的六个状态均未联合通过，其中四个在当前生产profile的共享交集上不匹配，两个在街段内先工程拒绝；未测的B/C值仍未被排除。**

### B. JSON中的Mm字段实写米（明确bug）

位置：auto_roads.rs中diagnostic[abEndStation75Mm]/[bcStation9Mm]直接序列化height_at，后者返回米。

保存结果为：
- B3000-R1/R2：2.92 / 2.92，而字段约定应为2920 / 2920mm；
- B2640-R1/R2：2.64 / 2.56，应为2640 / 2560mm。

mismatch样本中的approachMm/streetMm已正确乘1000，196/26/14计数不受影响。修复后续序列化（转换为毫米整数，或明确改名M），补纯转换小测试；旧run/receipt保持原字节，通过新勘误或明确标记的派生修正版披露。

### C. R3采样少了C端平台内侧4米

代码取x=172..180、z=171..189。对从x288走到x176的向西街段，固定C端8m平台实际覆盖x176..184，加端外4m padding后，应检查**x172..184**，不是172..180。

检查员用冻结natural逐点读取，并按相同Chebyshev保护距离×0.5m的zone填方规则计算：
- 原部分范围：上界5324mm，限制点(180,178)，natural=-676mm，fillCap=6000mm；
- 完整8m平台+4m padding：上界**5314mm**，限制点(184,177)，natural=-686mm，fillCap=6000mm。

因此station104（x184）出现5324>5314并不意外。5324仍可叫较松的必要上界候选，但不能说已覆盖完整端平台；5314也是更紧的必要界，不是已证全走廊最大可行端点。不能据此再补跑G4。

### D. 计数范围要标成candidate-stage

infer2/design6/append12是新候选循环显式发出的调用，不包括harness准备阶段的road_network plan、road_extensions::append、append_declared和frontage::append等直接调用；也不包括叶子内部调用。原表“worker自身调用”范围过宽。

把准备阶段与候选阶段分列，内部未插桩继续写not_instrumented；不能因candidate measureCalls=0而宣称整个准备链从未运行frontage。候选未进入地块阶段的主结论仍正确。

## 3. 下一步选择：一个显式参数对照，不是放宽合同

现road_extensions::append_one本已允许junctionLengthM>=8；12m路的junctionRadius=max(junctionLength,9)。将**仅两条新段**的junctionLength从8显式设为9，是现接口内的新候选参数方案，不是必须新增共享支路架构/修改生产默认或审核。

该方案使两新段各9m端平台覆盖当前交集，半径仍为9，不扩大豁免；它会改变新段profile和工程负担，因此只是值得验证的对照，不保证自然工程/地块通过。既有35条路及其原参数不变。

下一任务仅固定坐标/B=3000，C使用新参数下的infer_end及C=B两态，先工程；全部source/审核/yard保护继续沿用。不需要以“当前所有高程均无解”为前提。

见ds-v41f-b01-g5-junction9-control-task.zh-CN.md。G4旧六态不重跑。B未签收/C不推进。

## 4. 检查员交付

目录bevy/work/collaboration/inspector-b01-g4-20260918/，evidence.cjs/json记录绑定、源恢复、日志一致性、诊断格点和端平台上界；review-decision.json记录限界验收。doc-tree/diff另存日志。

无业务/测试源码修改、无子agent、无Git写操作、无F盘写入。只新增审查/转发回复/下一任务和证据，追加status。
