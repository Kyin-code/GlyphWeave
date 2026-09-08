# 里程碑 A 检查员正式签收（R3-U1 收口）

日期：2026-09-09（Asia/Shanghai）。签收者：主 Agent / 独立检查员。实现执行者：外部 DS v4.1f 及此前已审查的协作者。

## 1. 最终裁定

**`milestone-A-accepted`：里程碑 1 / A「稳定底座」通过。**

A-FINAL-R3-U1 的最后一个已知验收阻断已独立复验关闭。本裁定以 G:/difyllmwiki/GlyphWeave/docs/collaboration/acceptance-contract.zh-CN.md 的 A-01～A-10 为边界，结合前轮审查与当前增量复验，不是仅凭 worker 自报或机器 closure=pass 签收。

**worldQualityStatus 仍为 `fail`；不是 whole-world-quality-pass，不是里程碑 B/C 完成，不是全项目完成，也不是所有资产已经具备开源发布证明。**

本文件作为最新主 Agent 裁定，接续并更新此前 R1/R2/R3 的 needs-changes 状态；保留所有历史审查及 worker 原始候选，不修改旧结论来抹去过程。

## 2. 最终候选身份

仓库：G:/difyllmwiki/GlyphWeave。
HEAD：6020fbe2713e8c79c1b2934b244402dc93e1df14。签收的是记录 hash 的工作区候选，**不是声称这个旧 HEAD 已包含全部交付代码**。未执行 Git 暂存、提交或推送。

| 对象 | SHA256 / 身份 |
| --- | --- |
| R3-U1 增量候选，21382 bytes | d7689f9c9ebd6b6a34c9952cc741a165f26946f4016e59202bdb965d2dd907c9 |
| 保留的原 R3 候选 | d5b1aa9fb2725a96692b99045bc0423a85b227661418416c58dc02cc3379d433 |
| 保留的原 R2 候选 | f681f518b96a797be713e2b088780db673c248b817988e44578572390a9e415f |
| U1 checker，21347 bytes | 1891dde0f5e9b215ba5d45d2d7220ee6239ffb7fd211ea940b8eddbb01e299e8 |
| U1 test，18470 bytes | e7c2e68f5abbfd26daa3583fa23aa86482d2d5cf1d526c3a549f0835dcf99b4c |
| U1 worker receipt，7328 bytes | 2947434ba3facce6f03a3c3d36533a3984555e20b04706734565a779cdbc552f |
| 原生成 runtime | 99374af17e471e46e5c09185e28775fcd6661e71e025d6a45aa49f2e521b3bc8 |
| 独立 audit runtime | e414d4ae08fb7b66a05671d3d32db463f2870df5b149b73ff4ebd9648520c4da |
| manifest | d4fe4d8c72783067f6996621f2836fd5bbc8a154f3fb81c6d2e220fba9154749 |
| worldRevision | bcc8328c0618ac78756a37bd7eaf013db8098a4f9a73aec19e583822242e52c0 |

候选完整路径：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-final-r3-u1/final-acceptance-r3-u1-rebound.json。

原候选直接运行新 checker 因其 R3.checker 仍绑定旧脚本 hash 而失败，属于正确的版本绑定拒绝。本次逐字段确认增量候选只有 **8 个标量变化**：evidence 中 checker/test 两条及顶层 checker/checkerTest 的 bytes、sha256。世界、runtime、manifest、测量、parity、浏览器观测和其他字段未改。检查员独立构造同样的重绑定，结果与 worker 增量候选一致；此差异不再退回。

## 3. U1 独立复验结果

证据根目录：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-r3-u1-20260909/。

- 回执 12 项顶层文件绑定 bytes/hash 全部通过。
- R3 清单 612 项重新检查：相对该快照仅 checker、test、主 Agent 追加的 status 文档不同；没有未解释的代码变化。新增审查文档为记录增量，不冒充旧 runtime 构建输入。
- `node --test scripts/collaboration/*.test.mjs`：检查员独立运行，**252 passed / 0 failed**。
- 两个 mjs 的 `node --check`：均 exit0。
- 正常候选：exit0，closureStatus=pass，readiness=ready_for_review，38 evidence、18 logs。
- 先前检查员的 UTC 倒退反例，仅刷新脚本绑定后：exit1，`generation.sample-utc-order`。
- 独立 gate UTC 倒退反例：exit1，`gate.sample-utc-order`。
- generation/gate 的相等 UTC 正例：均 exit0 / pass，未错误要求严格递增。
- 对所有反例，样本、process 与候选 hash 绑定已同步刷新，保证是语义检查拒绝，不是碰巧被旧 hash 拦截。

主要证据：identity-review.json、independent-utc-retest.json、control-check.json、node-tests.log；复现脚本 review-u1.mjs。脚本用 wx 保存证据，若复跑需指定新输出目录，不覆盖旧结果。

## 4. A-01～A-10 签收矩阵

