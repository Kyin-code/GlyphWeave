# B01-G6-R1 诊断修复独立审查（2026-09-19）

## 1. 分开给结论

**技术交付：accepted-diagnostic-repair。执行过程：budget-noncompliant。**

最终采用产物的候选实体绑定、完整几何/支撑极值、raw trace及完整profile包可以接受，关闭G6诊断修复项，不要求再重跑原两态。但不能宣称严格遵守任务书：授权1次自然补证，worker报告实际执行4次。真实地块增量仍无，B partial，C不推进。H1未批准，也不因本次修复通过而自动获批。

## 2. 独立检查，不仅相信状态标志

- receipt SHA256 f55488804d598f0e3febbb93042dfd0f1e3920d8e0da2c4b9dbef3bf4fe5fcd8，12267bytes。
- 回执递归21项path/bytes/SHA256绑定全部匹配（其中部分路径重复，实际19个不同路径）；13份历史回执、17项G6/G5登记证据、7冻结W1输入匹配。
- 与G6快照比较的7个其它源码hash匹配。新增G3以来整个测试区域从内存副本排除并还原旧测试相邻两行换行后，auto_roads精确还原G1 hash e6af8cb6…，确认生产部分未改，没有实际修改文件。旧错误helper的函数定义已不存在。
- run.json与最终run.log中的JSON完全一致；32行、4包raw trace，baseline前后hash一致。未重跑自然harness。
- 独立--exact运行两个合成回归，2 passed/0 failed：g6_r1_candidate_binding_uses_the_building_not_the_road；g6_r1_first_point_deficit_is_not_the_support_deficit。
- 检查员将所有32行与上一轮独立冻结栅格复核逐项比较：12×10 footprint、朝向、中心、support范围、自然min/max及max坐标、必要高程区间、缺额全部匹配。
- 四个独立raw-trace文件与run.rawTraces相等；每包8 sample/8 detail/0 accepted。逐条与rows.rawSample/rawDetail关联匹配，共32行；不是只检查countsMatch标志。
- 检查四条AB/BC profile：两AB各85点、两BC各113点，与各自生产design.profileMm逐点一致，request与design.request一致；原始自然source hash一致，完整roadMesh/supportMesh存在。
- 用已有只读BLAKE3工具对规范化profile数组逐项重算4个profile hash，以及本轮rule-table的BLAKE3，全部匹配；所有32个site/sample design level与相应BC profile[station]相等。
- 所有保存profile均检查相邻坡度<=80mm/m、两端9m平台固定值一致。此为文件内容/合同必要一致性复核，不是重新运行production verify或final world审核。

## 3. 技术修复验收范围

### T1 建筑绑定：通过

bind_candidate_site真实调用production frontage::candidate_site，再取site.entity；没有road fallback。requestId/roadId/中心/parcel等有拒绝分支；合成测试故意改坏请求/中心/parcel均拒绝。北侧yaw180、南侧yaw0。32行不再把BC道路当12×10建筑。

### T2 原始字段：通过

28条no-cut首拒绝保持；4条existing_support_changed保留完整conflictingEntity/beforeMm/afterMm。
- (196,171)：5074→1960mm；
- (196,188)：4265→1960mm；
- 两个请求各出现一组，故4次attempt，不是4个不同位置。
conflictingEntity都是本态新增BC道路；站点90候选site设计1960，detail natural/design为null，两者分字段没有相互覆盖。

**这说明新增建筑carve影响了本次已暂存BC道路的硬支撑采样，不自动等于历史冻结35条路被损坏，更不能叫已证拓扑冲突。** 现存profile在x196对应station92为2120mm，before采样5074/4265、after1960与该值属于不同来源层，暂不据此定因；本轮没有复算carve合成链。

### T3 缺额：通过并收紧措辞

28条no-cut记录的首点差值与完整支撑最大缺额确实不同；4条support-change记录首点no-cut缺额为null，不应把“32行0个相等”说成32个有效数值对照。

最大完整支撑缺额1002mm@(230,210)，design147；28条里20条即便假设uniform+696仍不能满足no-cut。H1“696足够”“升B等价整体profile升高”已撤回。1002仅为当前采样支撑的必要条件统计，不批准任何新B值。

### T4 profile与schema：通过

完整design/profile及hash已有，raw原始信息未再丢弃。最终JSON中未发现单键Ok/Err的Result包装。五个标志与独立检查一致。此前两版schema/断言问题不会使正确的最终产物失效，但不能抹掉产生它们的运行成本。

## 4. 执行预算不合规，且不能只拿采用那次2.41秒当总预算

任务明确要求：先合成回归后最多1次原两态补证；一次自然输出不完整则停止，不自动为补日志重跑。worker自报自然harness实际执行4次，已经超额3次，不因同参数/无新搜索/耗时短而合规。

本次能从保存材料独立核验：
- #3 superseded-schema-defect完整run：2态/4measure/32attempt，JSONseconds=2.4041826；
- #4 superseded-nested-assertion日志：真实测试失败，测试计时2.19s；断言发生在结果组装阶段；无完整发布JSON；
- #5 adopted完整run：2态/4measure/32attempt，JSONseconds=2.412542；
- #1/#2没有对应完整独立日志/源码快照。#2据述在AB profile长度断言失败，最终源码中该断言位于候选measure之前，不能仅据最终源码确认其旧版已做完两态4measure。

因此：
1. 4次自然重放作为worker主动披露的执行事实记录；已有保存材料也能证实至少3次实际自然执行，单次上限违规已确定。
2. “4遍均4measure/32attempt”若按其自报为16measure/128attempt，须注明**自报、尚不能全部独立核实**，不能冒充检查员实测累计数。
3. 全轮准确累计耗时目前unknown；不能用最后2.41s替代。已有时间片段短于120s不等于已核验全轮。
4. receipt预算字段publishedNaturalReplays=1应解释为最终采用数；按其自述另有1次完整发布后作废产物，发布与采用要分开。

不要求再自然重跑来补成本账。不允许伪造缺失日志；若终端/调度历史仍有记录，仅只读补录，否则写unknown、说明哪一段缺证。纠正是文档/回执附录，旧证据保留。

## 5. 后续边界

本轮技术修复关闭，**不再授权自然运行，不给H1放行**。先用已保存profile/trace/source做离线分析或整理下一提案；需要新B/C、重放carve或任何自然pipeline调用时，必须另列范围和总预算再获得明确授权。

后续测试流程应先对现有序列化fixture执行schema/完整性断言、检查AB/BC点数及所有geometry字段，再运行昂贵路径；日志直接完整落盘，不将运行进程输出接到会提前关闭管道的Select-Object -First。失败即停，不自行把一次额度滚动成四次。

## 6. 检查员动作与产物

本轮只做源码/日志/JSON/离线既有极值比对、2个合成exact测试及只读BLAKE3计算。自然重放/生产design/append/infer/realize/candidate_final/bake均0。无业务/测试源码编辑、无agent、无Git写操作、无F盘写入。

证据目录bevy/work/collaboration/inspector-b01-g6-r1-20260919/含evidence.cjs/json、两个exact测试日志、规范化profile文件及检查日志；review-decision.json分列技术和过程结论。
