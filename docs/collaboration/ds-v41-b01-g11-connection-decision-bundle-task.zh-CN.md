# B01-G11：连接规则一揽子决策实验（可转发授权任务）

## 0. 目的与授权形式

接收本任务后可连续完成A～E全部阶段，中间不再发“请批准下一步/请确认已收到”的回复。普通编译/合成失败自行在白名单内修复并精确重测；自然实验只允许一次，不得以修复序列化/字段为由重跑。完成整包或遇明确越界阻断才报告。

目标：在固定ST18-R1/北南station18两个窗，判断两种不再跨肩部硬覆盖的连接生成规则能否同时满足完整旧support等值、完整pad等值及生产access到指定BC。即使全部通过也只是测试专用反事实诊断，不是生产realization或地块增量。

G-1b step4a accepted-bounded-negative，不重跑；工具合成验收已关闭，不重开历史台账。顺带完成step4a审查列出的来源/调用口径/实际管路断言与超时门修复，不单独交付说明轮。

## A. 固定两种方案，先离线冻结，不允许自发搜索

输入保持ST18-R1：A=(288,96)@8000、B=(288,180)@3110、C=(176,180)@-382；AB/BC 12m、junction9、shoulder3、rise80、fill6000；北/南homes station18，设计2390；建筑/parcel/入口及请求需求不动。homes/shops重复几何不另算容量、不新增候选。

两种规则：

- **G-1c / parcel-clipped connection**：取生产原connection硬矩形与本候选parcel的矩形交集，产生单个新矩形，不使用格点mask。北应为[268,161,272,166]，中心(270,163.5)，half=(2,2.5)；南应为[268,194,272,199]，中心(270,196.5)，half=(2,2.5)。保留target2390、priority2、blend2、disturbance。交集须由实际site/原carve推导并与这些坐标核对，不能硬填几何掩盖输入不符。
- **G-1d / no connection component**：仅删除该候选的唯一connection。pad及所有apron完全不变，任何owner/profile等不借此改动。

此任务明确不再要求候选硬矩形连续覆盖入口到路面；能否借助既存工程面通行由现合同access检查决定，不允许先凭几何缝隙认定全域不可达。两规则都是新测试专用候选生成语义，不是生产已支持；生产合成/支撑保护/access阈值不变。

最多2规则×2侧=4候选评估，每个独立叠加同一基线，不累积。冻结表在任何自然动作前create_new写出，记录规则来源、源hash、预算、状态顺序（G1c北/南，G1d北/南）与阶段门。

## B. 实现与合成，允许自行完成到通过

源码白名单仅auto_roads.rs #[cfg(test)]。沿用已验收身份/字段/归属/局部采样/时钟工具，新增最小规则构造与诊断入口；不改worldgen/frontage/access/solver/schema/CLI生产文件或公开接口。

合成必须验证：原geometry/parcel交集；connection唯一且只改/删该组件；所有其它组件完整签名不变；同基线独立评估；采样缺失/非法值拒绝；raw与夹限两列；support全集的枚举范围非空和漏格反例；完整pad255格；结果与工程面使用同一carve列表；使用生产build_access_graph并取该候选的link，不能过滤掉会影响生产选择的道路/障碍；缺地形数据不得视为空地/可走；发布前晚过期返回incomplete。

实际自然管路须逐格断言decision值与生产apply一致，cap的无限值用null+flag。预约和最终发布检查接在真实入口，非孤立fixture。编译和精确合成可反复修复重测，独立日志累计计数，不运行宽泛全套/ignored自然测试。

无法在白名单内通过上述合成或必须改合同/接口时停止并指出具体阻断；不得临时放宽测试以放行自然。

## C. 单次自然批次：累计预算，而非每态预算

- 自然harness≤1；冻结计划/自然基线重建≤1；AB/BC固定profile恢复一次并逐点对G8，准备叶子单列。
- 候选评估≤4，whole natural batch≤180s，包含准备、支撑检查、工程面编码和access；一旦超时余项not_run，结果incomplete，不重新申请本包内自发第二次自然运行。
- baseline反事实工程面≤1、candidate反事实工程面≤4；每面最多当前scene512×512，按需惰性生成。明确计为**测试专用反事实栅格构造**，不能将这些成本记作“所有final terrain=0”。生产frontage_supply::candidate_final_terrain调用仍0。
- 生产build_access_graph≤4次（仅进入access阶段的候选）。图内其它实体route属于内部调用，不把它们冒称0；每条路由原120m/6000阈值不变。
- measure=0、baseline/candidate realize=0、世界bake/发布=0、全场参数/地块搜索=0、128/clock/carve搜索=0。允许上述有限栅格构造与受影响支撑枚举，它们不是旧轮次“4点采样”预算，必须单列真实格数/时间。

