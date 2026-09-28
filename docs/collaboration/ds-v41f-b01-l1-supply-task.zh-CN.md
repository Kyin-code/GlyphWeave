# V4.1 B01-L1-S1：让道路供给服务于真实地块需求

## 1. 当前目标

R2显式轴与W1道路版本绑定已独立验收，关闭该修订队列。下一个问题是：CLI道路工程通过，但16住宅+8店铺仍0/24。现在做生成业务，不再新建检查平台。

仅在G:/difyllmwiki/GlyphWeave；记录HEAD和相关源码开始hash，保留dirty工作区。读最新收口审查、本任务、milestone-b-acceptance，以及frontage/auto_roads/road_extensions的实际生产实现即可，不重扫所有历史、不联网新搜资料。

不Git暂存/提交/推送/重置/切分支/清理，不启动agent、不动F盘/A冻结材料。R6已验收模块只读。原b-r1-control自然困难案例冻结为unresolved，不再用四档高程扫描消耗本轮。

## 2. 本轮输入与成功口径

以已经有16m+12m生产见证的W1控制原始manifest为只读来源：
`bevy/work/collaboration/ds-b04-c1-r2-b02-w1/w1-collector-local.manifest.json`。

复制到本轮新目录，记录原始输入hash及差异。冻结其seed、自然参数、scene/origin、land-use、显式建筑kind/count/id/尺寸/insets/setback、固定入口/既有节点/道路、fill/坡度/保护合同。新增控制不得变更这些来凑通过。旧世界缺新tag不代表几何证据丢失；从可信原manifest及有效程序做新版本诊断，不写回旧包。

本轮可调整的是**新增道路候选的位置/连接拓扑/合法内部高程和确定性的候选顺序**，以及下述显式启用的需求导向道路选择；不能删需求、缩小楼/院落、移动固定根、重标zone、改平自然地形或放宽fill。

目标是得到24需求的真实联合见证，再最多一次正式512m bake。若受限预算内做不到，partial可以接受，但必须交付可重复的新增合法供给/最好候选和具体未满足原因，不能仅再抄一遍0/24汇总。找不到完整见证不是数学全局无解证明。

## 3. S1：先定位真实候选供给，不先生成新图

复用生产frontage候选/完整parcel/工程preflight逻辑，针对W1已有道路，输出有界的诊断：

- request ID、road ID、station/side、建筑和完整parcel边界、目标用途；
- 第一个真实拒绝阶段/具体原因，最好保留一至三个可核对的坐标/自然/设计高程样本；
- 每个请求尝试/接受/拒绝计数；每条路能提供的合法位置及联合冲突后的实际数量；不能把各road独立容量简单相加当全局见证；
- 为什么有maxCandidateAttempts=8192却只有30次实际尝试：区分几何候选穷尽与预算耗尽，记录road长度/站距/可用路段范围。

可新增小型Rust内部诊断/harness，默认正常生成不开启大日志。必须调用同一生产候选/审核逻辑，不复制一个弱化版placement。不要为获取报告把内部类型全面pub。

## 4. S2：有界的需求导向道路候选

在现有自动道路候选框架上做最小、显式启用的改进，使选路不再只看zone中心距离，还能看**真实可实现的新增地块供给**。

- 仅对输入确有frontageProgram时使用已验证的有效需求；新策略用明确版本/开关启用，旧无开关路径不变。格式字段沿项目Program设计，不再引入第二个重复需求源。
- 所有候选仍先经过生产corridor工程/固定节点/用途授权检查。
- 新道路与既有道路一起通过同一frontage选择及完整parcel冲突检查；依据新请求真实供给增量择优，不能用新增路长/到zone中心距离冒充建筑实现。
- 仅只读试算，隔离候选状态；被拒候选不得污染已接受plan、terrain、计数或ID。
- 排序规则确定：明确住宅/店铺剩余需求、合法新增供给、必要工程代价与稳定tie-break；避免一种请求把另一种永远挤掉。
- 固定共享节点毫米高程不变；不能为了接入把原节点拉低。使用已修的16m施工，不能绕开真实final及旧支撑。
- 接受候选后必须在真实生产final上复核frontage/development/landcover/access；若工程或院落失败要回退候选，不能只看planned。

