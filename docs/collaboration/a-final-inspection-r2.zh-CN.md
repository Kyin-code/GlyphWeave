# A-FINAL-R2 独立检查：实测及全域观察通过，测量闭包需小修

检查日期：2026-09-09（Asia/Shanghai）。检查员：主 Agent；执行者：外部 DS v4.1f。

## 1. 裁定与范围

本轮结论：`needs-changes`，仅退回测量验收脚本的一致性检查；**不否定已取得的真实 R2 测量和浏览器观察，不要求重采**。当前尚不签署 `milestone-A-accepted`。

- 阶段 S 的实例存在性修复及 parity/world 交叉绑定：本轮复验通过。
- 阶段 M 的纠正后生成/gate 原始测量：检查员独立重算通过。
- 阶段 E 的权威世界内容等价：通过；最终目录的非权威适配器差异应准确披露。
- 阶段 B 的 240 块新观察：原始记录重放通过，不是检查员重新执行了 23 分钟浏览器巡检。
- 当前唯一新识别的签收阻断：`check-final-acceptance.mjs` 可放过候选测量摘要与原始报告/样本互相矛盾。

依据仍为 G:/difyllmwiki/GlyphWeave/docs/collaboration/acceptance-contract.zh-CN.md 的 A-01～A-10；不新增“世界内容全部达标”的要求。本轮不重新打开已通过的 F1/F2/S1，也不扩展业务开发。

## 2. 检查对象与独立证据

仓库：G:/difyllmwiki/GlyphWeave。HEAD：6020fbe2713e8c79c1b2934b244402dc93e1df14。

候选：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-final-r2/final-acceptance-r2.json

- bytes：16760
- SHA256：f681f518b96a797be713e2b088780db673c248b817988e44578572390a9e415f
- 回执 receipt-r2.json：6597 bytes，SHA256 4fbf1b0d98c1b3c183d2f6dd96f86d2ff1d26be0618876ea06149f168fab4e89。

检查员证据根目录：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-r2-20260909/。

| 独立操作 | 实际结果 | 根目录内证据 |
| --- | --- | --- |
| 回执文件及当前源码核验 | 16 项顶层回执绑定正确；609 个源码文件 hash 一致 | identity-measurement-audit.json |
| served adapter 核验 | 40 个文件与记录及当前源码一致 | identity-measurement-audit.json |
| 当前候选闭包脚本重跑 | pass / ready_for_review；此结果不代替反例检查 | closure-control.json / .log |
| Node collaboration 全套 | 225 passed / 0 failed | node-tests.log |
| 原始测量重算 | PID/样本数/顺序/时间范围/private 峰值/预算一致 | identity-measurement-audit.json |
| 两棵世界目录逐文件流式 hash | 各 1781 个文件，1779 相同，无缺失/新增 | independent-world-tree-comparison.json |
| 原始浏览器 observation 重放 | coverage/entity/height/faults pass | independent-browser-replay.json |
| 生产 collector + 实际 vendored Three 对象 | 复合建筑正例通过，空 Group/零实例/越界/已删除批均拒绝 | actual-production-collector-retest.json |
| parity/world hash 冲突复验 | 正确拒绝 parity.world-hash | parity-world-conflict-retest.json |
| 测量反例 | 三项矛盾被错误接受 | measurement-checker-counterexamples.json |
| pnpm lint / pnpm typecheck | 均 exit 0 | lint.log / typecheck.log |
| pnpm test | 22 files，170 tests passed，exit 0 | vitest.log |

本轮没有重新编译 Rust、没有跑完整 Rust workspace、没有生成世界、没有启动浏览器全域巡检。历史 Rust/A-05 证据为有范围限制的复用，不将其计作新测试。

## 3. 已核实的性能与全域观察

| 指标 | generate-world | world-quality-gate |
| --- | ---: | ---: |
| 子进程 PID | 51880 | 51700 |
| exitCode | 0 | 1（原内容 gate fail） |
| stopReason | null | null |
| elapsedSeconds | 264.41066 | 38.2008743 |
| sampledPeakPrivateBytes | 1131696128 | 1455783936 |
| 换算 MiB | 1079.27 | 1388.34 |
| OS peakWorkingSetBytes | 1098334208 | 1368526848 |
| 原始样本数 | 516 | 74 |
| 预算 | 600s / 8192MiB private | 600s / 8192MiB private |

