# DeepSeek v4f 任务书：Milestone A 证据与全域预览（待真实接入）

日期：2026-09-08。**本任务只能交给用户提供的真实DeepSeek v4f会话/工具；当前未启动，禁止其他模型冒名替代。**

权威根目录 `G:\difyllmwiki\GlyphWeave`；禁止改 `F:\modenmapbox`。先读总控说明及统一验收合同；当前做A，不启动C侵蚀、水文、城市新内容实现。分工基于任务边界清晰程度，不作模型强弱宣称。

## A-D1：只读证据校验器（首批）

### 目标

建立可以拒绝“目录叫final但未发布”“旧runtime日志证明新源码”“只有报告pass但实体缺失”等误判的可执行工具；不是再写一份进度总结。

### 写入锁

仅新文件（相对于权威根目录）：
- `scripts/collaboration/check-evidence.mjs`
- `scripts/collaboration/check-evidence.test.mjs`
- `docs/collaboration/a-d1-result.zh-CN.md`
- `bevy/work/collaboration/deepseek/` 独立证据输出。

Rust、原scripts、HTML、manifest、Cargo/package、原始work报告全部只读。需要工具依赖优先Node内置fs/crypto/stream；不得临时安装外部库。其他文件改动先申请。

### CLI和结果接口

建议 `node scripts/collaboration/check-evidence.mjs --world ABS_PATH --receipt ABS_PATH --out NEW_ABS_PATH`。

receipt由总控填写，至少有taskId、sourceSnapshot（当前sha256及修改文件集）、runtime（绝对路径+sha256）、manifest（路径+sha256）、完整命令与退出码、报告绝对路径及hash、允许的gate失败原文集合、claimedScope。缺省/未知不推断pass，输出incomplete或fail并非零退出。

输出JSON：format/version/taskId/status/checks[{id,status,source,reason}]/missingEvidence/unexpectedFailures。status=pass只指证据完整且内部一致，不代表world-quality-gate通过。必须同时输出worldQualityStatus，避免将A的证据验收当C质量验收。

### 必须检查

1. world.json/generation-report/完整场景索引存在，尺寸和manifest一致；r48未发布目录必失败。不要只看文件名含final。
2. 输入、runtime、报告、声明源码快照身份一致；缺失证据不能由当前文件时间猜测。
3. 生成失败/预算中止不能报完成；完整gate执行证据独立存在，不能用scale-audit替代。
4. worldRevision/sceneId在相关报告一致；宣称144 frontage时至少核对frontageAudit的请求/规划/实现/缺额，并将未能独立核对实体标记为未验证，不把计数当几何真值。
5. 实际gate失败逐条核对授权基线；新增/消失均列差异。不得按substring宽泛忽略所有landcover问题。允许已知内容缺口不代表忽略新的语义变化。
6. 缺少、截断、无效JSON、hash不符、伪造pass、旧版本报告、进程仍running都有负向测试。
7. 不能单字符串读464MB scene。首批只处理紧凑目录/报告；完整实体几何核对使用已独立验证的Rust/现有工具回执并标注scope，不假装自己重做全域验证。
8. 不修改输入目录、门禁或证据；输出使用新路径，已存在就拒绝覆盖。

### 首批验收

Node内置test；合成小fixture足够验证上述分支。一次真实r49紧凑报告核对和一次r48不完整目录拒绝（只读）。不跑最大生成、线性性能基准或完整Rust测试，不启动服务器。报告每个check证明什么/不证明什么。

## 后续队列（需总控另行派发）

A-D2 全域HTML：复用现有Playwright/浏览器，不新增依赖；固定同一页遍历240块，再返回起点并至少三轮复访关键区，检查区块/实体/高度来源、负origin、首尾部分块、网络失败恢复、无整份scene请求、资源释放和驻留上界。先探测可用的内存/缓存计数，不用不支持的browser heap数值伪造证明；需要HTML新增观察接口时仅提接口给总控，不抢写。

A-D3 大文件与来源清单：审计所有全量JSON读取入口，使用流/紧凑权威索引给出有证据的修复建议；盘点已有LICENSE和THIRD_PARTY_NOTICES，不重复声称它们不存在。记录crate/npm、vendor、模型/纹理、GIS和参考算法的版本/许可/来源状态。许可证兼容性未核实标待核验，不擅自复制第三方代码。

模型外部会话交付回总控：patch（或明确工作区路径）、改动清单、命令/退出码、JSON结果、日志、SHA256、未完成项、实际可得费用/耗时；没有token账单填unknown。主Agent独立复验后才接受。

## 可直接粘贴给DeepSeek的启动指令

“请在 G:\difyllmwiki\GlyphWeave 阅读 docs/dual-agent-collaboration.zh-CN.md、docs/collaboration/acceptance-contract.zh-CN.md 和 docs/milestone-a-deepseek-task.zh-CN.md。仅执行已授权 A-D1，在规定的新文件路径实现只读证据校验器及测试，不修改Rust、HTML或原始证据，不修改F:\modenmapbox，不跑大图、不增加依赖、不提交commit。先给拟改文件与验证计划，再执行并按任务书返回证据；后续任务和C阶段等待总控派发。”

## 用户确认：外部转交与禁止提交（2026-09-08）

DeepSeek v4f 不接入本会话，由用户在外部客户端转交prompt。完整启动指令见 `docs/collaboration/deepseek-v4f-forward-prompt.zh-CN.md`。总控先建立代码交接checkpoint并提供commit ID。

**v4f禁止git add/commit/push/merge/rebase/reset/clean/checkout/switch/stash、操作分支标签或修改Git配置/钩子/暂存区。** 只在白名单中编辑并留下未提交改动；主Agent验收后统一提交。任务书早前任何含糊的交付说明均以此明确约束为准。另机器的新增文件必须随补丁一并返回，不可只给不含untracked文件的git diff。
