# B01-G6 frontage诊断独立审查（2026-09-19）

## 结论

**needs-changes**。保留28次natural_above_design_level与4次existing_support_changed这一生产首拒绝分布，以及同两态零接受/未realize的观测；不接受“32条完整正确诊断已完成”，不批准H1升高B试验。

主要问题不是审核器需要放宽，而是测试侧把道路实体当成建筑实体，导致支撑范围证据错误；并把首个违规点缺额当成完整支撑面的最大缺额。修复取证后再讨论新参数，B partial/C不推进。

## 1. P1：sample_entity_for返回BC道路，不是候选建筑

位置auto_roads.rs:5020–5026：函数忽略_sample，按road_id返回plan.entities中的道路。调用方4877–4902随后把该道路的尺寸/中心/朝向当作候选建筑，算building+2m padding。

结果32行全部写buildingFootprintM=[112,12]、supportCheckRange=[174,172,290,188]，辅助natural min/max全部相同[-687,1796]。这其实是整条BC道路加2m范围。真实请求建筑是12×10；每个站点/侧别有各自中心和支撑范围，北侧yaw180、南侧yaw0。

具体反例：J9-R1/homes/st18/北侧，center=(270,157)。
- 正确footprint=12×10，yaw180；
- 正确support_check范围=[262,150,278,164]；
- 原记录范围=[174,172,290,188]，连生产首违规点(275,150)都不包含。

检查员按原effective请求、生产candidate_site/bounds几何和冻结natural独立读取：32行footprint错、32行support范围错、32行aux极值错、16行yaw错。production首拒绝自身没有因此改变，但测试侧附加几何证据必须重建。

## 2. P1：696只是首违规点最大差值，不是抬升充分量

support_check按z/x顺序遇到第一处违规即返回。该点的requiredFill为负，能证明当前设计太低，却不能代表矩形内最高自然点。

检查员不运行生产planner/solver，只读取已有32行设计值、冻结请求和natural，以真实building+2m padding逐格复核：28条保存的首违规坐标/自然/设计值均可复现。完整支撑最大no-cut缺额如下（同几何两请求相同）：

| 站点 | 北侧完整支撑最大缺额mm | 南侧完整支撑最大缺额mm | 备注 |
|---|---:|---:|---|
|18|27|110|两态|
|42|797|913|两态|
|66|858|1002|两态|
|90|743|887|仅J9-R1 no-cut；R2已进入existing_support_changed|

最明确反例：st66南侧center=(222,203)，真实支撑=[214,196,230,210]；首违规(217,196) natural155/design147，差8mm。但同支撑内(230,210)自然1149，差**1002mm**。假设整个pad确实上抬696后设计843，仍低于该点306mm。

因此即使额外假设所有pad能均匀+696，28条no-cut记录中仍有20条未满足完整支撑no-cut。**1002也只是当前这些支撑面针对既定design的几何必要抬升统计，不是批准B+1002的新候选，更不是全链通过证明。**

## 3. P1：提高B不等价于BC所有pad均匀抬高

生产road_alignment::solve求的是lower bounds的逐点最小坡度包络。B是端点条件，C、自然下界与坡度共同决定内部profile，改变B只抬升其实际支配的部分；不允许将solver输出整段平移。

用本轮现有记录即可说明：B=3000、junction9、station42设计1105。若只把B提高696至3696，起点平台经坡度传播到station42的下界为3696-80×(42-9)=1056，仍低于已有1105；在其它条件相同且设计可行时，该站无需随B变化，不能推导+696。此为现生产包络公式的条件分析，不是新自然试验。

H1必须撤回“等价”“可消除28次”的保证式依据。可以保留“某些高程调整值得以后研究”这一未定量假设，但本轮不授权任何新B/C值，不改变fill/no-cut/carve合同。

## 4. P2：原始拒绝字段与完整profile没有落盘

- raw trace的RejectedDetail中对existing_support_changed本来有conflicting_entity、before_mm、after_mm；当前重组rows时丢弃了这些字段。natural/design为null是诚实的，但不是原始detail只有null：真正有用的工程变化信息已在内存产生却未保存。
- 28 no-cut与4支撑变化是不同检查阶段，不应把existing_support_changed写作support_check的note或“拓扑原因已确认”；源码中它是加入建筑carve后既有硬支撑采样变化，需保留实际对象与before/after再定因。
- planProfiles只有request、len、first、last三个profile摘要，没有profile_mm全数组/生产design或完整候选plan。不能称完整profile包。
- detailComplete=true只核对samples/details数量，不验证几何、绑定字段是否正确，也不代表raw detail已完整持久化。

## 5. 可以接受的事实与验证范围

- G5算法勘误正确；现文件SHA256/BLAKE3分开记录。道路追加阶段与full final未运行的分层纠正保留。
- 当前receipt SHA256：b1d5c5cf78bdfa11c67688c60dbc6bdb556e65956c0ac7f1130800395a4cedc7。
- 12项当前path/bytes/hash绑定、12历史回执、9项G5证据、7冻结W1输入、7其它源码匹配。
- 内存移除G6新增测试和两个helper后auto_roads精确恢复G5 hash c00758a3…，确认生产未改。没有实际恢复/改写源文件。
- run.json与日志JSON一致；32唯一state/request/station/side；28/4汇总一致，28个生产首违规点由冻结natural和生产同序必要检查复现；所有点zone容量读数复核一致。baseline前后hash保持。
- “0次fill类别”表示本次保存的首拒绝未落在该类；不能据此证明抬高之后fill仍不限制，或所有后续审核都通过。
- 本检查员仅离线几何/栅格/源码/日志检查，无Rust编译、测试、自然harness、planner/design/append/realize/bake，无新高程实验。

## 6. 下一步

执行ds-v41f-b01-g6-r1-diagnostic-repair-task.zh-CN.md：先修候选实体绑定/完整原始字段/完整profile，补合成几何回归；再最多一次原两态诊断补证，禁止新参数。该补证为新轮单独预算，不覆盖G6旧证据。避免继续以错误辅助几何设计H1。

证据在bevy/work/collaboration/inspector-b01-g6-20260919/support-review.json、integrity-review.json；审查/任务/回复另存。无子agent/Git/F盘/业务源码操作，B partial/C不推进。
