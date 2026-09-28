# B01-G3-R1 收口：结论纠正、完整交集诊断与晚截止测试

* 任务：`docs/collaboration/ds-v41f-b01-g3-r1-closeout-task.zh-CN.md`；先读的独立审查：`docs/collaboration/b01-g3-v5-bundle-inspection.zh-CN.md`
* 检查员证据：`bevy/work/collaboration/inspector-b01-g3-20260918/level-probe.json`
* HEAD：`fb36dc4f9fdd38e6aa9c5a89676a65b96e5001b1`；工作区 `G:/difyllmwiki/GlyphWeave`
* **状态：`closeout_complete_needs_inspector_review`**（不是"增量见证"，不是"B 轴耗尽"）。
* 本轮**没有**自然五态 harness、**没有** natural baseline/candidate realize、**没有** bake、**没有** 128/80clock/carve/CLI 世界/gate/release/浏览器/最大图/workspace；未改生产算法或合同；无 Git 写操作、无 agent、未动 F/A。

## T1 结论、回执与搜索口径纠正

1. **2560mm 是下界，不是唯一可行值。** `8000 − 80×68 = 2560mm` 只给出接入段在 B 的**可达下界**；1920/1812/2471 低于它、7692 高于填方包络，只说明这四个采样点被拒。检查员用公开 `road_alignment::design/verify` + `road_extensions::append` 在冻结原 plan/natural/zone/水体/scene 上证明 **B=3000mm 的接入段全部通过（35→36 边）**；该探针未运行街段或完整 realize，故**不得**据此宣称街段或 8 地块通过。
2. **正确口径**：预注册五点已执行完，**B 取值域未穷尽**；没有证据表明完整 V5 必然不可能，也没有真实增量。
3. **未观测项保留**：五态全部停在与设计/匹配阶段，**没有进入测量或 realize**；正路径 `not_exercised`，不能写成"测量 0 地块"。
4. 旧 G3 文档只追加勘误（§8）；**旧 receipt、旧候选表、两次 run 文件字节不变**；本收口轮的新回执绑定新结果与检查员来源。

## T2 完整交集诊断修正（仅测试区，生产拒绝规则不变）

`auto_roads.rs` 测试区新增 `joint_support_diagnostic(approach, street, rect)`：

* 两侧高度都通过**生产** `RoadProfile::height_at`（端外钳制）读取，不再用 `(288 − x) as usize` 加原始数组 `get`；
* 比较矩形由**两实体生产支撑几何**导出（`length/2+4` 沿轴、`width/2+3` 横向的闭区间）；
* 闭区间**每格都必须取到两个值**，任何缺失记 `missingValues` 并置 `diagnostic_incomplete = true`，绝不静默跳过。

未改动生产阈值、plateau、自然面、fill 或旧道路；旧 140/23 证据文件保持原样，纠正写在本文与新回执中并链接检查员来源。

新增合成回归测试 `joint_diagnostic_covers_the_padded_closed_intersection`：用两条合成生产 `RoadProfile` 复刻 S01 的接入末端（station 75 = 2640mm、平台 2560mm）与街面开头（station 9 = 2480mm、其余 2560mm）序列，
断言闭区间 `[279,171,292,184]` **遍历 196 格**（含 x=289..292 四列）、**0 缺失**、**27 格不匹配**。
**真实 196/27 来自检查员已交付的 production probe**；本 fixture 只是诊断的合成回归，不冒充本轮自然重测。

## T3 晚截止保护（仅测试区，不重开生产时钟项目）

测试区新增可注入时钟的结果封装 `BundleOutcome`：`check()` 在 baseline 之后、设计/测量长阶段返回之后、realize 返回之后、写见证之前、最终输出之前**一致复查**截止；任何晚到期都置 `incomplete = true`、清空已暂存见证，并让调用方丢弃整个 staged 束。成功输出必须同时满足真实信用 + `commit_decision` + 未截止。

新增 4 个 `ExpireClock` 小测试（无 sleep、无自然 realize、无 80 点扫描）：

