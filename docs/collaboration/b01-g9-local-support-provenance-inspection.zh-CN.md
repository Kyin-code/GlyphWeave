# B01-G9 局部支撑诊断独立审查（2026-09-19）

## 1. 裁定

**accepted-mechanism-with-offline-attribution-supplement**。四点数值、候选connection的剔除对照和精确整平带控制支持既有blend填方被full_target覆盖这一局部机制。原run的owner标签错误，不能当作可靠归属；检查员已从冻结输入与G8保存设计离线恢复决定性索引，不需要为了修标签再自然重跑G9。

允许下一轮做候选连接方式的设计与合成元数据回归，不授权生产修复、自然参数实验、carve重放、realize或bake。B partial/C不推进。不能将方案B称为唯一可行方向或已证明可行。

## 2. 实际复核

- receipt 13545bytes，SHA256 3de62ae8bf3b145be6d567a67ea1b3850dacdb9ada4946a4b013ab61c545d3ff。
- 13项当前回执绑定匹配，9项其它生产源hash、7冻结W1输入、15项非文档G8证据、台账7项绑定保持。
- auto_roads生产前缀UTF8 81685bytes / SHA256 ba6deb959c3e3b913ed2c65a887f4860b1cddf505cfd2ae2fb71758997224d34保持；当前源是映射修复后的版本，不能称其已执行本次自然run。
- run.json与run.log内JSON一致；4点natural从冻结栅格读取一致；两冲突点before/after与G8原始detail一致，控制点2230不变；所有保存prefix终值与直接production apply输出一致。
- 候选idx2完全覆盖对应冲突点，移除它回到before；候选idx0/1不满足这个结果。用记录的候选几何恢复idx2为connection、idx1为apron、idx0为pad，不依赖错误标签。
- 独立运行g9_preflight_provenance_schema：1 passed。此为合成fixture，不是新的自然diagnostic；测试通过不代表优先级相等即可证明身份正确。
- 用已有只读BLAKE3工具核对引用G8的AB/BC profile数组hash；没有运行planner/design/append/measure/realize/bake或terrain composition。

## 3. 检查员离线恢复了具体来源，不必再重跑

依据冻结road-network-plan中的edge/site顺序、road_network::entities/site_entities规则、G8完整AB/BC design，以及生产plan_terrain_carves的几何和稳定优先级顺序，仅重建元数据索引，没有应用carve或生成地形。

37道路实体+21旧site对应79个base carves；对已保存4点的144条数值关系逐项核对priority、outsideX/Z、full/blend、target等全部匹配，144条旧错误标签也正好对应排序前索引，确认该置换规律。

| 生产排序后索引 | 离线恢复的真实owner/component | 几何与意义 |
|---|---|---|
|2|generated.roadside.scene-0.generated.road-network.scene-0.10-11.1 / pad|旧住宅，中心(256,120)，half13×13，priority1，target8m，blend120m；北冲突记录中把值推到6502的步骤|
|49|generated.road-network.scene-0.25-26 / road_pad|旧道路，中心(320,224)，half36×5.5，priority10，target8m，blend120m；南冲突记录中把值推到6987的步骤|
|78|generated.road-supply.g9.ST18-R1.bc / road_pad|新增BC，中心(232,180)，half60×7，有profile；两7m边界点的2230来自BC自身station20|

这些是与记录数值相符的离线来源重建，不冒称重新执行了生产自然追踪，也不声称记录中每个可能相等的贡献只有唯一来源。

## 4. 原证据仍有无效字段与后修代码限制

1. ownerEntityId/ownerKind/component均不可信，不只是两个字符串；原schema.ownerMappingVerified=true也应在派生说明中作废。该标志实际只检查protectionScope数组存在和baseCarvesAtCell非空，不验证身份。
2. 错误标签还污染了数值：candidateSites.roadCarveBlendM和cells.bandNotes.bcRoadCarveBlendM记录120，但真实BC blend为**97.6259999871254m（约97.626）**，不是120，也不是18。两冲突点仍在blend范围内，核心机制不变。
3. 最新tag_carves虽按稳定优先级排序，但逐项只assert priority相等与label非空；没有注释所称的完整geometry sequence比较。同优先级pad/apron或两个旧住宅互换，仍能通过。应加几何/实体/组件强绑定的合成反例，不重跑自然。
4. 当前source-hashes绑定的是运行后修过的源码。需注明runGeneration=pre-mapping-fix、currentSource=post-mapping-fix；没有旧快照就写unknown，不以当前源码hash证明原运行归属已修。旧run/receipt保留。
5. reservationHashBlake3=7715eabd…实际是BLAKE3(JSON序列化reservation文本字符串)，不是预约文件原始bytes的BLAKE3。独立复算原文件为686e9cf0290518b1792f8b4672267bd10c9389437861f91f3b1efed68404ad4a；字符串编码值与run一致。明确hashEncoding即可，文件SHA256绑定本身正确，不声称篡改。
6. receipt.status.milestoneA写not signed off会回退已签收里程碑。A于2026-09-09已正式签收；应说明本worker无签收权限/不修改A状态，而不是A未签收。随下一文档勘误，不重出旧回执。

## 5. 下一步设计为何还不能直接实施

候选pad设计2390，连接会穿过before6502/6987的保护采样，再接近路面profile。简单把整个连接target改成某个道路profile值并不能同时保持这几类值；简单截短carve使其不触碰保护带，也不自动证明仍有连续可行的行人通路。

下一轮必须引用现access/入口审核的宽度、坡度、路径与final实体合同，设计能连到道路且保持既有支撑采样的路径。不能用不再触发保护检查但实际断开的连接取代真实实现。若现模型无法表达该连接、或几何/坡度必要条件矛盾，应如实给阻断及最小接口提案，不保证B方案必行，也不自动转全局合成策略C。

## 6. 下一任务

见ds-v41f-b01-g10-connection-design-task.zh-CN.md：离线采纳归属补充、补强测试元数据映射、防止旧字段误用；以station18固定几何做连接方式设计，仅现有证据/源码/合成测试。生产逻辑不改、自然调用0，不再重放G9。

本检查员新证据offline-owner-recovery.cjs/json、evidence.cjs/json记录全部交叉比对；无源码修改、无agent、无Git写操作、无F盘写入，doc-tree/diff另存日志。
