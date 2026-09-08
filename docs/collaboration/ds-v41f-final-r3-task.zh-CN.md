# 可直接转交 DS v4.1f：A-FINAL-R3 测量闭包小修

日期：2026-09-09（Asia/Shanghai）。你是执行者；主 Agent 是独立检查员。交付 `ready_for_review`，不得自行签署里程碑 A。

## 0. 目标和必读

R2 的真实测量、新 240 块观察和实例修复已独立复核通过。本任务只修复最终 checker 对测量摘要/process/samples 的交叉一致性漏检，并纠正文档披露。**不重生成、不重 gate、不重浏览器巡检。**

先读：
- G:/difyllmwiki/GlyphWeave/AGENTS.md
- G:/difyllmwiki/GlyphWeave/docs/collaboration/a-final-inspection-r2.zh-CN.md
- G:/difyllmwiki/GlyphWeave/docs/collaboration/acceptance-contract.zh-CN.md
- G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-r2-20260909/measurement-checker-counterexamples.json

仓库仅 G:/difyllmwiki/GlyphWeave；不改 F:/modenmapbox。HEAD 应仍为 6020fbe2713e8c79c1b2934b244402dc93e1df14；若不同，报告，不自行 checkout。禁止 git add/commit/push/reset/clean/checkout/switch/stash/merge/rebase、修改索引/配置/历史，禁止启动其他 agent 或模型。

## 1. 白名单及冻结

只允许修改：
1. G:/difyllmwiki/GlyphWeave/scripts/collaboration/check-final-acceptance.mjs
2. G:/difyllmwiki/GlyphWeave/scripts/collaboration/check-final-acceptance.test.mjs

只允许新增自己的结果及证据：
3. G:/difyllmwiki/GlyphWeave/docs/collaboration/a-final-signoff-r3.zh-CN.md（标题/结论明确“待主 Agent 复验”，不是自行签收）
4. G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-final-r3/ 下的新文件。

不改 R2/R1/检查员旧文件，不改协调 status 和本任务文档，不改 Rust、HTML adapter、其他 checker、依赖、lock、资产或生成配置。不删除他人 dirty 文件。不修顺手发现的业务问题。

R2 候选冻结：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-final-r2/final-acceptance-r2.json；16760 bytes；SHA256 f681f518b96a797be713e2b088780db673c248b817988e44578572390a9e415f。

其原始 runtime、manifest、world、parity、measurement process/samples、browser observation 和所有旧回执均只读。新候选可引用这些原件，不能复制修改原始记录来迎合新校验。

## 2. R3-M1：一致性合同

在现有 `checkMeasurement` 中小范围完成；可传入必要的外层候选上下文，不建立新框架。

### 2.1 固定角色与同一运行对象

- generation 必须是 generate-world；gate 必须是 world-quality-gate。不能只检查两份材料彼此写了同一个字符串。
- measurement/process 所指 executable 与候选 generation runtime 的实际路径/hash 一致（本轮 gate 同样使用原生成 exe，不是 audit exe）；manifest 与 candidate.manifest 一致。
- measurement/process output 为 candidate.world 文件所属世界根目录，按 realpath 核对。不让另一世界自己的自洽测量代替本候选测量。
- process.samplesJsonl 的实际路径及 samplesJsonlSha256 必须绑定这次读取的原始 samples 文件。
- process format/version、PID、终态/exitCode/stopReason 做显式检查；拒 running、timeout、interrupted、不认识的终态或不相容退出码。
- 正常 generation baked/exit0 可通过；当前 gate failed/exit1 且原已绑定质量报告为已批准内容 fail 可通过测量检查，worldQualityStatus 保持 fail。不要把所有 exit1 当作合格 gate，也不要错误拒绝真实 R2 的质量 fail。

### 2.2 摘要与 process 一致

候选与 process 的 elapsedSeconds、sampledPeakPrivateBytes、peakWorkingSetBytes、sampleCount、timeoutSeconds、maxPrivateMiB、exitCode 等已记录的同名字段必须一致。数值应有限、范围合法；计数/PID/字节值用安全整数约束。不要给原数据添加不存在的测量字段。

实际 elapsed/private 峰值仍受 600 秒 / 8192 MiB 限制，process 和摘要都不能改预算。数值比较按序列化数字一致；如确有计时精度容差需要，明确适用字段及上限，不能让摘要 1s/1byte 通过。

### 2.3 原始样本重算