| 测试 | 断言 |
|---|---|
| `a_late_expiry_after_the_last_realize_refuses_the_witness` | realize 前未截止、返回后已截止 → 见证不发布、`incomplete`、`expiredAt` 有值 |
| `a_late_expiry_after_a_design_refusal_marks_incomplete` | 设计拒绝时刚过期限 → `incomplete = true`，不报干净负结果 |
| `a_staged_witness_is_cleared_when_the_deadline_passes_before_output` | 已暂存见证在输出闸门前截止 → 见证被清空 |
| `an_in_budget_witness_survives_and_a_refusal_stays_clean` | 未截止时成功保留见证；零增量/普通拒绝保持干净、不提交 |

守卫测试 `a_mismatched_shared_bundle_node_is_refused_by_production` 仍通过（共享 B 值不一致被生产 `extension start junction height mismatch` 拒绝，暂存副本只多第一段，基线 plan 未变）。
本轮**未**调用 `v5_bundle_tries_for_one_real_parcel_increment`，也没有任何过滤条件匹配到自然用例。

## T4 下一实验计划（只提出，不执行）

在**未变的 A/B/C 坐标、width 12、junctionLength 8、fill 6000、rise 80、固定根 8000mm**合同下，选择**不同的** B/C 状态：

1. **B 轴剩余自由度（现合同内）**：`infer_end` 返回的 2560mm 是最小可行平台；B=3000mm 已由检查员证明接入合法。下一批应以**有来源的稀疏取值**覆盖 `[2560, 7692)` 中的不同高度（例如按自然包络/旧工程相容高程各取一个，而非均匀扫整数域），并逐项记录 `infer_end` 返回值与自选值的差异；不得据少数点宣称完整可行区间。
2. **C 轴与街段长度**：在 B 固定为某个已证合法值后，改变 C 的位置（缩短/平移街面）以避开旧路支撑，同时保持两个请求共用窗口、不缩小需求。
3. **一致性要求**：每个状态仍需 `A→B`、`B→C` 两段共享同一 B 高程，并由生产 `append` 逐格审核兄弟路段交集；失败即整束回退。
4. **单独标注的备选参数方案（不在现合同内）**：若下一步仍受"9m 支撑交集 vs 8m 路口平台"限制，可另立提案研究 `junctionLengthM` 或"共享支路"合同——它必须作为**新候选参数方案**单独审批，与"现合同内剩余高程自由度"分开陈述；本轮**不**实施、**不**声称必须修改合同、**不**自动转设计 B、**不**放开 carve。

## 预算（total / unique / actual / thisRound / history）

| 项 | 值 |
|---|---|
| G3 自然实验 total（worker） | 2 次执行；unique 状态 5；实际 attempts **10**；重复 5 |
| G3 candidate realization | 0（上限 3） |
| G3 baseline realization | **2**（授权 1，超额 1；如实计入，不抹去） |
| G3 运行时间 | 8.04s + 7.94s ≈ **15.98s**（整轮上限 180s） |
| thisRound（收口轮） | 小测试 6 个（T2 合成回归 1 + 截止 4 + 守卫 1）；编译 1 次；**自然 design/append/infer/realize 调用 0**；自然 harness 运行 0；bake 0 |
| history（非本轮） | R2 冻结 128 扫描 1 / carve 诊断 1；G1 harness 3 次 / 70 状态；G2 全场枚举条目 37,946；G2-R1 路线评估 18 |
| 检查员探针 | 单独计：`level-probe` 的 design3/verify2/append3，**不计入** worker 本轮运行 |

## 交付与局限

* 交付：本文件、G3 文档 §8 勘误、`bevy/work/collaboration/ds-b01-g3-r1-closeout/`（新回执、测试日志、源码 hash、旧证据未变检查、git/doc-tree 日志）。
* 局限：本轮的 196/27 是**合成回归**（真实值来自检查员 probe）；B=3000 只证接入段合法；完整 V5 在 3000 是否可行**未测**；仍无真实增量。B 保持 partial、C 不推进。
* 未签收、未开始下一自然运行、未批准任何 junction/carve 合同修改。