不要实现全局优化/无限回溯。冻结上限：最多128个道路/拓扑候选状态，单条候选站点尝试仍受原request预算约束；总预检180s，达到任一上限立即停止并记录stopReason。把候选状态数、道路条数、frontage站点次数分开。不得在多个循环中偷偷各跑128实现无界放大。

如生产链耦合使上述最小试算无法安全接入，给出已经定位的最小接口边界与小复现；不要把本轮变成worldgen大拆分。

## 5. 必须测试与一次端到端准入

先用已知可行的独立小fixture测试需求导向选择（不是替换W1自然输入）：
- 两条工程可行道路中，一条只改善中心距离但不提供合法parcel，另一条能增加真实供给；新策略正确选择，不凭空加需求。
- 完整parcel冲突/用途阻断/fill拒绝、固定根高程冲突、无供给候选不获信用；候选回退不污染之前结果。
- 16住宅+8店铺数量及尺寸保持；同输入同候选顺序确定；策略关闭的legacy路径不变。
- 实现率计算要有实体/院落/access证据，不仅site IDs。

接着在冻结W1自然输入上做一次上述有界联合求解。只有找到完整可行见证（至少请求24/计划24/实际有入口24、真实yard无本批缺额、独立road审核pass）才准入最多**一次**新的正式512m generate-world及独立validate/quality/gate。生成/gate各120s/4096MiB、串行；源代码需冻结后构建debug runtime。没有见证就不烘焙，提交partial及最佳候选证据。

局部24完成时全场landcover/用途等其他缺额仍可存在，照实列出；不能把局部通过称B或整世界通过。若CLI复现与预检不一致，保留第一份证据，定位具体阶段，不追加盲跑。

## 6. 修改白名单

- `bevy/crates/core/src/worldgen/auto_roads.rs`：显式版本化策略、候选排序/有界试算。
- `bevy/crates/core/src/worldgen/frontage.rs`：提取/复用现有候选和诊断接口，保持既有合同。
- `bevy/crates/core/src/worldgen/road_network.rs`、`road_extensions.rs`：必要的候选事务/生产集成，不改已验收工程限制。
- 可新增`bevy/crates/core/src/worldgen/frontage_supply.rs`，只做生产函数复用与有界协调，不造第二套放置/审核算法。
- `bevy/crates/core/src/worldgen.rs`：模块导出、策略身份与必要接线/测试，不大拆文件。
- `bevy/crates/core/src/settlement/morphology_compile.rs`：仅严格读取新策略兼容/保留显式输入，已验收轴算法不改。
- 可新增`bevy/crates/cli/tests/frontage_supply.rs`，小型自给测试。
- 本轮结果`docs/collaboration/b01-l1-supply-result.zh-CN.md`、新证据`bevy/work/collaboration/ds-b01-l1-supply/`。

development/landcover/access/parcel_geometry和R6模块原则上直接复用只读；若必须改合同才能完成，停止该部分并报告最小理由，不放宽验证。资产/依赖/lock/自然算法/渲染/检查脚本只读。

## 7. 成本与交付

单项定向测试120s，首次必要编译300s。一个Rust重任务，不跑全workspace/core、release、1km组合、最大图、浏览器。不要重复生成旧对照。测试内部bake次数单独披露；优先内存小fixture。

交付：源码/输入hash、启用策略和版本、固定条件表、真实候选与拒绝诊断、bestLegalCandidate及联合供给数、预算/stopReason、rollback负例、正例/legacy测试、是否有24见证及CLI是否准入、每条命令/退出码/日志。旧证据只引用不覆盖。同步doc-tree:check和git diff --check。

B仍待主检查员签收。本轮结束停止等复验，不提交Git、不启动agent，不自动扩展B03/B04设施或B05最大图。
