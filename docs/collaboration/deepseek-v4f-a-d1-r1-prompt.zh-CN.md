# 夜间 v4f：A-D1-R1 修订 Prompt（可直接转交）

继续你已交付的A-D1，不开始A-D2、不重做项目调研。主Agent已复跑原41测试全部通过，但独立11类反例全部误返回pass/exit0，本轮结论needs_changes。今晚你是主力实现者，主Agent只做评审验收，Terra暂停新增工作。

项目 G:\difyllmwiki\GlyphWeave；基线6020fbe2713e8c79c1b2934b244402dc93e1df14。不要进入F:\modenmapbox。先只读git status/rev-parse，已有Rust脏文件属于待审查成果，来源未完全确认，不得改、回滚、格式化或归属给自己。

必须先读：
- docs/collaboration/a-d1-review-01.zh-CN.md（本轮精确发现和修复要求）
- docs/collaboration/acceptance-contract.zh-CN.md
- docs/milestone-a-deepseek-task.zh-CN.md
- 你自己的a-d1-result与两个脚本。

总控复现：运行 `node bevy/work/collaboration/review/a-d1-adversarial.mjs`（该脚本只读，不改它）。如当前本机没有review文件，请按review文档11种条件在自己的test中复现，不编造运行结果。

只允许改：
1. scripts/collaboration/check-evidence.mjs
2. scripts/collaboration/check-evidence.test.mjs
3. docs/collaboration/a-d1-result.zh-CN.md
4. bevy/work/collaboration/deepseek/ 中新的r1证据，保留旧输出。

【本轮修正】
R1：manifest、文件级sourceSnapshot、真正读取的必需报告hash、独立generation和world-quality-gate执行回执必须校验/绑定。runtime hash对不等于证明用过该runtime；缺可信运行/源码证明就incomplete，不猜、不改demo成正式授权。
R2：demo/worker-draft/none/unknown整体非零且incomplete；main-agent-acceptance字符串也不是认证，声明工具外的信任边界。
R3：schema版本/format/报告object/每份revision/场景非空与数量一致/生成gate状态严格；unknown不pass，pass+failures矛盾fail，不丢弃非法failure项。
R4：读取真实quality-report.scenes[i].frontageAudit.realizedWithAccess和planned，按sceneId/hash/revision绑定。requested=planned+unfilled，realized<=planned，规划缺额与实现缺额分别表达；几何未验证不能凭计数认定通过。
P2：拒绝scene目录逃逸；out不可进入只读world或别的输入，wx原子拒绝覆盖；有限正整数字节预算、目录/权限/读取错误结构化输出。

按review文档给的具体要求完成，不写一套与现有合同不兼容的新receipt后只改fixture通过。必要的schema扩展明确版本和旧格式incomplete策略，不能默默忽略统一合同的字段。

【验收】
- 先加入红测，然后修复；既有41项中标准有误的正向样例应补真实必需字段，不能弱化checker。
- node --test scripts/collaboration/check-evidence.test.mjs
- node --check 两个mjs
- 主Agent上述反例：11种不合法情况全部非零（但这是必要非充分条件，不可硬编码这些文件名）。
- pnpm doc-tree:check（PATH不支持可用已有corepack或Node等价命令并说明）
- git diff --check；额外自查新文件尾空格，git diff看不到未跟踪文件。
- r49/r48只读紧凑验证；没有可信receipt则integration_pending，不跑大图来补证据。

两次修复后同类问题仍失败，先报最小复现和阻塞，不无限重跑。用Node内置库，不安装依赖、不启动后台服务、不调用额外模型。

【绝对不提交】
禁止git add/commit/push/merge/rebase/reset/clean/checkout/switch/stash、分支/标签/配置/钩子/暂存区操作。只读Git可以。不要改其他协作者文件，不提前做A-D2。

完成返回taskId=A-D1-R1、ready_for_review、修改文件/hash、测试命令/退出码、11反例结果、真实验证状态、旧问题如何修复、未证实部分、无后台进程和“未暂存/未提交/未推送”。日志及新文件保持工作区待主Agent验收。
