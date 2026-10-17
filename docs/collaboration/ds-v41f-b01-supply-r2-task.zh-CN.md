# V4.1 B01-SUPPLY-R2：追加事务与生产实现信用收口

## 1. 范围

先读`docs/collaboration/b01-supply-r1-inspection-20260917.zh-CN.md`。只在G:/difyllmwiki/GlyphWeave工作，保留35边append、RequestBudget、结构化realized IDs等有效进展。原自然/W1输入、carve合成、fill/坡度/旧支撑合同保持；不重开R2/W1/R6验收队列。

不启动agent、不Git写操作、不动F盘/A冻结材料、不自行签收B/C。本轮不要求24、不再跑冻结128扫描或carve诊断。先做确定性小fixture，消除已明确的提交漏洞。

## 2. A：真正的追加事务，超时不发布

- 追加会话在独立working plan上运行；完整正常完成/确定性工作预算结束才按合同提交。墙钟中止丢弃本次追加全部修改，保留进入追加阶段的可信legacy基线。
- 所有重步骤前后检查截止，包括初始baseline realize、candidate realize以及写回之前。墙钟到限返回结构化error/incomplete诊断，不能`Ok`把prefix交给bake继续发布。
- 若诊断需要best-so-far，放非权威diagnostic，不让world/plan审计从partial成功前缀复现。无需调度框架/新依赖。
- 注入clock测试至少三个点：首次提交前；第一次真实成功后准备第二轮；realize执行期间跨限、返回后尚未commit。三者plan/ID/计数/已承诺几何与会话入口一致，外层生成不能当成功使用。禁止只重跑trip_at=0。
- 正常确定性候选预算触顶与墙钟超时分开：前者按明确语义返回可重现partial结果，后者不能决定成功世界的前缀。

## 3. B：滚动承诺集合，不从字符串推断归属

- apply_realization直接使用已验证的`realized_ids_by_request`，按request ID精确匹配、去重、一ID只能归属正确request；不能用contains/rsplit猜测，也不能禁止合法ID来绕过。
- 会话维护完整当前committed sites/entities/parcel/固定入口和已验证报告；每次成功原子更新。后续候选测量必须将这些真实几何当占用，不从只含roads的plan空取。
- 第二轮守卫必须保护第一轮刚提交的ID/几何，不仅会话起始集合；按request数量有增也不能交换旧ID。失败/超时不更新任何部分。
- 小反例：request `homes`与`scene-0`，一栋homes建筑ID含scene-0，不得同时记入两个需求。生产parser先确认请求合法。
- 两轮fixture：初始0，第一轮成功A；第二轮A丢失但同请求B+C有增，拒绝并保留A；正常A+B通过。必须有非空承诺正例，不能仅在冻结0地块上比较两个空列表。
- 在现有小供给fixture上增加至少一次真实阶段验证失败后的原子回退；可用测试边界注入破坏最终yard/access，仍调用真实verify来拒绝，不要求自然随机地形恰好触发“偷地”。测试机制不得暴露为生产跳审入口。

## 4. C：非递归共享生产实现审核，不维护第二套弱检查

### 4.1 所有道路和真实final

当前新road_support_status只看profile道路，不能替代基础道路支撑合同。优先从现有road_network生产审核中提取**无规划递归**的完整支撑/实体审核叶子，参数为已经可信构造的plan/natural/final/entities；已有审核与供给协调共用该叶子，阈值和覆盖范围完全不变。

必须覆盖ungraded base整段支撑、graded精确band、fill/cut、固定节点/边和extension设计来源；不要在auto_roads内调用会再进入自身的整套plan_with_style。若源重建已由候选append_one和可信基线承担，写清对应调用，不省略来源断言。

canonical final构造复用正式生成相同基面/量化/scene origin/权威输入。对worker报告7.998/7.9975差异先给小样本、确切量化顺序，再共享生产路径；禁止放宽毫米误差、全局更改自然/carve语义。若确实只能通过新生成语义解决，停止此项并给最小接口提议，不伪称等价。

小验收：同计划正式路径与预检路径支持/院落/入口样本一致；合法正例通过；base道路非中心支撑+1mm、graded边缘+1mm、删固定base边或改来源、入口断开各自命中正确拒绝。可复用现有测试fixture，不重开R6测试矩阵。

### 4.2 新地块不能消费旧缺额

保留旧13m²缺额作为历史事实；按source parcel/surface划分旧与新，新增parcel yard必须无缺额，旧缺额/失败不得换身份或扩大。failure比较用稳定来源/分类集合，不只len。用现有LandCell/SurfacePatch.parcel_id及verify结果复用，不新建并行账本。

反例：初始旧yard缺13，候选旧yard补13但新yard缺13，必须拒绝新增信用；不要求全场既有缺额立刻归零。

## 5. 白名单

- core/src/worldgen/auto_roads.rs：会话事务、滚动承诺、归属和小测试。
- core/src/worldgen/frontage_supply.rs：共享实现协调、按parcel信用、诊断/fixture。
- core/src/worldgen/frontage.rs：已有realized IDs/RequestBudget必要接口，不改门/支撑阈值。
- core/src/worldgen/road_network.rs：提取等价非递归审核叶子及就地测试，不改变合同。
- core/src/worldgen.rs：仅共享正式final构造的最小接口/调用/测试，不改apply_terrain_carves语义、不大重构。
- settlement/landcover.rs必要时仅提供现有按parcel核算信息的只读/附加接口；不改变realize/verify阈值、所有权或总账语义。其余development/access/parcel_geometry/R6只读复用。
- 可新增cli/tests/frontage_supply.rs的小型生产等价/超时拒绝测试；CLI生产代码仅确需阻止incomplete发布时的最小接线，须明确记录。
- 新结果docs/collaboration/b01-supply-r2-result.zh-CN.md；R1结果可追加勘误。
- 新证据bevy/work/collaboration/ds-b01-supply-r2/，create-new，旧receipt/世界不覆盖。

完整路径前缀为G:/difyllmwiki/GlyphWeave/bevy/crates/（源码白名单所述core/cli）。资产/依赖/lock/渲染/自然算法只读。

## 6. 测试预算、交付和停止

每项定向测试120s，首次编译300s，单Rust任务。以小型内存fixture为主；只允许最多一次最小合法尺寸、≤512m的自给TempDir bake，用来比较真实生成/预检/独立审核，不产出正式控制世界。没有必要就不bake。

禁止全CLI/worldgen/settlement/workspace、release、大图/浏览器、128冻结扫描、原自然四档、重复全部R6。A/B/C三个小目标分别报告，不把未完成C包装成“全部完成”。

交付：源码hash、真实命令/exit/log、三种clock位置、合法ID归属、两轮承诺/回退、共享道路审核前后等价、量化小反例的处理、旧与新增yard分账、实际bake次数与未完成。保留冻结旧文件身份，不为更新tag重写旧包。

doc-tree:check和git diff --check通过后停止等检查员。不改全局carve、不提交Git、不启动agent、不自行签收B。
