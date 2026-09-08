# A-FINAL-R3 独立复验：主要修复通过，UTC 顺序漏项待补

日期：2026-09-09（Asia/Shanghai）。主 Agent 独立检查，未代写生产修复。

## 裁定

`needs-changes`，范围仅 R3 原任务 2.3 中未实现的 UTC 顺序检查。暂不签署里程碑 A。不是发现真实测量错误，不要求新的性能测量或浏览器观察，也不扩展 B/C。

R3 已完成耗时/峰值摘要交叉一致性、样本 PID 绑定、private 峰值重算、elapsed/CPU 顺序和外层 runtime/manifest/world 对应检查。原先三个阻断反例均已独立验证修复有效。

## 独立执行结果

检查证据目录：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-r3-20260909/。

| 检查 | 结果 |
| --- | --- |
| R3 候选/闭包报告/回执/反例文件的用户所报 hash | 全部匹配 |
| 回执的顶层文件 bytes/hash | 全部匹配 |
| source-manifest-r3 当前文件复核 | 612 个文件全部匹配 |
| R2→R3 源码范围比较 | 代码仅 final checker 和其 test 变化；无清单文件删除 |
| 未改 R2 候选运行当前 checker | exit0，pass / ready_for_review |
| R3 候选运行当前 checker | exit0，pass / ready_for_review；38 evidence、18 logs |
| collaboration 全套独立重跑 | 248 passed / 0 failed |
| 两个 mjs 语法检查 | exit0 / exit0 |
| R2 真实 UTC 顺序复核 | generation 516 条、gate 74 条均有效且非倒退 |

源码快照对比还包含主 Agent 上轮追加的 status 文档与两份审查/任务文档，不能误归因于 DS 越权修改。源码 hash 不是二进制编译来源证明；本轮不重编 Rust，仍沿用已记录的 runtime 来源链。

记录：identity-review.json、node-tests.log、independent-counterexamples.json、real-utc-order.json；可复现检查脚本为 review-r3.mjs（输出用 wx，重执行前应指定新输出目录，勿覆盖旧证据）。

## 已修复反例

以下均在真实 R3 候选副本上独立运行 CLI，原始证据未修改：

| 反例 | 实际错误 ID | exitCode |
| --- | --- | ---: |
| 候选 elapsedSeconds=1 | generation.elapsedSeconds-mismatch | 1 |
| 候选 sampledPeakPrivateBytes=1 | generation.sampledPeakPrivateBytes-mismatch | 1 |
| 样本错 PID，samples/process 绑定同步刷新 | generation.sample-pid | 1 |
| elapsed 倒退，绑定同步刷新 | generation.sample-time | 1 |
| CPU 倒退，绑定同步刷新 | generation.sample-cpu | 1 |
| UTC 无法解析，绑定同步刷新 | generation.sample-utc | 1 |

## 唯一退回项 R3-U1：UTC 可解析不等于顺序正确

文件：G:/difyllmwiki/GlyphWeave/scripts/collaboration/check-final-acceptance.mjs。

当前循环计算 `const utc = Date.parse(row.utc)` 后，只判定它是有限数；维护了 lastElapsed/lastCpu，却没有维护 lastUtc。因此未落实原任务“UTC 可解析，时间顺序一致”这一要求。

独立反例：仅将 generation 第二条样本 UTC 改成 `2000-01-01T00:00:00Z`，保留该条 elapsed/CPU/PID 等数值，并更新副本的 samples bytes/hash 及 process samplesJsonl/path/hash 和外层 process 绑定。checker 仍 exit0、closureStatus=pass、readiness=ready_for_review。

这不是用 hash 错误制造失败，而是证据内部的 UTC 时间倒退仍能通过。原始采样 UTC 已独立确认顺序正确，故只需补检查与测试，不需重采。严重性 P2；因为本项已在 R3 明确验收范围内，不能把它标作全部完成或静默豁免。

## 其他事项

- R3 已在新文档披露 R2 最终目录的两项非权威差异和两次测量，不再退回这两项。
- R2 大图实测、240 块观察、A-05 相关字节等价复用不重新打开；历史证据保留。
- 质量 gate 六类内容失败、两项许可未解析继续保留；未作开源发布许可已完成的断言。
- 当前检查到 GlyphWeave 历史 preview PID 60644 / 8099，未停止或修改；另一个 grassland 项目的 Vite 进程与本任务无关，不处理。当前进程列表不能证明整个执行期间的进程历史。
- HEAD 仍为 6020fbe2713e8c79c1b2934b244402dc93e1df14。主 Agent 未执行 Git 写操作、未启动 agent、未修改 F:/modenmapbox。

下一步仅执行 G:/difyllmwiki/GlyphWeave/docs/collaboration/ds-v41f-final-r3-u1-task.zh-CN.md。补完这一原任务漏项后，仅复验增量和候选，不发起新的泛化审计轮次。
