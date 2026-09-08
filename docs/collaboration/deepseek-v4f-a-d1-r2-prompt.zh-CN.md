# 可直接转交外部DeepSeek v4f：A-D1-R2

你是GlyphWeave协助实现者。夜间由用户手动将此任务交给你；不要启动任何其他模型。总控只验收。请先阅读本任务，再按白名单执行一次有界修订。

## 工作区与基线

只操作 G:/difyllmwiki/GlyphWeave，绝不修改 F:/modenmapbox。
HEAD必须仍为6020fbe2713e8c79c1b2934b244402dc93e1df14；如变化先报告差异，不改Git恢复。
本次是A-D1-R1的定点修订，不是重新实现或提前开展A-D2。

只允许修改：
- G:/difyllmwiki/GlyphWeave/scripts/collaboration/check-evidence.mjs
- G:/difyllmwiki/GlyphWeave/scripts/collaboration/check-evidence.test.mjs
- G:/difyllmwiki/GlyphWeave/docs/collaboration/a-d1-result.zh-CN.md
- G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/deepseek/ 下新建R2证据（不覆盖旧证据）

先只读：
- G:/difyllmwiki/GlyphWeave/AGENTS.md
- G:/difyllmwiki/GlyphWeave/docs/collaboration/acceptance-contract.zh-CN.md
- G:/difyllmwiki/GlyphWeave/docs/collaboration/a-d1-review-02.zh-CN.md（本轮详细验收标准）
- G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/review/a-d1-r1-independent.mjs
- G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/review/r1-independent-q9Rao5/summary.json
- G:/difyllmwiki/GlyphWeave/bevy/work/r49-maximum-generation-measurement/process.json
- G:/difyllmwiki/GlyphWeave/bevy/work/r49-maximum-gate-measurement/process.json

## 事实及必须完成的5项

R1原42测试通过，但总控在候选能pass的基础fixture上逐项变异，9个负例全部pass/0。不要用旧11反例基础fixture已incomplete来证明语义正确。

F1：严格区分generate-world和完整world-quality-gate；解析hash绑定的过程JSON并核对operation、runtime身份、manifest身份、output、状态/退出码/stopReason、scenes。使用已有process-benchmark v1语义。任意文本log的hash不足以证明绑定；log路线缺结构化运行声明则incomplete。声明来源真实性仍由总控在工具外确认，不实现签名或编译证明。
F2：已完整完成的质量失败与timeout/budget-stopped/crash/abort/running分离；后者不能被allowedGateFailures抵消。外层/过程文件矛盾拒绝。
F3：manifest受限解析与真实schema检查（当前真实输入glyphweave-world/version=1）；核对scene身份/数量/尺寸与两份过程证据。输入与发布manifest可能规范化不同，不要求二者同hash。
F4：receipt.world.path必须与被检查world是同一真实目录，不能以“其子目录在内”替代身份相等。
F5：gate.failures对pass和fail都必须先验证字符串数组，缺失/null/object/scalar/混合类型不得默认为空数组。

详细字段、限制及当前代码位置以review-02为准。不要绕过未知/缺失字段，不弱化既有authority/版本/hash/frontage/只读约束。不为旧demo改authority标签伪造授权。

## 验收与资源

先给每项写红测，再定点修复；提供完整真实schema形状的合成正向和正常失败获授权正向。明确合成材料不是实际生成证据。逐项反例只改变一个条件，保持其他证据完整。
对过程JSON的runtime hash/manifest hash/output/operation/exitCode分别设置独立负测，不能只靠一个同时错5字段的用例。
至少覆盖：两种错误operation；过程内容矛盾；timeout与budget-stopped；manifest非JSON/未知schema/scene尺寸冲突；world子目录；pass时failures缺失和非法类型。保留合法路径等价及质量fail不翻转的正向。

执行：
1. node --test scripts/collaboration/check-evidence.test.mjs
2. node --check scripts/collaboration/check-evidence.mjs
3. node --check scripts/collaboration/check-evidence.test.mjs
4. node scripts/generate-tree.mjs --check（等价doc-tree检查）
5. git diff --check，另查未跟踪白名单文件尾随空格。

总控review目录及脚本只读。旧独立control可能因新合同不完整，必须在你的测试中补齐正向，不能把全拒绝当完成。真实r49/r48只读紧凑检查；缺受信receipt维持integration_pending，不需大图证明。禁止安装新依赖、跑完整Rust/最大图、启动后台server或额外模型。

本次只做一次有界R2修订；同类问题若仍失败，交最小复现、剩余风险和阻塞，不无限重写重跑。

## 绝对不提交

禁止git add/commit/push/merge/rebase/reset/clean/checkout/switch/stash，禁止修改分支/标签/Git配置/钩子/暂存区。只读Git可以。不得清理、格式化或覆盖他人Rust/docs改动；HEAD保持不动。

交付taskId=A-D1-R2、ready_for_review或具体阻塞、文件及hash、逐项修复说明、正负测试结果、命令/退出码/日志路径、真实检查状态、未验证项、费用未知则unknown、无后台进程，并明确“未暂存、未提交、未推送”。交付后等待总控，不自行开展A-D2。
