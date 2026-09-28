# V4.1 B01-G9：station18局部支撑合成来源诊断（不改策略）

## 1. 目标

G8限定负结果已接受，station18 no-cut已解除，但加入建筑后现有BC支撑采样6502/6987变为2390。下一步不再加大B/C，只查清这两个before来自谁、candidate哪个carve触发变化、保护检查在什么范围拒绝。

先读b01-g8-st18-first-increment-inspection.zh-CN.md及已保存G8完整profile/raw trace。下一步是否更改合成/保护合同必须另行审批，本轮只能诊断及提出设计。

## 2. 唯一输入和采样范围

只用G8 **ST18-R1**：B3110、C=-382，原A/B/C坐标、两新段junction9/width12/shoulder3/fill6000/rise80，旧自然/zone/需求/35路/义务不变。不跑R2、不改高程、不再测station90。

仅构造station18的北/南候选，优先homes请求；shops与homes的地面参数/几何是否一致先通过既有数据核对，不为重复几何另跑一套。

固定4个采样坐标：
- 北冲突(268,171)，预期base sample6502，加入北候选后2390；
- 南冲突(268,188)，预期base sample6987，加入南候选后2390；
- 北整平带边界控制(268,173)，横距中心7m；
- 南整平带边界控制(268,187)，横距中心7m。

控制值未知，交生产函数读取，不预填。目标两点分别距BC轴9/8m；路面6、整平带7、transport9等范围应从现合同确认。所有点都是同一份冻结natural，不调自然值匹配预期。

## 3. 允许一次诊断重建，不是再次跑G8

先用已保存JSON/合成carve fixture验证输出schema/字段单位/空贡献/失败日志；再进行≤1次专用diagnostic harness，总runtime≤90s。入口create-new预约写入在重建之前，失败即停，不重试。

用可信manifest/source准备一次原35边基线；按已保存G8请求追加AB/BC一次（或读取完整计划若已足够）。核对新段profile与G8保存值逐点相等，source hash/旧身份一致；若差异停止，不迁就旧数字。

从production frontage::candidate_site获取两个真实候选实体（不road fallback）。按production相同顺序得到base carves和分别加入候选后的carves；候选北/南要各自相对同一base，不串行累积两个建筑。

允许对这4点调用现有plan_terrain_carves/apply_terrain_carves和相同zone/no-cut sample规则；**不调用整轮measure/PlacementState搜索，不做full-field采样，不生成新world，不调用realize/candidate_final_terrain。** 调用次数和顺序明确记账。

## 4. 完整保留实体到carve到采样的来源链

每个相关carve至少记ownerEntityId/kind、component（主pad/前侧connection/apron）、原数组序号、profile/request hash、cx/cz、half_w/half_d、实际blend_m、priority、target或本点profileTarget、natural与fill上限。

每点给出：
- 自然值、BC longitudinal profile值、该点相对pavement/flat-top/footprint/blend的位置；
- production base sample与加入候选后的sample（before/after），与G8两点旧rawDetail核对；
- 对该点有影响的全部carve来源，outside/full-cover/blend关系；不是只列最后一个或优先级最高一个；
- base阶段最终值的实际贡献来源，candidate主pad/connection/apron何者能覆盖并改变该点；
- full_priority/full_target/graded_influence选择分支的解释，是否丢弃已有blend填方、是否有其它更高优先级硬支撑。

源头归因必须与原生产leaf结果一致。可在测试区计算有限解释性逐项数值或逐前缀/单项剔除对照，均限定同4点、记录每次调用；这些是诊断，不是新默认算法，不改变实际审核决定，不以一个重写的“模拟生产”作为唯一证据。

每实体/组件归属不可由ID子串猜；保持原生成顺序和显式owner映射。若唯一来源无法确定，完整列出并说明原因，不硬选。

输出两条整平带控制点是否仍保持road profile，说明冲突是在精确整平带还是额外受保护带；不因此自动缩窄保护范围或加容差。

## 5. 结果与设计建议边界

先确认/否定以下候选机制：生产硬支撑覆盖是否重置到natural+candidate target，从而移除其它对象先前提供的blend fill。结论需有owner、具体组件和数值支持。

如果机制确认，只提出一份后续设计建议，可比较保持既有支撑/候选连接高程设计/显式合成语义等取舍，明确哪些会改变生成结果/版本/合同/旧证据。**本轮不实现任何策略变更、不改生产审核、不把新building覆盖旧支撑视为可以忽略。**

不尝试移动建筑、抬B/C、减setback/院落、提高fill、放开no-cut、扩大junction或改地形。不另开参数搜索/全场诊断。

## 6. 白名单、预算与交付

- 仅auto_roads.rs现cfg(test)及新文档/证据；不更改worldgen.rs/frontage.rs/road_network等生产源、内部public接口或默认。
- ≤1诊断harness、≤1固定计划重建、北/南2候选、4坐标，≤90s；合成测试与编译单列。候选measure=0、baseline/candidate realize=0、candidate_final_terrain=0、bake=0。
- 不跑G3–G8旧自然test/128/clock80/独立全场carve/CLI世界/gate/release/browser/max/workspace；不Git/agent/F/A。
- 自然诊断失败/输出不完整立即停止，保存日志，不自行重跑。

新目录G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g9-local-support-provenance/，新文档G:/difyllmwiki/GlyphWeave/docs/collaboration/b01-g9-local-support-provenance-result.zh-CN.md。

保存预约、规则、input/source hash、一次run、实体/组件归属与相关carve包、4点before/after/控制数据、精确阶段和调用计数、日志、回执。重复引用的旧证据hash不修改。

顺带向G8报告追加四项勘误：station18共8attempt；R2 st66设计150/首缺289和5/全缺855和999；G6-R1旧源330309/319bdceb…而非G6的294054/1994ef49…；生产前缀UTF8 bytes81685（SHA256 ba6deb95…不变）。旧G8回执保留，不为勘误额外运行。

doc-tree/diff/hash检查后停止，结果为diagnosis_ready_for_review；没有修复实施、没有真实增量证书，B partial/C不推进。
