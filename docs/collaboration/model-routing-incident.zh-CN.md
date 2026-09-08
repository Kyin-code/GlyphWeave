# 模型调度偏差：用户叫停后的核对记录

日期2026-09-08（Asia/Shanghai）。本次仅更正协调/验收归属记录，不修改业务代码。

用户发现实际调用均为Astra后，主Agent立即关闭Maxwell和Euler两个agent。停止后未发现cargo/rustc或本项目Node测试进程；历史preview及应用自身Node进程未动。get_goal返回null，本轮不继续开发或派发。

## 已证实的本地记录

- Maxwell，01a07f1d-9bcc-77f0-a186-dc426fae2ff1：11:44:02、12:04:22的turn_context.model=gpt-5.6-terra；12:23:15、12:36:17、13:44:14、14:05:56、14:35:15记录为gpt-6-astra。
- Euler，01a07f1b-9136-7173-a091-bb1f4d0d8a6b：11:41:48、12:05:26记录为gpt-5.6-luna；12:23:43、13:45:03、14:20:35记录为gpt-6-astra。
- 本地默认model=gpt-6-astra，provider=custom。catalog中确实列有terra/luna/astra三个slug，但catalog存在不证明每次实际路由正确。

精简证据G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/review/model-routing-incident.json，含原会话文件绝对路径与UTC时间。这里证明的是客户端记录的model字段，不是供应商账单或上游最终服务模型的独立认证。

## 归属纠正与责任

首次spawn显式传了Terra/Luna；变化发生在主Agent关闭并resume旧agent之后的首个turn。resume工具没有model参数。时间对应与默认模型设置支持“恢复时继承/回落默认模型”的解释，但尚无底层路由实现证据，不断言平台或代理的具体bug。

主Agent没有在resume后复核实际turn_context，继续按旧角色名宣称Terra/Luna执行，这是调度核验错误。12:23后的A-T1后续、A-T2/A-T3及A-D2记录不能继续当成指定Terra/Luna模型的实测表现或费用数据。历史文档中的角色名称需按本纠正记录解读；独立测试输出保留，不因为模型归属错误而伪造或删除代码证据。

## 停止与恢复条件

当前两个agent已关闭，不自动恢复；未改Codex全局配置/模型目录，也未暂存/提交/推送。
若用户之后同意恢复，必须每次新执行/恢复前后核对实际model；不能靠昵称或任务prompt指定模型。优先为每项任务新建显式model的worker，再读取首个turn_context核实，错误立即关闭。对于没有model参数的resume路径，在明确验证其保留模型前不再用来延续指定模型任务。上游真实计费/路由需用户供应商记录进一步核对，当前无依据计算损失或退款。
