# B01-G17：固定西端接入真实工程门整包（2026-09-20，主检查员授权）

## 0. 授权与目标变更

用户已要求推进。本任务**明确批准另立西端接入目标**，不继承G16/G16A的“北窗不可达/西端唯一”结论；G16A证明尝试保持关闭，原北窗可达性仍unknown，不重开。

目标：用一个完整固定候选，回答能否在真实冻结输入下保持旧support和自身pad，并让生产access返回指定BC。不是再搜出坑级数/唯一入口，不改变生产合同。

授权覆盖A～E整包：来源与规则冻结→测试侧实现→精确人工合成修复→一次自然批次中的真实性核验/D1/D2→归档与结果分支。普通合成失败自行修复，不逐项让用户确认；自然实际失败不得重跑。只在白名单越界、自然技术故障/耗时用尽或整包完成时报告。

## A. 唯一候选：G-W1（不要沿用“零新增参数”说法）

### 固定世界和场址

沿用冻结W1/ST18-R1：A=(288,96)@8000、B=(288,180)@3110、C=(176,180)@-382；AB/BC宽12、junctionLength9、shoulder3、rise80、fill6000。仅北侧homes station18，中心(270,157)，12×10，pad255格[262,150,278,164]、设计2390；入口/parcel/yard/setback/需求不变。AB/BC和全部既有实体保留。

本包明确允许改变接入位置意图：不再限定x282窗口，采用BC西端x170..176附近接入；指定road_id仍为BC，不允许连到road-auto或AB就算成功。

### 规则与carve顺序（唯一、无现场调整）

基于G-1d：生产生成该候选pad/apron/connection并身份绑定，**删除唯一原connection**，保留pad与全部apron逐字段不变，然后按T1→T2→T3→T4→T5→W1→W2追加下面7块。不是保留原全长connection再加七块；G15原稿的歧义由本表取代。

|块|cx|cz|halfW|halfD|target mm|blend m|priority|disturbance|profile|
|---|---:|---:|---:|---:|---:|---:|---:|---:|---|
|T1|260|162|1.5|7|2390|2|2|160|None|
|T2|257|162|1|7|2160|2|2|160|None|
|T3|254|162|1|7|1930|2|2|160|None|
|T4|251|162|1|7|1700|2|2|160|None|
|T5|248|162|1|7|1470|2|2|160|None|
|W1|210|162|38|7|-10000|2|2|160|None|
|W2|171|165|1|9|-500|2|2|160|None|

上述数值是**本次明确授权的测试候选输入**，不声称从自然推导、静态已通过或必定恢复自然。W1低target只是提交给未改动生产apply的参数，不能直接把输出赋为natural，也不能事后恢复保护值。缺graded influence、其它同/高priority、zone和编码等必须照生产规则处理。任何结果按实际输出判，禁止人为补台阶、改负target、调blend/位置或增加第8块。

控制C0：原G-1d北候选（只pad/apron），优先用G12保存面与结果，不独立重跑旧harness。实验C1：G-W1。没有南侧镜像，没有第二个新方案，没有级数搜索。

## B. 先完成实现与人工验收

仅auto_roads.rs的#[cfg(test)]可改；access.rs/生产worldgen/frontage/solver/schema/CLI/默认值/审核不动。复用已验收G11显式carve→同口径栅格→生产access以及G12观察器，禁止借用会重建旧connection的candidate_final_terrain验证新列表。

人工fixture必须覆盖：
1. 唯一connection删除、7块精确参数/顺序；pad/apron和base全身份不变，base++proposed不全局重排。
2. W1低target在graded有效/无graded/高priority压制/同priority重叠/zone clamp时的实际结果；T5与W1重叠取生产最大target，不编造“恢复自然”。
3. 完整proposed影响域×所有既有保护域、pad255、三带距离单位；只看target!=saved值不算支撑变化；无完整采样/非法输入明确拒绝。
4. 水/保护分区/no-cut/fill按现有判据，新增工程矩形影响也不得绕过固定合同；未执行审计写not_run，不冒称full-engineering pass。
5. 输出和预算门实际接到单一batch_clock，序列化后写前检查；发生晚过期保留诊断记录但status=incomplete，禁止成功/witness。
6. 生产access返回road_id不是BC的反例必须被拒；不强制goal、不跳过其它道路/实体/水、不改变0.30/0.25/120/6000。

以上不足或必须改生产接口时停止并报告具体阻断，不因用户说推进就放宽测试。正常人工修复可以重复精确运行，日志用新文件；不要运行会重写旧create_new产物的原自然入口。构造是否工作由自然批次回答，不把“合成PASS”当工程通过。

## C. 运行前冻结与一次真实性门

新目录ds-b01-g17-west-entry-engineering-bundle。先写rule-table/create_new预约，保存执行版源码快照（运行前、原始字节）、两份配置/输入hash及预算；清单最后再出，禁止post-run代码冒充执行版。

自然批次最多一次，单一batch_clock从最早自然动作之前开始。固定manifest/seed/scene/zones/water重建一次真实自然栅格与计划；保存原始毫米自然面、完整既有实体/grounding/真实carve签名和保护域表，避免后续反复用代理。

