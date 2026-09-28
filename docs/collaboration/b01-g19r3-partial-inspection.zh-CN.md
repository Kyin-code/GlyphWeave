# B01-G19R3 partial独立审查与根因定位（2026-09-20）

## 裁定

**partial-confirmed / directional-width-root-cause-localized / no-new-scope**。

核心未闭合不是未知的landing接线问题，也不是原反例地面齐平台；检查员已直接调用当前生产库，定位band_coverage区间合并的无穷边界初始化错误。保留已完成的写入原语和语义回归；仍不验收整体，不接ST18或默认世界。

## 独立核验与运行

证据目录：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g19r3-20260920/。

- 清单5/5条hash+字节一致；回执直接记录的core/CLI hash匹配在盘。core282672B/0bc5f35a6a7d6574e11e74058f1a43807a07a1ce4769139e3e5e7bece26c68e1；CLI19152B/99c90e46f1a926e61d654e74e35372e45eaddfaebacba2b16898be700205ecd6。partial回执尚未绑定清单本身，不把这次核验冒称最终证据包完整。
- 检查员2次cargo：重编CLI成功；core实际10项测试9过1失败，失败是narrow-directional-deck应拒却built。progress的“10/10全绿”及done里“6×0.6已拒”不符合日志，应在接续记录中覆盖，不重写历史日志。
- 检查员另1次rustc编译只读探针，链接本次cargo实际构建的glyphweave_core rlib，调用公开build、CollisionContext::band_coverage和member_frame。没有修改产品源码、插入生产debug分支或另写一份替代判定器。
- 检查员4次人工CLI：当前合法positive-terrace built/found→verify verified；生产write_new_owned的final-write与final-write-sync两处注入均拒绝，bundle/views均清理。此处原正常路径的创建所有权问题已有实质修复，不再重开已修的相同缺陷。
- 自然/ST18/旧世界生成0；独立成本不占worker额度。

## 1. 根因已经确切定位：无穷初始化使覆盖恒为f64::MAX

core engineering_connections.rs约3481起：
- band_interval正确给原窄平台返回约[-0.3,+0.3]，即横向0.6m。
- band_coverage却初始化low=NEG_INFINITY、high=INFINITY，再用low.min(start)、high.max(end)合并；无论加入何区间，它们都保持无穷。
- width=(-low).min(high)*2因此是INFINITY。
- narrowest从f64::MAX开始，min(INFINITY)仍为f64::MAX；f64::MAX本身是有限数，所以末尾is_finite检查通过，返回1.7976931348623157e308，而不是0.6。

公开生产函数探针实际输出（coverage-probe.log）：
- frame_interval=Some((-0.30000000000000027,0.30000000000000027))
- coverage=1.7976931348623157e308
- empty_owners_coverage=1.7976931348623157e308
- is_f64_max=true
- route_status=found

refresh_walkable_capabilities确实消费了band_coverage，只是拿近乎无限宽与1.5m比较而放行。**下一步不是继续猜“landing有没有走到闸门”，而是修复生产区间并集和缺数据语义。** 无owners/无有效frame也返回最大值，说明这是通用错误，不能只为0.6m case写特殊拒绝。

## 2. 原反例没有变成“地面齐平1m”

检查员将最初反例与当前tests/resources/g19r2/narrow-directional-deck.json进行完整JSON数据比较：一致，差异0。

- naturalMm全0；baselineMm全0；
- platform顶面y=1m，centre=(8,1,8)，平面6×0.6m；
- a/b端口在顶面y=1m；无groundConnections。

不能将平台顶面高程误称地面高程。ground_contact表示从0m地面承托到1m平台，不表示周围地面自动抬到1m。不要改成地面齐平、扩大平台或增加旁路迁就预期；真正的同高地面联合支撑属于另一输入与显式支撑关系，不是本反例的解释。

## 3. 修复不能只替换两个初始化常量

应从真实有限区间开始合并，按重叠/合法接触分组，只取包含行走线横向t=0的连通组，所需对称带取两侧余量较小者。不能跨空隙合并，也不能跳过前面的无关分量后错丢真正含0的分量。

缺owners、缺frame、零个有效测站、没有含0连通组都不得返回MAX或INFINITY表示安全；结构边缺支撑应明确失败。ground边由工程地面承托的部分与结构支撑部分要分段/按支持域判，不能通过if width==0则跳过来混合二者。

frame必须匹配实际支持层/高度/对象；不能将上层与下层平台的二维投影合并成一条虚假宽面。逐段检查不应只看windows(2)的from而永远漏掉最后端点/关键支持边界。当前检查员未运行这些新变体，不把静态风险冒称已复现，它们是原不变量的直接单元边界。

## 4. 对“2/3/4已闭合”的限定

- 接触框从实际part.id及其顶面几何构造，方向正确；端口→该结构→该part→接触点的精确关系仍应在最终变体中确认，不能由helper存在替代完整验证。
- write_new_owned创建成功后立即登记，再写入/同步；检查员两处实际原语注入已测通过，认可该修复。后续只做必要回归，不重新实现一遍。
- r3_restored_semantic_regressions本次真实运行通过，覆盖去掉填方、坡度、保护/授权、支承、单向、unsupported、落盘篡改等。保留这些有效断言，不再以旧资源存在assert代替。
- 结构边支持带仍未在独立验证端重算；修好生产band_coverage后必须同步完成，原始bad input与自洽篡改包两种拒绝都要有。
- 本轮CLI worker尚未跑；检查员的4次仅是独立核查，不冒充worker已经完成最终交付。

## 5. 预算/接续

G19R3自报cargo9/20、CLI0/30，未到额度。只提供了部分执行日志，旧使用次数仍标self-reported，不虚构完整历史账本。续作最多cargo11/CLI30，若发现漏计则扣减，不增加自然额度。

原progress有相互冲突的done/tests/open及错误“齐平”诊断，应新增/更新明确的接续覆盖记录，绑定当前0bc5f35a…；不要求单独确认稿。新任务是G19R3同目标续作，不另立G20，不扩新设施，按确切根因完成区间单测→生产调用→结构边独立验证→CLI/视图/清单一次交付。

A不变、B partial、C不推进。检查员本轮仅保存独立证据与任务文档，没有修改产品源码或旧worker证据。