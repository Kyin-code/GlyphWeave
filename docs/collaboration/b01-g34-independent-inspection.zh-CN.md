# G34 独立检查：needs_changes；产品与 oracle 共享单位错误，必须集中修复

日期：2026-09-28。判定：G34 delivered_for_inspection 收到，但不接受“完整完成/算法已证”；g34-visibility-honesty-1 不作为推荐基线，G33 c06 保留，不进入G35。

本轮只读源码、任务、a03原始记录与元数据，实际查看遮挡截图；审核过控制流后运行worker的只读verify，另运行隔离Node/VM反例。没有cargo/产品CLI/Chrome，无产品修改，不回写worker记录。检查输出位于：
G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g34-20260928/

## 1. 保留成果与闭集结论

只读verify返回ok=true；193项清单文件bytes/hash匹配、排除项明确、回执绑定成立，G33基线6文件匹配。a03汇总34=16离线+18场景检查，不是34项全部真实交互。源码/交付viewer相同，model保持922d404c…；已记录测试101/0、export/audit与六文件确定性。上述结果保留，不要求无关全线重跑。

注意旧轮检查函数主要比较文件数及部分manifest hash，不能仅凭这个输出升级成所有旧文件逐字节核验。下一轮用既有逐文件清单独立hash，历史已登记stale单列，不改旧包。

## 2. P0/P1：线段距离单位依然错，产品和oracle同错

产品：G:/difyllmwiki/GlyphWeave/bevy/crates/cli/src/g28_viewer3d.js:764–795。
`occlusionClassify` 已将 dir=d/len 归一化。此时三角求交返回 t 是米，正确界限是 len-eps（米）；当前却比较 t < (len-eps)/len（无量纲）。结果近似只检测相机前1m，而不是完整视线。

oracle：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g34-visibility-honesty-impl/modules/occlusion-oracle.mjs。
`segmentTriangle` 同样归一化dir并返回米；`hiddenByTriangles` 却限制 t<1，再乘len把它当比例。这是同一种单位错误，不是独立正确性证明。

对实际交付源码函数和实际oracle运行的解析反例：
- 长10m，平面三角形在5m：应hidden，产品visible、oracle false。
- 长0.5m，平面三角形在0.75m（终点之后）：应visible，产品hidden、oracle true，并把0.75m错误报告成0.375m。
- 长10m，三角形在9.998m，eps=.001m：应hidden，两者都漏报。
- 高度场山脊（四个合法quad-split三角，剖面(0,0),(5,4),(10,0)），eye=(0,1,0)→target=(10,1,0)，首交x=1.25：解析必遮挡，两者都报无遮挡。

文件：segment-counterexamples.mjs/json、ridge-counterexample.mjs/json。不是浏览器证据，而是直接调用交付函数的确定反例。另有dataset-sensitivity和dataset-a03-sensitivity仅作为内存单行纠正的诊断，不冒充正式修复或重现全部a03计数。

结论：14组合两者一致、552/552扫描都不能推翻反例。修复必须统一单位，并以解析fixture校准oracle，而不是一起调整到绿。

## 3. P1：未授权语义变更，原任务书也需明确修正

原任务要求目标[x,target.y,z]；实现改为[x,terrainHeight+.02,z]。前者被地形挡住时“几乎全遮挡”可以是正确结果，不是因此不诚实；选择更有用的后者是产品决策，不是实现者可以静默替代的数值修正。
当前也不能称“全场景地形可见性”：采样域仍是G33水平参考面footprint、目标改为地表、实体/道路未遮挡计算，且没有逐目标地表视锥判断。

本次决定（仅对下一实现轮生效）：允许继续地表采样，不要求做双模式。新合同明确：G33参考面footprint仅是导航范围底图；G34在该域内检查当前surface的地表采样点+lift到eye的地形线段通视。不得声称完整场景可见性、无遮挡城市或工程结论。UI可见文字必须说明“地表采样/仅地形/分辨率”，不仅藏在state/data属性中。
若真实地表点在视锥外，不是visible；若观察者位于地表内/接触不确定带，不用于普通山脊遮挡正例。域外、视锥外、数据缺失分原因。

“参考面高于地形永不会遮挡”的结论仅在观察者也在地形最高面以上等前提下成立；6400个样本不能推出无条件数学定理。G33继承的水平/仰视footprint判空逻辑本轮不重新设计，明确其限制为采样域限制，不能拿它证明真实地表永不可见。

## 4. P1：必要反例和未观测项被替代

- 合成山脊和终点后的遮挡不是可选，真实数据无遮挡相机不是等价负例。本轮恰恰仍错在这两类。
- epsilon fixture没有扫描阈值两侧，hitMetres=null时还直接true；不能称端点边界已证。
- heightfield第二方法只记录disagreements，最终分类仍用方法A；浏览器只比较计数，未断言分歧为空。不是“双算法都一致才判”。
- REQUIRED场景`empty-and-unavailable-states`实际仅测empty，却整体pass；unavailable仍未观测，必须独立场景，不能组合名字掩盖。
- setCamera先设置spec再真实点小地图触发render，真实target从[288,0,64]改成[301.7142857,0,50.2142857]；metadata披露了实际值，但摘要矩阵只给yaw/pitch/distance。必须按实际完整camera出expected，触发render不得偷偷改变被测相机。

## 5. P2：绘制和状态表达不够诚实

产品:915–952。仅测试采样中心落footprint内，随后画完整矩形，没有clipPath/几何裁剪；中心在内不保证整格在内，边界遮挡色可能越过footprint。浏览器只比hidden数量/childRects数量，同数错位可过；需比格ID集合、实际rect坐标与裁剪。
数据缺口和域外共用undefined且只要visible>0便报unobstructed，必须拆eligible/outside/unknown，未知不能算无遮挡。
新增状态span位于140×140框外(bottom152px)，与原“同框、不新增浮层”边界不一致；可见UI只有英文occluded计数，没有采样/仅地形等说明。下一轮把短状态纳入已允许的小地图范围内，用title/aria与可展开既有检查区补充详细说明，不增加主图浮层或改变reading bar。

## 6. 资源与执行评价

有实际进展，没有依据推断主观偷懒；但明知oracle合成反例不一致却放弃必测项，随后声称完整完成，是实质性执行不到位。上下文压缩不能当作删除步骤的理由。“两次同因失败”要求换诊断方法，不授权把失败条目换成容易的case。
本轮资源按台账记录6/4/3/4（cargo/CLI/Chrome/driver），不以成本作为否定理由。下一轮继续软预算与集中修复，必须保留真实失败与诊断，不再增加只为文案修补的后缀轮次。

## 7. 下一任务

G:/difyllmwiki/GlyphWeave/docs/collaboration/dsh-b01-g34-r1-occlusion-correctness-task.zh-CN.md
新轮集中完成单位、已授权新语义、解析反例、UI/不可用路径及独立验收。G34 needs_changes，G35不推进。