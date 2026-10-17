# G35：范围、基线与验收闸门（不授权实现）

日期：2026-09-28。本任务只做G35的目标确认、冻结边界、视觉意图、验收设计和预算闸门，**不授权修改产品、不运行cargo、不运行产品CLI、不启动验收Chrome**。没有明确用户目标和独立范围批准前，不进入G35实现。

## 0. 严格执行条款

**请严格按任务书完成任务，不得因为其他任务无关问题擅自中断或停止部分步骤；不得因为上下文压缩、会话切换、摘要丢失或可修复的脚本问题擅自删减步骤。**
如遇上下文不足，保存checkpoint并恢复继续；checkpoint必须记录G34-R5最终包hash、G35已完成/未完成项、发现的范围冲突和下一条命令。此闸门没有产品实现授权，不得借“顺手修复”改源码或重跑产品链。

## 1. 已接受的基线

唯一候选基线：G34-R5交付包
`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g34-r5-final-browser/cli-runs/r5-c02-export`

绑定基线：
- production ID：`g34-terrain-visibility-6`
- viewer：以R5 receipt和包内/current source hash为准
- model：`922d404cdbd413e567d5ab89094d69ef75f785ab73e469c5676a79dc025f23ff`
- G34闭集：1178项，R5 verifier `ok=true`、`problems=[]`

G34-R5的限定不能被抹掉：Chrome退出码未观察、16个纯读取场景trail为空、R3/R4历史无逐文件manifest、早期台账继承漂移。G35不得把这些历史例外写成R5产品失败，也不得悄悄删除。

## 2. 任务内容

### T0：明确用户目标

在任何实现讨论前写出一个且仅一个G35目标：
- 用户问题/收益；
- 不做什么；
- 与G34已有语义的关系；
- 可见变化与不得变化；
- 如果没有明确目标，状态为`scope_blocked`，不猜测、不实现。

### T1：冻结边界

绑定G34-R5包六文件、源码viewer/Rust、model、shader/材质和G34已接受功能。建立允许修改文件白名单和受保护函数清单；默认禁止触碰：G34 visibility snapshot、clip语义、G32/G33导航与footprint、G30读图、主图拾取/相机、几何和model。

### T2：视觉意图与反例

在任何参数或源码改变前写`visual-intent.zh-CN.md`，包括：基线截图/状态、候选变化、预期收益、可接受妥协、明确退化反例、两尺寸场景、状态/键盘/鼠标可达性。

### T3：验收合同和资源

写唯一required scene表、独立expected/oracle、失败判定、notObserved规则、坏包/恢复策略和截图闭集策略。资源只写计划，不消费本轮产品预算；建议先给cargo/CLI/Chrome/driver/archive五类软计划与硬上限，但不要用“小额度”逼迫删除必测项。

### T4：独立裁定材料

写scope、baseline binding、visual intent、stop rules、budget proposal和delivery checklist。所有材料只落在新的G35 scope目录；不得改G34-R5或任何历史目录。目录树检查通过后交`scope_ready_for_implementation`或`scope_blocked`，不能写`delivered_for_inspection`。

## 3. 上一阶段未做好的事项（只需登记，不得借机修改历史）

G35范围文件必须附上这份清单并说明“历史例外，不回写”：
1. R5 Chrome退出码没有观察；
2. R5 16个纯读取场景trail为空，但有边界记录；
3. G34-R3/R4没有逐文件manifest；
4. G34早期台账存在继承漂移；
5. R5仍有2次失败driver尝试，成功矩阵与失败尝试要分开；
6. G34-R5已完成闭集，不再开纯文档收口轮次。

## 4. 完成条件

完成条件不是“写得很长”，而是：目标单一、边界明确、反例和验收可执行、预算合理、历史例外不被改写、没有产品进程痕迹。完成后等待独立检查和用户确认；**G35实现仍未授权**。

**请严格按任务书完成任务，不得因为其他任务无关问题擅自中断或停止部分步骤；如需暂停，先保存checkpoint并在恢复后继续。**