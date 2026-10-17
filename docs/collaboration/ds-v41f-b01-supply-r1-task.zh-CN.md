# V4.1 B01-SUPPLY-R1：修正供给算法的基线、回退与实现信用

## 0. 目标和边界

只在G:/difyllmwiki/GlyphWeave。先读`docs/collaboration/b01-l1-supply-inspection-20260917.zh-CN.md`，保留本轮有效S1诊断与v3入口。B仍partial，不自行签收、不推进C；不启动agent、不Git写操作、不改F盘/A冻结材料。R2/W1/R6保持已验收，不重开。

这次**不修改apply_terrain_carves/自然算法/fill/坡度/保护/旧支撑范围**。先解决新增S2算法的真实缺陷，不根据122状态误导性结论启动全局地形重构。无需让本轮达到24，也不新生成正式512m世界。

## 1. 修复事务回退和累计账目（先做小红测）

- 当前`total < saturating_sub(total,baseline)`永远不成立。按request ID对照新total与baseline，任一需求倒退拒绝；相同计数但丢失/替换既有site ID、几何、固定入口也拒绝。既有义务不凭总数相同抵消。
- outstanding必须来自当前可信的实际实现集合，不累计减掉以后已丢失的信用。baseline包含进入追加阶段前已有的对应请求实现，而非硬编码零。
- 克隆试算只有完整验证通过后提交；失败、零收益、某请求减少而另一增加都不能污染计划、terrain、ID、计数。
- 加真实rollback测试：基线住宅2/店铺0，候选变住宅1/店铺1拒绝；同总数换site身份拒绝；正常纯新增通过。允许用测试专属注入结果驱动事务守卫验证，但至少一条通过真实生产阶段构造的失败验证回退，不能只有算术单测。
- diagnose跨road传剩余count和剩余站点预算；多路正例不超需求、不超累计maxCandidateAttempts，accepted IDs及顺序与frontage::append一致。所有request traces汇总真实尝试/拒绝，不能只取第一个trace冒充总计。

## 2. 把“verified”接到真实实现链

保留轻量预估用于排序，但它只能叫predicted/planned。提交道路前用已有函数构造候选完整工程final、候选实体/开发/地表、yard realization和AccessGraph；据真实实现且有入口的稳定ID计增量。

复用已验收叶子函数，不重写R6 checker或另一套碰撞算法。注意调用边界：在auto_roads内调用会重新运行auto_roads的road_network::verify_with_style可能递归；将阶段协调放在合适的内部模块，或复用已验证候选的工程叶子审核。必要时说明小接口抽取，禁止大拆worldgen。

正例必须显示planned、realizedWithAccess、yardUnresolved和road工程状态；负例至少覆盖“规划可行但最终地表/入口不完整”，该候选不获verified信用。遗失yard或断开入口应命中对应错误类别，不能只触发无关JSON错误。

## 3. 在正确冻结基线上做追加求解

S1已经重建正确35边；S2不能把自动程序改v3从33边重新开始并声称旧路保持。

- 从原manifest可信重建并校验的W1完整基线（31基础+2显式+2原自动）进入**追加阶段**。原27节点、35条道路及其设计/标高/来源、既有21个义务site和所有固定输入均保持。
- 添加前后对稳定身份、边端点/profile、节点高程、site几何做精确比较。原auto.0/.1已占用，新增ID不得复用碰撞。
- 可在现有内部接口支持trusted base的追加试算，无需发明通用checkpoint/大JSON存储平台。基线信任来自原manifest的生产重建和已有来源合同，不信任产物自报pass。
- 如果v3生产入口仍设计为“从头生成替代v2”，保留为明确的另一种新输入语义；但不能用其33边结果替代本轮固定35边追加实验。两种模式结果分别报告，不修改冻结原件。默认legacy v1/v2布局/有效身份不能漂移。
- 在基线、计数、实现链修复后只运行一次有界冻结追加评估：总128状态、总180s、站点受请求累计预算；没找到见证则partial，不强求耗尽预算。

## 4. 候选范围、预算与可重复性

