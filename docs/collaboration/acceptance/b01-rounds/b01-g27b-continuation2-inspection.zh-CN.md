# B01-G27B continuation-2 独立检查（2026-09-23）

## 1. 裁定

**G27B保持partial，授权有效、不回hold。接受trace的已实证局部修复，不接受“§3.2完整闭合”的扩大结论。** P1独立来源重建仍保留；本轮缺陷是可选报告合同不完整，不是来源信任绕过复发。

continuation-2只是一个中间工作检查点，不能作为完整收尾任务完成。worker本轮只耗cargo1，剩cargo11/CLI23/browser7/aux8。所称“可用执行容量耗尽”没有可供本检查核对的容量计量；即使确为模型单轮容量限制，也不等于已授权预算耗尽或产品hold。允许诚实记录中断和保留可恢复状态，但下轮直接续做原任务，不再审批、不重置预算。

## 2. 文件与实际运行核验

- worker源码context_preview3d.rs为70032B，SHA-256 cee80cf60fcacd38c9865cb3c66d3a04ed5ca62bba71b728b02721940ff9c1fa，与交付一致。
- continuation-2清单5/5，无未列文件；receipt的manifest/report/progress/commandLog/sourceBinding五项匹配。
- 15组前序清单（含14组worker清单及上一轮检查员清单）已列文件无漂移；G25的context_preview.rs与上次检查时相同。
- 本轮检查员实际重跑CLI单测：60 passed / 0 failed；随后构建当前源码成功。未重跑core，不把上一轮82项保护回归记为本轮实测。
- 检查员新构建已更新target二进制；worker“交付时target未含本轮修改”在其停点成立，但现在不能再称target仍旧。DSH后续修改后必须重新构建自己的最终版本。

## 3. 八次生产CLI观察

固定输入仍为显式world、G24 continuation-4 clean context、G25 p16 preview；trace反例全部从真实G25审核报告复制，只改一个目标字段，真实来源不变。

| 编号 | 条件 | 实测 | 结论 |
| --- | --- | --- | --- |
| 01 | 合法来源，不提供trace，export | exit0 | 可选trace没有变成来源信任根 |
| 02 | 审核01的新包 | exit0，四段pass | 正向生产链仍通过；不表示包合同负例已闭合 |
| 03 | 合法来源+真实G25报告 | exit0 | 真实报告正对照 |
| 04 | 恰四条stages，其中同名sourceBinding重复 | exit1，点名出现2次 | 唯一性修复成立，不只是因长度为5被拒 |
| 05 | 真实报告仅totalPass=false | exit1 | 矛盾总判定被拒 |
| 06 | notEvaluatedStages改为字符串terrainParity | exit0 | 错误类型被当作未提供，漏检 |
| 07 | schemaVersion改context-preview-audit-v999 | exit0 | 报告未知版本未校验 |
| 08 | evaluatedStages改为空数组，仍有四pass | exit0 | 分阶段摘要矛盾未校验 |

以上06—08只证明报告留痕误接受，不证明可放行不匹配的world/context/v1；不能推翻前轮已实测的P1修复。

## 4. 源码定位与口径更正

文件：G:/difyllmwiki/GlyphWeave/bevy/crates/cli/src/context_preview3d.rs（本轮绑定版本）。

1. 第127行仍直接fs::read(path)，没有该trace分支的普通文件/读取字节上限检查或复用受限读取helper。“受限读取”未实现，不能按报告接受。此项为静态核验，未创建超大文件或动态symlink实验。
2. 第182行以if let Some(...as_array())处理notEvaluatedStages；错误类型悄然跳过。应明确missing/type/value合同，不得将解析不匹配等同可选缺省。
3. 未校验真实报告schemaVersion=context-preview-audit-v1及evaluatedStages与四段状态一致性。新增正例自己构造了缺schema/evaluatedStages的简化报告，不能据此声称真实生产报告合同完整。
4. 第138行的modelSha256对比的是read_v1_package读出的传入v1模型字节hash，不是序列化fresh重建值的hash。准确口径应为“先对来源重建做语义比对，再绑定被读取v1模型的实际字节”。保留这种分工，不为修文案误伤JSON重编码或f32/f64差异。
5. 仍可见viewer资源只查非空、HTML内嵌模型仅比五组字段、access/roads/nodes固定Y、实体gl.LINES、最近样本Math.round地形拾取。worker对这些未完成的披露真实；本轮不重复跑其旧反例，也不声称已闭合。

## 5. 推进决定

原完整任务书保持不变：G:/difyllmwiki/GlyphWeave/docs/collaboration/dsh-b01-g27b-complete-chain-task.zh-CN.md。

新增执行令：G:/difyllmwiki/GlyphWeave/docs/collaboration/dsh-b01-g27b-remaining-execution-order.zh-CN.md。它不是新产品任务或新预算包，只明确继续顺序、trace漏项与容量中断的恢复规则。不可再次只修trace后交一个“收口包”；下一实际实施重点是完整资产/模型合同和真正有向几何/三角拾取，随后最终生产CLI与WebGL观察。

已封存continuation-2不回改。后续新增运行可放continuation-3，目录编号仅为证据隔离，不代表预算重启。G27B累计消耗cargo9/CLI9/browser0/aux0，累计上限20/32/7/8，剩余11/23/7/8。本次不追加额度。

检查员本轮独立成本：cargo2（测试+构建）、CLI8、矩阵驱动1、浏览器0；hash/只读核验另列。未修改产品源码、固定来源或worker旧证据；未启动3D浏览器验收、工程build、ST18、默认世界或路线系统。

**A冻结、B partial、C不推进。**
