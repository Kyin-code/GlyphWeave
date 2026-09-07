# GlyphWeave 双协作者说明、调度与成本控制（v2）

日期：2026-09-08（本地 Asia/Shanghai）。**本版替代 2026-09-07 初稿的并行 A/C 派发。**

## 1. 目标与职责

- 当前交付：Milestone A 稳定底座；最终交付：Milestone C 自然—工程—人文耦合。B 街区/聚落生成是 A→C 的必经内容，不删除。
- 用户要求主 Agent 只做整体把控、接口/范围冻结、验收和集成评审；业务实现交给协作者。主 Agent 不与协作者重复实现，也不擅自扩大任务。
- 权威仓库 `G:\difyllmwiki\GlyphWeave`；禁止修改 `F:\modenmapbox`。当前分支 `refactor/authoritative-baked-audit`，大量历史 dirty/untracked 文件，禁止 reset/clean/批量覆盖/删除证据/自行提交或 rebase。
- 目标不因时间或费用压力缩水；采用小批次交付、先证明再集成降低返工，不以降低需求、放宽阈值、删对象换绿色。

## 2. 模型身份与分工依据

用户指定 GPT-5.6 Terra、DeepSeek v4f。当前工具可直接指定 `gpt-5.6-terra`，没有 DeepSeek v4f 调用入口；真实 DeepSeek 接入前，其任务为待派发，不得以默认模型替代或声称两者都运行。

分工是风险/可用性策略，不是未经测试的模型能力排名：Terra 承担跨模块 Rust 约束和数据一致性；DeepSeek 首先承担输入输出明确、写入边界窄的自动化验收和证据工具。用首批任务的正确性、返工、可取得的耗时/费用数据调整分工；没有供应商账单就不报告金额或“节省百分比”。不假设某模型更便宜、擅长地理或更强。

| 角色 | 当前 A 职责 | 不负责 |
|---|---|---|
| 主 Agent | 定义接口、冻结输入、审查差异、独立运行最终验收、结果/失败报告、调度 | 重复写业务代码、以自评替代验收 |
| Terra | A-T1 固定入口与最终图一致性；后续 A-T2 生态性能、A-T3 存量/稳定 ID | HTML 验收脚本、提前做自然侵蚀/城市新内容 |
| DeepSeek v4f | A-D1 只读证据校验；A-D2 全域 HTML 漫游/内存记录；A-D3 大文件工具与来源清单 | 未授权修改 Rust 生成/门禁、以截图证明几何正确 |

## 3. 事实基线及不能混淆的状态

最新已完整生成的基线是 `G:\difyllmwiki\GlyphWeave\bevy\work\r49-frontage-maximum`：

- 6000×10000m，seed=7；输入 `G:\difyllmwiki\GlyphWeave\examples\maximum-local-height-frontage.manifest.json`。
- 输入 SHA256：`D4FE4D8C72783067F6996621F2836FD5BBC8A154F3FB81C6D2E220FBA9154749`。
- 生成 388.4590032s；完整 gate 176.5044953s，**fail**。这不是 SLA，也不是整个项目通过。
- 786 实体、270/270 道路、300/300 站点、新 frontage 144/144（120住宅+24商铺），原设施4/4；城市建筑72000m²。
- 自然 hash `823ef8a1fe17f607bef7c6a7808589eca235f0cd046538421defe1dcd924ba06`；保护高度改动0；农田428；生态342379/342380；未实现50230050m²。
- 六类 gate 失败是道路目标、用途实现、地表实现、生态缺额、土地规划/实现、road-first 开发；记录原文，不只比较失败条数。
- 旧 r48 超时目录不是完整世界。r49 原线性/索引农业同输入95.1673948s/10.0373725s，完整结果与发布文件hash相同；不能外推为整图9倍加速。
- **当前源码不等于已验收 runtime**：在 r49 生成之后新增了构图后重置 frontage entity 的代码，仅看到 core369通过/2ignored和check记录，未完成最新全工作区与最大图验收。A-T1 优先检查这个顺序风险，不能复用旧世界证明新改动正确。
- 仓库已有 MIT `LICENSE`、`THIRD_PARTY_NOTICES.md`；此前“缺少顶层LICENSE”的说法不准确。现有文件不证明全库代码/模型/数据来源合规。