明确当前候选集：节点、4方向、长度、由tier_for选出的宽度、infer_end策略；若只扫描12m就写12m，不宣传“所有授权tier穷尽”。无需为本轮扩成全局优化或加入零收益多步道路；可选状态空间不是全局解空间。

`sweepExhausted`只表示当前定义列表用完。不得从无单步正增量推断24全局不可达，亦不得推断改carve是唯一出口。

- 同样状态预算、同样输入应得到同样成功plan/报告；不能把wall-clock超时产生的不同前缀写成权威成功plan。
- 墙钟到限返回结构化incomplete/error且不提交该轮成功前缀，诊断可单独保留best-so-far但不得用于发布。用测试可控clock/cancel检查截止边界，无需引入依赖或真实sleep长测试。
- 统一候选测量、复核、诊断的截止/状态预算；不能扫描阶段180s，之后再无限做32个复核。重步骤前后检查即可；无法细粒度中断的单次最长时间如实披露。
- 以确定性工作计数重放审核，禁止放宽审核比较忽略不一致。

## 5. carve冲突只补最小诊断，不改规则

在**保留35边**的上下文，复用生产采样函数复验(168,220)及实际候选。记录：自然值、无候选final、有候选final；每个参与carve的owner、priority、flat/blend范围、profile target；冲突点属于路面/整平/运输/外过渡的哪一层。

已知正确几何：auto.0宽12、中心x160，点x168横距8，平顶半宽7/运输半宽9。不是“9m平顶外1m”。

提交一个小确定性冲突测试和最小后续设计提议：哪些既有承诺必须保持、怎样拒绝冲突/允许合法新增、是否需要版本化工程策略。不要实现全局max合并、不下调支撑要求、不删除既有义务，不声称修此就一定24完成。若真正冲突无可行追加，拒绝保持合法结果是允许的。

## 6. 白名单

- `bevy/crates/core/src/worldgen/auto_roads.rs`：供给事务、计数、预算/模式和就地测试。
- `bevy/crates/core/src/worldgen/frontage_supply.rs`：完整实现协调、可信基线诊断、小fixture。
- `bevy/crates/core/src/worldgen/frontage.rs`：共享候选/剩余预算接口与测试，原工程阈值不变。
- `bevy/crates/core/src/worldgen/road_network.rs`：最小可信基线/阶段接口，不改变工程审核合同。
- `bevy/crates/core/src/worldgen.rs`：仅新增协调调用/版本接线或读取现有carve诊断的测试，不改carve合成语义。
- `bevy/crates/core/src/settlement/morphology_compile.rs`：如必须，仅严格保留新显式策略的接线，不动已验收轴算法。
- 新结果`docs/collaboration/b01-supply-r1-result.zh-CN.md`，旧结果追加醒目勘误，不改旧receipt。
- 新证据`bevy/work/collaboration/ds-b01-supply-r1/`，create-new。

development/landcover/access/parcel_geometry/R6工程函数复用只读；若确需修改生产语义或白名单外，说明最小接口理由并将该部分标阻断，不自行大重构。内部类型不全面pub。

## 7. 验证与交付预算

只跑本轮相关小fixture及frontage/auto_roads供给定向测试；单项120s，首次编译300s。允许一次修复后冻结追加诊断180s，无新正式CLI bake、全CLI/全worldgen/全settlement、全workspace、release、最大图、浏览器或原自然四档搜索。内存final实现可以运行，任何测试内bake单独计数并说明必要性。

交付按项列结果：事务rollback、真实实现信用、35边基线保护、请求级累计预算、超时不发布、纠正候选范围、carve小诊断。需要给bestLegalCandidate=null或真实有增量见证，不用“all work complete”概括partial。

记录源码/原输入/冻结基础hash、候选状态与站点真实累计、旧世界完整性、命令/exit/log。对旧报告的33/35、122范围、计划与实现信用、CLI测试数量/内部bake披露追加勘误。doc-tree:check及git diff --check通过后停止等复验，不自行签收B、不提交Git、不启动agent。
