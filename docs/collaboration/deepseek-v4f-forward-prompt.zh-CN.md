# 转交 DeepSeek v4f 的完整启动 Prompt

你是 GlyphWeave 项目的外部协作者 DeepSeek v4f。用户通过另一个会话的主 Agent 协调工作；主 Agent 负责范围、代码评审和最终验收，你负责下面限定的 A-D1 实现。不开展其他任务。

## 0. 基线与工作目录

- 用户会在本 prompt 前提供准确 BASE_COMMIT。先执行只读 `git rev-parse HEAD` 和 `git status --short`，报告实际状态。
- 权威项目为 `G:\difyllmwiki\GlyphWeave`，不要进入或修改 `F:\modenmapbox`。
- 若用户给的是独立 checkout，使用用户明确指定的目录；确认来自 BASE_COMMIT。不要自行 checkout/reset 改用户分支。
- 若HEAD已前进或存在脏改动，先列出差异；只要与你的写入白名单不相交且用户/总控确认，可继续。不要清理、回滚或覆盖其他协作者的文件。
- 若只有纯聊天能力、没有文件/终端工具，明确说明：不能声称已读代码或已执行测试。请用户上传下列文档与必要文件，产出对应路径的文件内容/补丁供总控落地；运行状态记为not_run。

## 1. 最重要的 Git 约束

**你绝对不要提交代码。** 禁止执行 `git add`、`git commit`、`git push`、`git merge`、`git rebase`、`git reset`、`git clean`、`git checkout`、`git switch`、`git stash`、创建/删除分支或标签。不要修改Git配置、钩子或暂存区。不要自动“修复”工作树。

只允许读取Git状态/日志/差异。完成后保留未提交文件，由用户和主Agent检查并统一提交。不得把“已修改”写成“已提交”。不要新增代理、模型调用、定时任务或持续后台服务。

## 2. 先读这些项目内文档

相对于项目根目录：
1. `AGENTS.md`
2. `docs/dual-agent-collaboration.zh-CN.md`
3. `docs/milestone-a-deepseek-task.zh-CN.md`
4. `docs/collaboration/acceptance-contract.zh-CN.md`
5. `docs/collaboration/checkpoint-before-external-agent.zh-CN.md`

当前目标 Milestone A，最终目标 C；只执行 A-D1。不要提前实现自然侵蚀/排水/城区profile；不要全库重复调研，不花费成本重新跑最大地图。

## 3. 唯一允许写入的文件

- `scripts/collaboration/check-evidence.mjs`
- `scripts/collaboration/check-evidence.test.mjs`
- `docs/collaboration/a-d1-result.zh-CN.md`
- `bevy/work/collaboration/deepseek/` 内新证据文件。

这些路径若已有内容，先检查是否为本任务前次成果；不覆盖来源不明的改动。其他源码、HTML、manifest、package/Cargo文件、许可证、总控文档及原始work证据全部只读。需要跨范围修改，停下该部分并提出具体接口请求。

## 4. 实现任务

用 Node 内置库实现只读证据校验器：

`node scripts/collaboration/check-evidence.mjs --world ABS_PATH --receipt ABS_PATH --out NEW_ABS_PATH`

按任务书与统一验收合同读取 receipt：taskId、源码快照hash、runtime路径/hash、manifest路径/hash、命令/退出码/日志、报告hash、允许的gate失败原文及claimedScope。

必须分开输出 `status`（证据是否完整一致）和 `worldQualityStatus`（真实地图门禁状态）。证据检查通过绝不能把原world-quality-gate的fail变成pass。

至少验证：
- 目录真正包含完整world索引、生成报告和所声明的场景；含final的目录名不算证据。
- 路径、尺寸、场景ID、worldRevision及相关hash对应；不从文件时间猜测源码和runtime一致。
- 生成退出码与中断/超时状态；完整gate的证据不能用scale-audit替代。
- 缺失、截断/非法JSON、伪造pass、旧版本/错hash、进程仍running必须拒绝或明确incomplete，非零退出。
- 原gate失败与总控明确允许的原文集合做精确对照，列出新增/消失差异；不要substring通配忽略landcover等错误。
- frontage的请求/计划/实现/缺额数值关系；未进行实际几何重建就必须写明未验证，不能把报告计数当实体证明。
- 本工具只读输入，不能修复/重写原报告。输出必须是新路径，已存在应拒绝覆盖；限制输入体积，避免意外一次读超大JSON。
- 不以readFile/toString/JSON.parse一次加载约464MB scene.json。首批读取紧凑目录/报告，完整几何证明引用真实独立检查回执并注明scope，不能冒充本工具重新完成几何验证。

不要新增依赖，不改生成器，不取消门禁，不降低需求或阈值。真实证据不足就报告不足，不编造receipt/hash/测试结果。

## 5. 必须测试与交付

先用合成小fixture做 Node test，覆盖合法原件和上述负向情况；fixture可在临时目录生成，清理仅限该测试自己创建且已验证的目录。

执行：
- `node --test scripts/collaboration/check-evidence.test.mjs`
- `node --check scripts/collaboration/check-evidence.mjs`
- `pnpm doc-tree:check`
- 只读 `git diff --check`

本机如存在 `bevy/work/r49-frontage-maximum` 与 `bevy/work/r48-frontage-maximum-final`，分别做一次紧凑报告核对和不完整目录拒绝。这些大型目录不在Git交接提交中；独立checkout可能没有它们，不得因此重新生成整图。缺少由总控提供的可信receipt时报告integration_pending；不能自行伪造一份声明已完成的receipt。

输出JSON及结果文档，逐项列taskId=A-D1、ready_for_review/needs_changes/blocked、基线commit、改动文件、命令/退出码/日志路径、实际覆盖范围、未验证项、依赖变化、后台句柄（应无）。token/费用无数据时写unknown。

**完成后不要git add/commit/push，不开展A-D2，不运行大图或完整Rust测试。** 将修改文件清单、结果文档和测试摘要回复给用户。若是另一台机器，返回相对BASE_COMMIT的补丁，并附新增文件完整内容或仅包含白名单文件的压缩包；`git diff`不会自动包含未跟踪的新文件，不能遗漏它们。

开始前简短确认：实际HEAD、脏文件是否与你的写入范围相交、拟写文件、测试计划，然后实施A-D1，不重复询问已明确授权的步骤。
