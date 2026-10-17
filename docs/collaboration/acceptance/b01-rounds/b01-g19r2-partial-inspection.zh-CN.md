# B01-G19R2 partial独立审查（2026-09-20）

## 裁定

**partial-confirmed / positive-regression-unresolved / new-contact-rule-rejected**。

接受worker如实交partial和保留未完成列表；不验收“不变量已实现”这一强表述。源码中已有部分修复，但合法正例失败，label前驱/独立重采/回归覆盖仍缺，并发现新的统一0.25m低实体豁免误放行。续作是G19R2同一目标的上下文接续，不是重新申请自然试验或新设施开发。

## 独立检查

证据目录：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g19r2-20260920/。

- 在盘core engineering_connections.rs为245678B，SHA256 cdf8ae9aa51bfb73f467f532a3620402bbda4e0c24f3f61ea28fc270ae70ea2a，与partial回执匹配；CLI、access、auto_roads与G19R1审查版本一致。
- 检查员重新cargo build CLI成功；定向5测试复现3过2失败，失败均为approach在楼梯脚被stair-terrace::tread1阻挡。没有执行历史自然测试或全库回归。
- tests/resources/g19r2不被.gitignore忽略，case_root已改向该路径，迁移方向正确。git status显示当前仍untracked；应称“可随版本控制交付的位置”，而非声称已经提交/跟踪。检查员未做Git写操作。
- partial目录实际仅expectations.json、receipt.json、step01-tests.log。没有完整运行账本，因此12次cargo/0CLI是worker自报，不能独立确认全部历史命令或停止原因。

## P1：0.25m统一低实体豁免不是安全的接触规则

代码core engineering_connections.rs约3228–3233：在区分obstacle或结构之前，只要solid.max.y <= here.y + MAX_SUPPORT_STEP_RISE_M即continue。函数未接收capability，规则对轮椅/车辆同样生效，而且尚未检查是否真的沿该实体上表面行走。

检查员人工小场景：16×16平面、直线ground边(2,0,8)→(12,0,8)、查询wheelchair；显式障碍x6..8,z6..10,y0..0.2，未有坡道/台阶边，也未改变底下平地。

结果wheelchair-low-solid：build exit0、route found、verify verified；输出路径仍y0，穿过0.2m高实体。仅改障碍top=.3，wheelchair-taller-solid正确ground_edge_blocked_by_solid拒绝。

这不是“允许脚接触地面”，也不是可接受的现实路缘特例：即便行人可跨某高度，也必须有明确已实现的跨步连接与实际路径，不能让ground路径在实体内部穿行；轮椅更不能继承行人假设。本轮没有授权增加通用跨越障碍能力。

**结论：删除或替换此全局高度豁免，不批准以0.25m阈值作为修复前提。** 允许的只有真实支持面/端面接触与已验证的ground↔stair↔landing衔接。原用户默认不穿墙/不穿结构、不自动开洞/消障碍的规则保持。

## 拟议“有向切片”只是候选方法，尚未验证

把正方形人体体积沿行进方向缩成half=.05m的薄片，可能解除终点前伸过长造成的误拒；但不能仅在原来最多.5m间隔的采样点各放一片而认为路径已被连续检查。这样会留下未覆盖间隙，尤其当前结构部件不像obstacle那样有额外段级检测。

必须检验覆盖所有相邻采样区间/拐点的连续通行体积，明确朝向、支持面和终端接触。可以使用保守连续扫掠、经证明无间隙的区间体积或等价实现；本审查不强制某个SLICE_HALF参数，不把“设计已定”当证明。

验收要成对：合法楼梯脚接驳通过；实体位于两个采样点之间、横向薄墙/支柱、转角、反向时仍拒绝。不能通过缩人体、删墙/步、移动反例障碍或重新引入整组豁免回绿。

## 其余未完成与实现风险

1. label前驱仍按node保存，需要绑定非支配状态的唯一身份，否则搜索cost/climb与重建路径可能不同。edge真实climb/descent是正确方向，不等于完整搜索已修。
2. 验证器应从原始输入与保存工程栅格/真实结构重新计算每能力坡度、水域、体积/支持关系，而不是相信保存facts。合法ground/结构接驳要共用明确合同。
3. validate_port_membership目前使用旋转部件AABB包含与max(xExtent,zExtent)作为宽度；这不是任意朝向实际轮廓/横向可用宽度。声明levelId还必须对应命中的部件层。续作要加入旋转、面角外点、错误层、窄长面反例，不声称新归属规则已经完全正确。
4. 新6类反例尚未进回归；原17项有效断言保留/替代表缺失。不要先追求5/5再忘掉其余合同，测试资源应自包含且可入库。
5. G19R1发布的commit/时钟门未在本轮修改；作为原任务未完项一并验证，不因partial汇报略去而视为完成。

## 预算与接续

原G19R2授权cargo≤30、CLI≤40；自报已用cargo12/CLI0，**不支持“执行次数预算用完”**。模型上下文或会话内部资源限制与工程调用次数预算是不同概念；上下文限制本身本次不可核实。诚实停下交partial可以，但应分别记录停止原因。

续作按已报12作为起点，追加cargo最多18、CLI最多40；若发现漏计旧执行，相应扣减，不能通过新目录重置累计上限。检查员本次2cargo/3CLI是独立审查成本，单列不计worker额度；自然/ST18/默认world操作全0。

不补造失败日志，不要求新确认稿；接手者先读此报告和完整续作任务，用完整小场景完成剩余实现/反例/可视几何/审核一次交付。A不变、B partial、C不推进。