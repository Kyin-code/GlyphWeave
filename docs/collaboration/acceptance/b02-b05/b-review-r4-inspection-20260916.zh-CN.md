# B Review R4 独立审查

日期：2026-09-16（Asia/Shanghai）。结论：**partial / needs-changes**；不签收B。主Agent仅检查、运行小探针和写任务，无子agent调用、无生产代码修改、无Git写操作。

## 1. 已确认的进展

- 回执SHA256为046deb2e27336b1528ee7ee4ecc691348d16aa1fb8323ab32eda51a14b43e78b；14项源码/文档/落盘文件/日志绑定全部匹配。
- 独立运行road_parcel_feasibility定向测试：6 passed、2 ignored、0 failed，执行3.29s。非零工程测试本轮确实执行并通过。
- 两个ignored测试会直接覆盖worker固定证据目录，**本轮没有运行它们**。其日志已检查；检查员另写只读probe重读5份落盘产物，调用当前生产joint审核及landcover::verify，正例通过：24/24/24、yardUnresolved=0、requiredUnconnected=0。
- 非零工程正例使用plan_terrain_carves/apply_terrain_carves，natural/final确实不同；这个具体修复成立，不再退回“完全没有非零工程正例”。
- 状态和requiredConnected/requiredUnconnected分列已改善；原自然四次尝试有实际测试执行日志，不再是单纯引用R2。

证据目录：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b-r4-20260916/。
证据：identity-review.json、targeted-tests.log、persisted-probe.rs、probe-results.jsonl、probe-build.log、probe-library-build.log。

为使探针链接当前源码，检查员执行一次 `cargo build --offline --manifest-path bevy/Cargo.toml -p glyphweave-core --lib`，16.28s；不是release构建，不生成世界，不改生产代码。完整core/workspace、gate、浏览器均未重跑。

## 2. 核心阻断：联合入口仍不是完整工程审核

文件：G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/worldgen/road_parcel_feasibility.rs。

新入口验证自然hash、节点表和下游yard/access，但仍没有把已冻结base的实际道路边、扩展设计全部几何、最终道路支撑与计划来源组成完整审核链。development从同一传入plan重建，不能替代道路来源/固定边保存检查。

### 独立正负例（使用真实落盘24地块正例的内存副本）

| 输入变更 | 实际结果 | 应有结果 |
| --- | --- | --- |
| 未改正例 | frontage pass，24/24/24，requiredUnconnected=0，yardUnresolved=0；独立读取surface验证通过 | 局部正例通过 |
| 固定base road中心[64,160]的final高程从0改为+1mm，natural/plan不改 | joint返回Ok，frontage仍pass/24，所有failures为空 | 明确固定道路支撑/工程来源失败 |
| 扩展design.roadMesh首顶点y增加1m，request/profile等不变 | joint返回Ok，frontage仍pass/24，所有failures为空 | 设计几何与源重建不符，拒绝 |
| 删除固定base edge，保留原nodes/其他扩展 | node audit和joint均Ok，roadEdges变2，frontage仍pass/24，所有failures为空 | 固定道路义务被删除，拒绝 |

注意：正例的developmentStatus和landcoverStatus本来就因全场未开发土地为incomplete，这三种破坏没有增加相应工程错误。本报告**不是说完整world-quality-gate被绕过**，也不是把Ok本身当pass；问题是该入口没有区分“仍有内容缺额”与“固定工程被破坏”，且任务要求的关键工程反例实际没被拒绝。

## 3. TrustedBaseContract的当前能力边界

构造函数目前：
- 仅比较base.natural_heightfield_hash与调用者给定hash；
- 将base所有node高程统一设为base.level_m×1000；
- 保存坐标→高程，不保存base边的身份/端点/实际profile义务；
- 类型字段全公开，构造函数不重建或验证base本身。

如果前置条件明确是“外部已经独立验证的等高初始base”，局部合同可以成立；但它不是一个能自动验证任意可信base的构造器。尤其已有graded扩展的base，不能把所有node强行视为同一level。结果文档“由nodes、level/profile构建”的profile部分不符合实际代码。

不要求新建安全权限框架。正确做法是从原base保存其必须保留的边/节点/高程来源，candidate只能按允许的新增操作修改；共享新节点一致并不能替代固定base边存在性。

## 4. 原自然四状态：确实执行，但三个是预先违反固定端口的反例

实际循环对每个level=8000/7000/6000/5000，把所有显式走廊的startLevelMm/endLevelMm都改成该值，然后调用plan_with_style。

- 8000重现原station45失败。
- 7000/6000/5000把连接原8000mm父节点的入口也改低，因此在接入时直接junction mismatch；这三项是在验证拒绝已知非法端口，不是保持固定入口后探索新的合法内部高程/路径。
- 本轮4个状态全部在plan阶段失败，没有一个进入joint审核、frontage或真实供给评估。这可称“4个计划尝试及拒绝回归”，不能扩大为原自然地块联合可行空间已经获得有效搜索。
- 下调状态的junction检查发生在context.design之前，因此该次运行本身不能证明7000/6000/5000三个网络的每条孤立走廊都design可过。7000孤立走廊的历史证据另列，不能推广。

原场景仍unresolved是正确的，但四状态结果不能用作容量或不可行性结论。

## 5. 评估器还有未落地分支

这些不是假设新需求，而是前轮已要求的流程没有接线：

- 自然评估成功分支把natural同时当作final交给joint；它没有使用非零正例中已存在的工程final生成步骤。因为四次都提前失败，所以该缺口未暴露。
- `plan_with_style(..., None, ...)`继续忽略原settlementProgram等设施输入；不能据此验证“原需求保持”。
- bestLegalCandidate只检查frontage_planned==24，不核对实际24可达、yard、工程失败；最终status又硬编码unresolved。这两个条件都不能支撑正确的候选判定。
- 所有plan Err写extensionCount=0只是占位：8000失败可能在部分扩展已处理后发生；不能把无法取得中间状态表示为“已证明零条通过”。应写不可得或实际前缀计数。
- 仍保留extensions.len()>MAX_JOINT_CANDIDATE_STATES的错误名称检查；虽然本次attemptedStates确实独立数到4，但道路条数和状态预算仍需保持明确命名。

## 6. 持久化证明还缺什么

落盘的五份JSON真实存在、hash正确，并且本轮独立重读验证通过。但它保存的是0m平地built_artifacts，不是非零工程正例；scene/zones/trusted/natural/final仍由测试重新构造，目录本身未保存这些必要输入。

所以可以说“部分产物序列化往返通过”，不能称完整非零工程候选可从目录独立重建。持久化测试再次从plan生成joint实体，不直接用读回entities作为joint输入；外部landcover::verify消费读回entities，但不替代所有道路/固定工程检查。

下一轮只需把已有非零正例的最小来源和natural/final一起保存并重读，不需要上传大图或新建完整发布框架。

## 7. 下一步裁定

保留R4真实进展，不重跑全部core、不继续添加不接生产的报告字段。

优先顺序：
1. 让当前三个实际工程反例通过已有道路/源审核被拒绝；明确可信base与候选输入边界。
2. 将已验证工程final步骤接入自然候选评估成功分支，完整传递原需求。
3. 固定8000mm入口不动，仅选择/调整新内部节点或合法路线，做有限真实联合尝试，不再把整体降端点当合法规划策略。
4. 保存并重新读取非零工程正例的完整最小输入，返回可复验的候选证据。

任务：G:/difyllmwiki/GlyphWeave/docs/collaboration/ds-v41f-b-review-r5-task.zh-CN.md。A保持accepted，B保持partial；无最大图/浏览器准入。
