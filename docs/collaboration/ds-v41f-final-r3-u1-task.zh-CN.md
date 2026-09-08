# 转交 DS v4.1f：A-FINAL-R3-U1，仅补 UTC 顺序漏项

日期：2026-09-09（Asia/Shanghai）。本任务是 R3 原有要求的小补修，不是新增阶段。

## 1. 必读与冻结

读取 G:/difyllmwiki/GlyphWeave/docs/collaboration/a-final-inspection-r3.zh-CN.md。

R3 主要修复和三项原反例已独立通过，248 项测试通过，R2/R3 候选正例通过。唯一漏项：UTC 只检查可解析，未检查倒退。真实 R2 generation 516 条、gate 74 条 UTC 顺序均正常，不需要重采。

只工作于 G:/difyllmwiki/GlyphWeave，不改 F:/modenmapbox。HEAD 6020fbe2713e8c79c1b2934b244402dc93e1df14。禁止 git add/commit/push/reset/clean/checkout/switch/stash/rebase/merge 或索引/配置写入；不启动其他 agent；保留所有他人 dirty。

允许修改仅：
- G:/difyllmwiki/GlyphWeave/scripts/collaboration/check-final-acceptance.mjs
- G:/difyllmwiki/GlyphWeave/scripts/collaboration/check-final-acceptance.test.mjs

允许新增仅：
- G:/difyllmwiki/GlyphWeave/docs/collaboration/a-final-r3-u1-result.zh-CN.md
- G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-final-r3-u1/ 下自己的新证据。

原 R2/R3 候选、source manifests、process/samples、runtime、world、browser、parity、旧文档、检查员证据全部只读。不要改 status 或旧回执。

## 2. 最小修复

在现有 samples 循环保存上一条已解析的 UTC，要求当前 UTC >= 上条 UTC，失败用明确的检查 ID（建议 `${label}.sample-utc-order`），CLI exit1。

沿用现有 elapsed/CPU/private 检查，不重构外围逻辑、不改预算或质量状态。允许解析精度内相同的 UTC（例如 Date.parse 的毫秒精度），不要改成严格递增。process.timestampUtc 是结束记录时间，不把它当作开始时间。不要顺带扩展任意 wall-clock 漂移/历史时间认证框架；这里只实现原任务已要求的相邻顺序检查。

## 3. 验收

1. 增加 UTC 倒退负例：elapsed/CPU/PID 保持正常，UTC 第二条早于第一条；同步刷新样本、process、候选绑定后，仍必须明确拒绝。
2. 增加相等 UTC 正例，防止毫秒精度造成误拒；已有正常 UTC 正例继续通过。建议同时用小 fixture 对 generation/gate 验证该通用分支。
3. 定向运行 final checker test，以及一次全部 collaboration tests；两个 mjs 执行 node --check。
4. 用修复后的 checker 复验原 R3 候选（只读引用，不能改它）：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-final-r3/final-acceptance-r3.json，SHA256 d5b1aa9fb2725a96692b99045bc0423a85b227661418416c58dc02cc3379d433。应保持 pass / ready_for_review。
5. 对检查员 UTC 反例候选只读重验：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-r3-20260909/backward-utc-rebound-candidate.json。输出保存自己的全新路径，应明确 exit1。
6. doc-tree:check、git diff --check，记录实际结果，不修改白名单外文件。

交付只需新结果文档和 receipt：列本轮 checker/test hash、命令/退出码/日志、新正负例结果、被复验原 R3 候选 hash。无需重建整个候选证据网络或复制原世界；将本轮脚本增量回执附加于原 R3 候选，交主 Agent 复验。

禁止 generate-world、world-quality-gate、cargo test/build、Rust 编译、浏览器巡检、linear 大测、启动预览服务、下载依赖。不能修改原始采样来让测试通过。只写 ready_for_review，不自行 milestone-A-accepted，不推进 B/C。

完成后停止等待检查员。任何白名单外需求先报告，不自行扩大。