输入manifest/seed/scene/zones/water与旧证据一致，基线保留所有既有实体和AB/BC，不得加入人工8000mm测试台面。四候选分别从同一个不可变基线出发；进入每个阶段前后查剩余时间。

## D. 每个候选的完整检查链与条件分支

### D1 旧支撑与pad

base_carves由生产生成并全身份绑定；proposed是该候选原carves经唯一规则处理。顺序base++proposed，不全局重排。支撑检查域按生产frontage完整规则：**全部proposed组件（pad/apron/connection及blend影响域）与所有既有实体保护域的交集**，逐格frontage同口径before==after。禁止仅查旧4点或仅查connection。

候选building+2m闭矩形255格必须等于2390；保留no-cut/fill/zone/water等现有判定，不因试验跳过强制合同。有早期违规即保存准确原因、坐标/前后值及已检查范围，后续阶段not_run；预期工程拒绝不使整个批次中断，继续下一个固定候选。

### D2 条件式access，不隐含完整production-pass

仅当D1检查通过且时间充足，生成该候选显式base++proposed的完整反事实工程栅格，再调用生产build_access_graph。

工程面由冻结自然栅格毫米样本→同口径生产apply及zone/protected夹限→生产毫米编码器→完整BakedTerrain形成。以同一份carve列表读取support/pad与access，在验证过的整数格上校验编码/读取后毫米一致。它是**冻结栅格驱动的测试反事实面**，不是AuthoredSeed生产final-terrain已验证。不得一边用修改后的carves查support，一边用旧规则栅格查access。

道路、障碍、水与候选实体完整传给现有access逻辑；记录候选link.status、road_id、path、length，只有connected且road_id==本态BC才是该项通过。exit/facade0.30、HALF_WIDTH0.75、坡度0.25、路由120/6000全部不变。路由None只按生产status记，不编造坡度/水/耗尽细类；局部直线失败不证明所有绕行失败。

如果反事实面不完整/缺采样、身份或pipeline不一致，属实现/输入错误：立即整轮停止，输出incomplete，不继续用它下结论、不自然重跑。任何预期合同拒绝则记录并继续剩余固定状态。

### D3 结果矩阵与终点

每侧每规则一行：geometry、旧support、pad、其它已执行前检、access、资源、firstObservedFailure、not_run原因。需要区分两规则、两侧，不能双算homes/shops或推成24地块能力。

- 四候选全部早期失败：给出确定失败类别及代表点；不再加宽/改高/发起新搜索。
- support/pad过而access失败：终于获得真实候选工程面上的路径结果；不改access合同或自动选C-1。
- 至少一候选support/pad/access通过：记counterfactual_diagnostic_pass_not_witness。只交后续生产一致性/完整realization的最小集成提案，不实施、不调用realize、不生成世界。

无论哪个分支，完成四态（或技术错误/时间耗尽）后停。负结果不外推所有几何/全局不可行。

## E. 一次交付、自动自检、无需用户逐项转达

使用新目录ds-b01-g11-connection-decision-bundle，预约/规则表/执行源码hash冻结在先；产物完成→清单→回执→只读校验。源码最好保留本轮执行快照或明确不可变commit/hash+完整差异，不用文件长度当源码证明。

提交一个总报告：方案矩阵、结论、阶段门、源/输入绑定、累计命令/合成运行/自然计数、实际栅格格数和耗时、旧证据取代声明、step4a非阻断更正。失败日志保留，不覆盖旧证据。hash清单与回执不互相循环绑定。

普通合成失败无需重新审批；实际自然失败不重试；发现超出白名单、想增加第3方案/更多自然运行、改no-cut/fill/access/protection/需求或生产合成语义，才向主检查员提出新审批。

整包完成后才汇报；无需中途要求用户确认你已经理解。B partial、C不推进，A不变。一次反事实通过也不签收B。