- 从已通过 bytes/hash 校验的样本内容解析；避免校验一个文件快照却另读另一份内容进行计算。
- 原始行数 = process.sampleCount = measurement.sampleCount，且 >0。
- 每行 PID 都等于 process.pid，不能只是 >0。
- elapsed、privateBytes、workingSetBytes、cpuSeconds 有效且非负；UTC 可解析，时间顺序一致；elapsed 在进程总时长内，elapsed/CPU 不倒退。
- 从行数据重算 max(privateBytes)，等于 process 和摘要的 sampledPeakPrivateBytes。
- OS peakWorkingSetBytes 与摘要一致，且不小于样本 workingSetBytes 最大值。**不得要求 OS 峰值恰好等于离散采样峰值。**
- timestampUtc 在本版报告是结束记录时间，不要当作启动时刻。只做已有字段能支持的检查，不为补造 start 字段而重采。
- JSONL 损坏、数值非法、少/多样本、错 PID、次序混乱等必须给明确检查 ID 和非 pass 结果；CLI 为非零。不能仅发生未解释崩溃。

## 3. 必须通过的验收样本

用微型 fixture 做主要回归，少量真实 R2 候选副本验证；禁止每个反例重复做全世界/浏览器长任务。

正向：
- 未改 R2 真实候选经修复后 checker 仍 pass / ready_for_review，generation/gate measured。
- 小 fixture 的 generation exit0 + 正常 gate pass。
- 小 fixture 的正常内容 gate fail/exit1：测量允许，质量状态仍 fail。
- OS working-set 峰值大于样本最大值，且其他字段正确：允许。

负向至少覆盖：
1. 只把候选 generation.elapsedSeconds 改为 1。
2. 只把候选 generation.sampledPeakPrivateBytes 改为 1。
3. 样本一行 PID 改动并刷新外层 bytes/hash：拒绝。
4. 同时刷新样本与 process 的文件 hash 后，样本 PID 仍不同于 process.pid：仍拒绝（不能只碰巧由 hash 不符挡住）。
5. process/摘要 sampleCount、elapsed、private 或 working-set 峰值分别不一致。
6. 保持摘要/process 峰值相同但与原始样本最大 private 不符。
7. process.samplesJsonl 路径/hash 指向别的文件。
8. generation/gate 角色对换，或使用与候选不同的 runtime/manifest/output（对应小 fixture 文件真实存在并自洽）。
9. running/timeout/未知终态/不允许退出码/非 null stopReason。
10. 负值/空样本/无效 UTC/时间越界或倒序/CPU 倒退/预算上调。

每个反例保留变更字段、期望错误 ID、实际错误 ID/退出码。不得修改检查员反例原件。缺证据仍 incomplete；不要靠改状态枚举掩盖错误。

## 4. 文档纠正与证据复用

在新 R3 文档中追加说明，旧文档不改：
- 补采后的最终目录各 1781 文件，1779 相同，差异是 generation-report.json.output 以及 preview/app.js 观察适配器；说明 R2 “仅一项差异”的比较时点。权威世界语义相同，不是整个目录逐字节相同。
- 如实登记第一次失败测量及第二次纠正测量；第一次生成 266.9902462s + gate 38.1974344s。承认超出一次测量/遇差异停止的授权，不删除旧失败证据、不伪称只跑一次。
- 保留六类原 gate 内容失败、两项许可未解析、A-05 字节等价复用界限。
- 新 source snapshot/receipt 反映本轮 checker/test 源码，不能把更新后的 JS 快照说成原 Rust exe 的重新编译来源。原 runtime 来源链与本轮验证脚本增量分别记录。

## 5. 最小执行与交付

所有命令从 G:/difyllmwiki/GlyphWeave 执行，输出到全新文件名：

1. 定向 `node --test scripts/collaboration/check-final-acceptance.test.mjs`。
2. `node --test scripts/collaboration/*.test.mjs`（一次最终回归）。
3. 两个 mjs 的 `node --check`。
4. 用新 checker 检查未改 R2 候选，保存新结果到 ds-final-r3；再产出 R3 候选及其闭包检查。新候选仍引用 R2 原始世界/采样/观察。
5. `corepack pnpm doc-tree:check`；如不可用可 `node scripts/generate-tree.mjs --check`，记录等价替代。
6. `git diff --check`；只报告他人既有问题，不动白名单外代码。

交付：新结果文档、receipt-r3.json、新候选/检查结果、源码增量 hash 清单、定向/全套测试日志和正负例记录。回执逐项写命令、退出码、实际证据路径、未验证项、后台句柄；最终状态仅 ready_for_review。

**资源禁止项：不运行 generate-world、world-quality-gate、cargo test/build、release 构建、浏览器巡检或 linear 大测；不启动预览服务；不调用其他模型。** 此任务不需要这些操作。任何额外需求先停止并报告原因，不能自行突破预算。既有 8099 预览不动。

完成上述单一缺口后停止，等检查员复验；不自行推进 B/C 或追加通用加固。