- AB/BC profile逐点对G8，35/27/21基线计数以及G12四点/既有工程面一致性核验。
- 使用真实base生成/核对完整基线面；若与G12冻结baseline编码hash不同，先区分合法编码差异与值差异，真实格值不同为input_mismatch，停止，不在新基线上默默继承旧证据。
- 对归档published自然与本次真实自然做全格比较（同坐标/单位），保存exact-match数量、差值范围/代表点与hash。仅archivedNatural<=engineeredSurface绝不是同源证明。
- 两份自然不同，不伪造相等、也不改候选参数。仍可在同一次批次中用**本次权威真实自然**检验冻结G-W1：其构造已固定且不依赖代理重算；报告明确旧自然交接假设未证或失败，所有结论只绑定本次真实输入。不因此追加自然重建。

## D. 同一批次完成D1→D2→一次决策

### D1：旧support、pad、实际工程限制

保持旧基线不可变。base_carves从完整真实实体生产生成，候选列表按A构造并身份验证。枚举全部proposed组件（保留pad/apron及七块）的完整硬覆盖+blend影响域，与全部旧实体保护域的交集，production frontage同口径before==after。域由真实实体决定，不使用G15 id猜测代理，不限于AB/BC四点。

完整pad255格必须==2390，入口和现有no-cut/fill/zone/water/障碍/parcel前检不放松。对于W1等负target不能直接从target值推断no-cut成败，依据既定生产判据与最终工程输出，不允许切削自然、破坏旧support或绕过保护区域。额外构造的实际影响域及自然/工程fill变化单列；若必须新解释一个合同才能判，则记needs_contract_decision并停止，不自行改审核含义。

预期工程拒绝正常输出：最多保存每类8个代表违反点和总检查范围。D1失败则access=not_run，不为了完整矩阵浪费下一图或试第二方案。取首失败后的其它独立小诊断仅可在固定集合/剩余预算内完成，不做全域新搜索。

### D2：仅D1通过后生产access

同一base++proposed列表，真实natural→生产apply/zone夹限→生产毫米编码器形成完整counterfactual candidate面。support/pad与面读回同口径核验；缺采样不是可走，数值不一致属技术错误。

完整实体、道路、障碍、水都交生产build_access_graph，目标候选观察开/关各一次且完整图逐字段等价；路由120m包括facade→exit段，不把自己指定的西行折线当生产必走路径。保存实际link/path/distance/road_id、观察原因及有限样本，返回AB/auto-road等其它road就判wrong_road，不改绑定规则。

### 结果分支

- D1拒绝：state=engineering_rejected_at_fixed_candidate；真实输入验证后，此七块候选关闭。只给具体阻断，不宣布所有西端方案无解。
- D1过、access拒绝/错路/超长：state=access_rejected_at_fixed_candidate；记录实际类别/路由输出，不现场调参。
- D1+access通过：state=counterfactual_diagnostic_pass_not_witness；在**同一交付内**给最小生产一致性/最终terrain/完整realization集成提案，明确本次counterfactual与production-authored encoder的差异和version消费者。此包不实施集成、不调用realize、不签收B。
- 技术错误/超时：state=incomplete，保存已有证据和准确阶段，禁止第二次自然运行补字段。

## 累计预算（整个自然批次，不是每个阶段）

- harness≤1，plan/natural重建≤1，固定AB/BC复原一次；新候选≤1。
- 自然阶段总墙钟≤180s，涵盖准备、真实性比较、D1、栅格、access和最终序列化；人工编译/合成单列。
- 完整counterfactual面≤3：基线≤1、G-W1≤1、同批C0核验面≤1（优先用保存C0避免多生成）；每面512×512。真实自然面保存1份，非候选面。全部格数/调用内部成本如实记录。
- build_access_graph≤4：G-W1观察开/关≤2，C0仅必要一致性核验可另≤2；不得启动独立G11/G12自然入口。内部其它实体link/routes单列。
- 自然对比扫描仅冻结512×512一次；其它只限D1受影响域/面必要核验/≤32归属诊断点，不做新的候选或站位扫描。
- measure=0，production candidate_final_terrain=0，baseline/candidate realize=0，bake/世界发布=0，128/clock/全场搜索=0。反事实面合成有成本，绝不能写“所有terrain合成0”。

## E. 最终交付与停机

新目录完整保存：冻结规则/预约/执行快照、原始自然面及同源比较、可复用实体与carve/保护域表、基线与候选工程面/毫米hash、D1逐阶段矩阵、生产access及观察对照（若运行）、一次自然run及日志、累计命令和人工失败日志、总报告、清单/回执及只读核验。总保存量≤30MiB；超限先缩减非必要verbose样本，不能丢必需面/身份绑定还声称complete。

旧G15七块静态“0可能格”仅历史假设，不作验收前提；G16A原目标证明仍关闭。旧文件一律不覆盖。要修复运行后工具时另存版本，不能将其hash写成刚才执行版。

按dsh-inspector-reporting-protocol.zh-CN.md一次汇报：一句话结果＋来源分层结果矩阵＋最多3条新信息＋预算＋一个后续结论。不要中途提交“已收到批准”。A不变、B partial/C不推进；此包不自动允许西端第二方案、加块、改target或再跑自然。