证据以 `bevy/work/r49-maximum-summary.json`、生成/gate测量、农业reference报告及当前源码为准；上面日期与旧叙述冲突时看产物和日志，不按记忆认领作者或工时。

## 4. 派发与写入锁

首批只派 A-T1 和 A-D1。其余任务排队；C 当前仅维护最终要求，不启动 C 实现。

- Terra A-T1：`bevy/crates/core/src/worldgen.rs` 的 final-entity-access-and-audit 窄块、`worldgen/access.rs`、`worldgen/frontage.rs` 测试、`bevy/crates/cli/tests/baked_audit.rs` 相关测试、`docs/collaboration/a-t1-result.zh-CN.md`。
- DeepSeek A-D1：新 `scripts/collaboration/check-evidence.mjs`、`scripts/collaboration/check-evidence.test.mjs`、`docs/collaboration/a-d1-result.zh-CN.md`、独立 `bevy/work/collaboration/deepseek/`。现有 Rust/HTML/manifest/gate 全部只读。
- 主 Agent：本协作说明、任务书、验收合同、状态板、审核报告；需要实现修正则退回责任人，不在背后补另一套实现。
- 无共同写文件。发现必须跨边界修改，先提交接口需求，停止该部分而继续可独立部分。主 Agent 明确授予锁后才能修改。
- 外部 DeepSeek 若共享目录，必须先读锁表/检查自身 git 状态；若在其他 checkout，需接收包含当前未提交基础的快照及hash，不能从落后的 main 盲目cherry-pick。返回相对该快照的补丁，不用整个目录覆盖。
- 不创建额外用户任务、额外协作者或模型替代；须用户授权。两个角色是上限，不是必须同时烧资源。

## 5. 执行、失败与预算

1. worker先返回理解、拟改文件、验证命令；按已授予的小任务执行，不重新阅读全项目或重复外网研究。
2. 小批次：一个问题、一次实现、针对性测试、一次评审；不能把A或C整体当作一个无限任务。
3. 同一失败原因最多两次实质修复尝试；第三次前提交最小复现、日志和决策选项给总控。保留失败，但这不自动标记整个项目blocked。
4. 完整生成/quality gate/线性基准/全workspace按资源槽串行运行；协作者小型检查可并行，禁止相互污染性能基准。发起前总控授予 RUN 槽。
5. 最大图实验保持600s/8192MiB；这是生成和完整gate各自的进程预算。超时保留原失败，扩展研究预算需另行明确，不改原通过口径。
6. 初始worker不自行跑最大图、重复基准或启动预览。交付候选之后总控才集中运行。只读文档改动不触发完整重生成。
7. 运行runtime副本，不锁 `target/release/glyphweave.exe`；保存具体PID/session、exe与输入hash。不得按进程名杀其他人的预览；观察超时不代表进程结束，不盲目重启。
8. 不重复打印几百MB JSON，读取摘要或流式解析；报告实际token/费用仅在工具/账单提供时填写，其他填unknown。
9. 除人工确认/真实阻塞/里程碑交付，避免高频进度询问；不设置自动轮询或定时任务。

## 6. 交付与验收

统一交付：状态 `ready_for_review / needs_changes / blocked`，不可自行宣布 milestone_complete；含输入/基线身份、改动文件、命令退出码、失败记录、证据路径、行为变化/兼容性、许可证变化、已知缺口。格式见 `docs/collaboration/acceptance-contract.zh-CN.md`。

所有结果必须区分：计划≠实体、生成退出0≠质量通过、局部通过≠全域通过、旧runtime≠最新源码、截图≠可达性证明。A可以在完整gate保留经批准的B/C内容缺口下验收稳定底座，但不能声称完整质量gate通过。具体例外必须逐条核对，不能通配忽略。C最终需要其完整范围的验收，不继承A例外。

主 Agent 合并流程：只审查本任务增量→核对原需求→针对性复现→接口/模块测试→锁定候选源码/输入/二进制→一次完整集成验收→决定通过或退回。未测试或间接证据一律视为未完成。