| 合同 | 签收范围与依据 | 本轮是否重新执行重任务 |
| --- | --- | --- |
| A-01 源码/输入/runtime 对应 | R2/R3 源码与 runtime 来源链，U1 两个 JS 文件增量 hash；检查员核对当前源码变化范围与候选绑定 | 不重新编译；沿用既有已审查构建来源，不把 JS 增量说成 Rust 重建 |
| A-02 固定实体/入口/访问图 | 已审查 frontage 入口修复与正负毫米测试；最终 quality frontage requested/planned/realizedWithAccess=144/144/144，unfilled=0 | 重读绑定报告；不生成新场景 |
| A-03 确定性/稳定 ID/存量约束 | 已绑定 stable-v1 / inventory-preserving 回归及历史完整 workspace 日志；R2 权威世界与冻结世界字节等价 | 复用相关 Rust 源码未变的证据 |
| A-04 最大图/独立 gate | 6000×10000m，240 chunks；R2 原生成及 gate 独立测量，六类批准内容失败原样保留 | 不重跑 |
| A-05 性能/等价 | R2 测量原始样本已独立重算，R3/U1 机器交叉一致性与负例关闭；linear/indexed 等价按相关源码及权威输入未变复用 | 不重新跑 linear 或测量 |
| A-06 权威数据/投影闭包 | 已修复并复验 Rust header/scene audit、auditInput 和三个消费者绑定；最终 parity 与 source/header/world 绑定 | 当前 closure 核对；不重新编译 audit |
| A-07 240 块浏览器生命周期 | R2 新观察 240 coverage、3 warm、9 revisit、2 retry、2 late，实体/高度/faults pass；主 Agent 前轮原始记录重放、生产 Three 对象反例通过 | 不重新进行 23 分钟巡检 |
| A-08 自然/保护/最低需求 | 当前绑定 quality/gate 及此前独立保护/来源检查；自然路由完整性 pass，保护与需求差异显式记录 | 复用相同权威内容；不外推成真实侵蚀或水力安全 |
| A-09 许可库存/披露 | 既有库存、Three LICENSE、建筑 NOTICE；两项未解析继续明列 | 仅合同要求的库存与披露通过，不作发布许可认证 |
| A-10 正负回归/文档 | 本轮 Node 252；前轮独立 Vitest 170、lint/typecheck；历史 Rust workspace 470 passed/3 ignored 及 R1 audit 增量测试；当前 docs/tree/diff 检查 | JS 补丁只重跑相关 Node，不假称本轮 Rust/Vitest 重跑 |

历史审查依据：
- G:/difyllmwiki/GlyphWeave/docs/collaboration/a-final-inspection-r1.zh-CN.md
- G:/difyllmwiki/GlyphWeave/docs/collaboration/a-final-inspection-r2.zh-CN.md
- G:/difyllmwiki/GlyphWeave/docs/collaboration/a-final-inspection-r3.zh-CN.md

上述历史证据的复用不等同“当前代码任意变化都自动继承通过”。后续改动相关模块，需要按影响面更新证据。

## 5. 性能口径与保留失败

R2 纠正后的真实测量：

| 阶段 | elapsedSeconds | sampledPeakPrivateBytes | OS peakWorkingSetBytes | 样本 | 子进程退出 |
| --- | ---: | ---: | ---: | ---: | ---: |
| generate-world | 264.41066 | 1131696128（约1079.27MiB） | 1098334208 | 516 | 0 |
| world-quality-gate | 38.2008743 | 1455783936（约1388.34MiB） | 1368526848 | 74 | 1 |

两者预算均为 600s / 8192MiB private bytes。测量范围是子进程，private 峰值是离散采样峰值，不是全应用内存或瞬时峰值保证。浏览器记录总耗时 1393592ms。单次测试不证明任意种子/机器都满足同样性能。

完整 gate 仍 fail，六条原失败不隐藏、不清零：

1. `"scene-0" automatic road targets remain out of reach after "segment_budget_exhausted"`
2. `"scene-0" purpose-area allocation or realization remains incomplete`
3. `"scene-0" landcover remains unrealized`
4. `"scene-0" ecology density budget has 1 unfilled plants`
5. `"scene-0" land-use planning or realization remains incomplete`
6. `"scene-0" road-first land-use development remains incomplete`

自然审核范围仍是 sampled-routing-integrity-not-hydraulic-safety；报告仍有 closed sinks / unresolved flats，不能解释为全域水文物理模拟已完成。建筑 Quaternius 精确上游包及 @cloudflare/workerd-windows-64@1.20260701.1 的许可证元数据仍未核实；A 通过不等于这些材料可以无条件公开发布。

R2 第一轮参数绝对化失败、第二轮纠正测量的流程偏差，及最终目录 generation-report.json.output / preview/app.js 两项差异已如实披露；不删除旧记录。

## 6. 收口与下一步管理

1. A 修订队列关闭，不再派 A-FINAL 的新加固/补采任务；worker 保持停止，不自行启动 B/C。
2. 冻结本候选、runtime、世界、原始采样/观察和检查员签收记录。bevy/work 下证据有 Git 忽略项，**本次尚未执行证据归档或 Git 提交**；正式版本存档需要另行安排，不能以文档中的路径当成已备份。
3. 后续内容开发建议先围绕道路目标—可达地块—用途分配—实际地表/建筑实现的缺额闭合，沿用 Rust-first 与 Agent 只提供需求的边界。不以放宽 gate、改低目标或藏掉缺额来完成 B/C。
4. B/C 开工前由检查员单独整理并批准窄任务书、样例和运行预算；不得自动沿用 A 的六条例外作为 C 合格条件。
5. 在真正公开发布前收口未核实的资产/依赖来源；本文件不提供法律合规意见。

这是管理建议与交接边界，**不是授权 DS 立即进行上述开发、归档、提交或清理**。当前无需再向 DS 派 A 补修任务。

主 Agent 本轮仅审查、运行 Node 小测试和写签收记录；没有生成/gate/浏览器重采、Rust 编译、调用其他 agent、Git 写操作或修改 F:/modenmapbox。
