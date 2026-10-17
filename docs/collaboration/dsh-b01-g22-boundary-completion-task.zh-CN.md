# DSH：G22同包局部收尾——包装入口的无效数据与空路径

用户转发本任务后按原G22范围和余额执行，不新开包、不增加额度；无需确认稿。

## 已接受结果

检查员已独立复现29/29核心测试和17项最终CLI；原ground窄承托及远处构件差分均正确拒绝，裸地与合法楼梯通过，指定身份/覆盖/几何篡改审核拒绝。保留双轴状态、空间筛选、扫描线和楼梯，不重做主体算法。

先读G:/difyllmwiki/GlyphWeave/docs/collaboration/b01-g22-inspection.zh-CN.md；反例代码/结果在G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g22-20260920/state-probe.rs及state-probe.log。当前core469f181e…/support79cf0bf6…。

## 两个必须收尾的原入口契约

1. `top=NaN`的frame被包装层按层过滤后返回NoStructuralSupport+NotApplicable，ground_refusal=None。无效字段不能作为“明确位于其它层”的筛选证据。先校验筛选所依赖数据，或传播Unknown；校验不能只存在于已经被过滤后的集合。
2. 空路径/单点/全退化路径不能以空interval通过ground_refusal/structure_refusal。生成明确无效路径状态，消费者检查其全局失败，不因for循环无元素而默认无拒绝。有效路径允许的重复点可按明确策略规范化，不误拒既有合法楼梯。

增加具名生产API回归，分别测试无效top、非有限/空/全退化路径、正常裸地与合法结构、预算失败。坏值应拒绝/unknown；有效无结构路径仍交权威地形门。不要为把helper案例放进CLI而放宽JSON或新增生产参数。

保持N1固定完整CLI输入和已有拒绝预期不变；这些是API边界缺口，不能反过来宣称原完整CLI修复未发生。

## 最终验证与预算

按自报余cargo4、CLI7，失败命令累计，漏计扣减。先计划最终回归和CLI构建。建议7CLI优先用于N1控制/远处各拒绝、裸地build+verify、最小折返build+verify、一个身份自洽篡改指定重查；最小直跑/其余语义保留核心矩阵，明确哪些最终版实际观察、哪些检查员前版观察，不声称整矩阵全重跑。

最后产物按版本明确绑定，新文件不得覆盖旧日志/回执/清单。测试导出需独立本轮新路径、create_new，普通测试不写交付目录。

无新增设施、无自然/ST18/default/bake/realize、无Git写操作、不改access/auto_roads及安全阈值。预算或必要门不足则partial停止；完成交delivered_for_inspection，不自动整体里程碑验收。

A冻结、B partial、C不推进。
