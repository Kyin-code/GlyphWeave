# V4.1 B Review R4：补全联合预检的实际执行与工程边界

先读 G:/difyllmwiki/GlyphWeave/docs/collaboration/b-review-r3-inspection-20260916.zh-CN.md。

保留R1 F1/F2/F3及R3已通过的合成平地24地块测试。不重写现有checker，不要求CLI新增全平地开关，不扩大为全局城市优化器。原B任务功能目标不变。

## 1. 定义新模块职责

现有road_parcel_feasibility本质上是单plan汇总器，不是256状态搜索器。你需要二选一并写明确接口：
- 对外候选入口持有冻结input/自然源/原base义务，执行真实规划/工程及独立审核；
- 内部汇总器仅接收经过上述流程验证的上下文，降低可见性/命名避免任意plan被当已验证候选。

不能继续宣称audit_joint_plan直接做了它实际上未调用的corridor/append_one验证。调用已有函数不等于其前置条件自动满足。

允许改：G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/worldgen/road_parcel_feasibility.rs；worldgen.rs仅必要模块/测试工程入口/管线接线；road_network.rs、road_extensions.rs、frontage.rs仅复用接口所需的小范围调整，保留原安全约束。可新增bevy/crates/core/tests/b_joint_pipeline.rs或等价测试与自己的work小runner。若现有工程步骤需要提取为可复用小函数，限定保持旧行为并写文档，不复制另一个terrain carve实现。

不改A输入/世界/runtime/证据、原自然算法、fill/坡度/保护/水域/固定入口判据、依赖/资产/许可证，不提交/推送Git，不启动其他agent，不改F盘。

## 2. 固定义务和来源

从可信base plan的nodes、edges、实际profile/level构建固定节点合同；覆盖没有extension的base节点、被删除节点、共享节点以及新节点。

- base节点实际0m而fixed要求8000mm必须拒绝；义务节点不存在必须拒绝，不能is_some_and略过。
- fixed map不能由待审核的新plan自我生成；核对base源/输入身份，不擅自移动/删除原节点。
- request端点高程、actual profile端点、共享node、road edge对应必须一致；不能只让两个伪造request相等就通过。
- 原scene/natural hash/source不符要明确拒绝（或只能由构建受信上下文的入口接收，且入口保证此项）。
- 保留原manifest的settlementProgram/housingMinimum等义务，不在通用入口传None绕开。

测试包含检查员audit-probe的固定base高程冲突/缺失节点/错误natural hash，以及有效base+扩展节点的正例。不能以“零需求所以允许忽略固定道路”为理由跳过。

## 3. Natural与Final明确分开，真实工程正例

原0m flat正例可保留；另加合法且非零工程差量的合成正例。不要仅把flat高度改个常数后仍让natural==final。

使用生产工程步骤从natural和候选计划生成final terrain，在final上执行实体支撑、入口、access和yard；独立审核同时拥有原natural、计划来源与final。原natural不可原地覆写。

该正例仍明确是test integration fixture，不是原自然案例或CLI certificate。24住宅/店铺及两宽度链原目标保持；至少一个实际工程点的natural/final毫米高程不同且符合预算。若无法安全复用生产工程步骤，报告具体接口阻断，不复制算法或假装工程阶段发生了。

产物写自己的新目录，保存最小输入/自然及final高程、road/site/parcel/entity/surface与报告，重新读取后调用独立生产审核。删yard patch、改固定门、final高程+1mm、source hash错、共享node/profile不一致必须拒绝。持久化证据不是伪造世界发布目录，不绕过原CLI审核。

## 4. 状态和访问统计

JointAudit保留development/frontage/surface等报告的status、failures、必要incomplete原因；拆分engineeringStatus/contentStatus，不把所有场景unplanned自动当工程失败，也不丢弃真实fail。

- 函数Ok只意味着评估完成，不意味着可bake/24满足。
- 所有required link必须逐项connected：用requiredConnected/requiredUnconnected和ID列表；optional connected不能抵消required disconnected。
- required=0时不推断24个需求已实现；请求从冻结原输入取得，不信任候选自报数量。

## 5. 实际联合状态评估（本轮核心业务交付）

将“256”落实为真正尝试的联合候选状态计数，在进入一次状态评估前计数，失败状态同样消耗预算；扩展道路段数另设字段，不能混为state count。

不要求发明全局优化器。允许由小runner或Rust生产模块提供有限确定性的候选集合，复用之前合法设计线索/新节点高程候选，按统一base和已有约束进行联合验证。每个候选要经过合法接入、final工程及完整parcel冲突/入口，而非仅design-only。

冻结自然输入沿用examples/b-r1-control.manifest.json与已有自然raster；不换seed、改原用途、放宽fill/缩小楼体或删16+8需求。保留已有固定节点，新节点在合法区间内选择。

实际对原自然案例运行一次最多256状态的评估，记录每个状态的ID/父状态、关键输入变更、接入结果、真实已接受住宅/店铺与地块数、拒绝原因；缓存自然与AlignmentContext，禁止再逐个启动几百次cargo run/整读scene。

如果前序无法进入评估，明确naturalJointEvaluation=not_run及阻断；不能再引用R2搜索把它写成R4 evaluated-unresolved。实际执行但没有24见证则unresolved，保存最好的合法结果与缺额。不要求捏造全局无解证明。

## 6. 资源和验收

只跑受影响模块/小集成测试；不跑完整core/workspace、release、最大图、浏览器或外网研究。单项定向测试/预检120s，首次确需编译可单独300s；超时保存进度，不自动重启或提高上限。

本轮不强制完整512m bake。实际联合见证达到24并通过工程时，最多允许一次原自然512m generate+独立gate，各120s/4096MiB；没有见证不运行，仍可交出有效的联合评估partial。正例用上述测试harness持久化，不称generate-world成功。

## 7. 交付

新增 docs/collaboration/b-review-r4-result.zh-CN.md；新证据 bevy/work/collaboration/ds-b-r4/：
- 最小源码/输入/运行时身份、命令/退出码/日志；
- persisted-positive/及重新读取审核的计数和负例结果；
- natural-joint-evaluation.json：真实attemptedStates、extensionCount分列、最好合法候选、缺额与失败原因；
- 固定义务/自然来源/访问统计反例；
- syntheticControl、nonzeroEngineeringControl、originalNaturalCase、naturalJointEvaluation、cliIntegration分别写结果。

纠正文档中z224南侧/z160北侧标签；R2原diagnostic表仍有requiredFillMm，旧文件不可改，新结果明确纠正字段，不说旧表字节已经更新。

完成交ready_for_review或partial，不自行签收B，不推进C，不Git提交/清理/启动agent。