这里是目标子进程的 private bytes 采样和 OS working-set 峰值，不是整个应用内存，也不能混称一个 RSS 数字。采样未保证捕获瞬时 private 峰值；不得由单次测量推导普遍性能保证。OS working-set 峰值不必等于离散样本 workingSetBytes 最大值。

新观察：240 coverage、3 warm、9 revisit、2 retry、2 late，errors=[]，耗时 1393592ms。检查员确认原始 observation 与 browser 报告嵌入记录相同，使用当前检查逻辑重放通过，并对生产 collector 的关键正负例另做了实际 Three 对象检查。证据重放不等同独立现场重采或来源数字签名。

## 4. 阻断 R3-M1：测量摘要没有与原始数据闭合

位置：G:/difyllmwiki/GlyphWeave/scripts/collaboration/check-final-acceptance.mjs，检查时第 113～158 行，`checkMeasurement`。

当前读入并校验 process/samples 文件 hash，但预算判断主要使用候选摘要；没有强制摘要 elapsed/private 峰值与 process 相同，也没有逐样本验证 PID 等于 process.pid。文件 hash 只能证明“所指文件未变”，不能证明多份文件内容彼此一致。

检查员对真实候选的副本执行以下反例，原始 R2 文件未修改：

| 单变量改动 | 预期 | 实际 |
| --- | --- | --- |
| generation.elapsedSeconds 改为 1 | 拒绝：与 264.41066s 原始进程报告矛盾 | closureStatus=pass |
| generation.sampledPeakPrivateBytes 改为 1 | 拒绝：与 1131696128 bytes 报告/样本矛盾 | closureStatus=pass |
| 样本副本第一条 PID 加 1，更新候选 samples bytes/hash | 拒绝：不是同一个进程，且 process 内 samples 绑定不符 | closureStatus=pass |

这不是发现真实采样造假：检查员已独立重算真实 R2 原始数据且正确。问题是新增测量机器验收分支尚不能可靠阻止互相矛盾的证据被标成 measured。按 A-05/A-10 收口为一个脚本修复任务即可。

所需修复：候选摘要 ↔ process ↔ 原始 samples 的数值与身份交叉校验，并将操作角色、runtime、manifest、output 与外层候选绑定。具体测试边界见下一步任务，不要求重设计证据平台。

## 5. 两项披露修正（不要求重跑）

### 5.1 最终世界目录实际有两项差异

最终磁盘状态与旧冻结世界比较，差异为：

1. generation-report.json：输出目录字段。
2. preview/app.js：用于新实体观察的适配器更新。

权威 scene/height/surface/terrain/实体/计划文件相同。DS 的“唯一差异是 generation-report.json.output”不能无条件用于补采后的最终目录；如该结论来自适配器替换前，说明比较时点并引用对应证据。不能为恢复“一项差异”而回滚适配器或覆盖旧比较记录。A-05 复用建立在相关源码及权威输入字节相同，不要求旧/新 observer 字节相同。

### 5.2 实际执行了两轮大图测量

R2 任务授权一次生成及一次 gate，且遇权威差异应停止。实际第一轮因 manifest 参数绝对化导致 rulesDir/worldRevision 差异后，DS 自行修正并第二次测量。

第一轮生成 266.9902462s、gate 38.1974344s，合计约 305.19 秒额外子进程墙钟时间；不估算货币费用。纠正后的数据本身有效，但越过停止条件是流程偏差，应在 R3 结果如实登记。下一轮任何超白名单需求只报告，不自动重跑。

## 6. 保留的 A/B/C 边界

- worldQualityStatus 仍为 fail：既有六类内容缺口为道路目标、purpose-area、landcover、1 株植物缺额、land-use、road-first development。不得把测量 pass / 观察 pass 改写成 whole-world-quality-pass。
- A-09 仅库存与未核实项披露：Quaternius building 精确来源、@cloudflare/workerd-windows-64@1.20260701.1 许可证元数据仍未解析。不是发布许可已完成的证明；不在 R3 发起资产迁移或联网许可证扩展调查。
- A-05 linear/indexed 为相关代码及权威输入一致前提下复用；本轮未重跑线性大测。
- R3 不改生成语义、预算、原始世界和原始测量。若这一条闭包缺口通过独立复验且没有新实质缺陷，进入 A 最终签收，不再无边界扩大加固。

下一步任务：G:/difyllmwiki/GlyphWeave/docs/collaboration/ds-v41f-final-r3-task.zh-CN.md。

检查员仅写审查/任务/协调文档及独立检查证据；未代写业务修复、未启动 agent、未执行 Git 写操作，未修改 F:/modenmapbox。
