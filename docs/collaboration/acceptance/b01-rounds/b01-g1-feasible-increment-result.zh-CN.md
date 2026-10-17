# B01-G1 结果：从 0 供给走向一个真实可行增量（受控小实验）

- 任务：`docs/collaboration/ds-v41f-b01-g1-feasible-increment-task.zh-CN.md`；先读的独立验收：`docs/collaboration/b01-supply-r4-cli-inspection.zh-CN.md`
- HEAD：`fb36dc4f9fdd38e6aa9c5a89676a65b96e5001b1`；工作区 `G:/difyllmwiki/GlyphWeave`
- 状态：**partial（ready_for_review）**。这是业务可行性试验，不是 checker 修订；A/B/C1～C3/CLI 来源审核链**未改**。
- 结论一句话：**在预先冻结的 22 个设计状态（6 条走廊）内没有取得真实增量，`bestLegalCandidate = null`**；这不等于全局无解，也不等于必须降低需求。

## 1. 冻结条件与研究假设

输入只读：`bevy/work/collaboration/ds-b04-c1-r2-b02-w1/w1-collector-local.manifest.json`
及其原 35 路 / 27 节点 / 21 义务建筑。seed 42、origin (0,0)、512×512、zone、需求
（16 homes + 8 shops，id/kind/count/尺寸/insets/setback）全部不变；固定节点毫米高程不变；
21 义务不删除、不搬移；fill 6000mm、坡度 80mm/m、肩 3m、路口 8m 与保护合同不变。

被检验的假设（任务给定）：现有 `infer_end` 只给**新增自由端点**选最小工程可行高程，
当前候选又只取 `tier_for` 返回的**一种宽度**（12m）；先检验"在同一工程包络内换一个合法端点高程 /
换另一个**已授权**宽度，能否换来一个真实地块增量"，而不是先去改全局 carve。

## 2. 冻结的候选表（执行前写入证据）

来源：`bevy/work/collaboration/ds-b01-supply-r2/append-solve.json`（R2 的冻结 128 状态追加评估）。
其中 16 个 `no_real_parcel_supply` 状态是"生产设计/append 通过、但实测 0 个真实地块"的近似命中走廊；
按要求排除 2 个 `wrong_purpose_or_protected`（保护/分区阻断）后取 6 条**不同几何**走廊。

表：`bevy/work/collaboration/ds-b01-g1-feasible-increment/candidate-table.json`（create-new）。

| 走廊 | start → end | 长度 | 端点类型 | 原实测结果 |
|---|---|---|---|---|
| c1 | 416,160 → 480,160 | 64m | 新增自由端点 | 0 地块，`existing_support_changed`（8 次站点） |
| c2 | 288,224 → 224,224 | 64m | 新增自由端点 | 0 地块，`existing_support_changed`（8） |
| c3 | 288,224 → 288,96 | 128m | **既有节点（固定高程）** | 0 地块，`existing_support_changed`（20） |
| c4 | 288,96 → 288,160 | 64m | 新增自由端点 | 0 地块，`occupied_or_road_support`（8） |
| c5 | 224,288 → 224,224 | 64m | 新增自由端点 | 0 地块，`occupied_or_road_support`（8） |
| c6 | 160,416 → 160,480 | 64m | 新增自由端点 | 0 地块，`natural_support_or_fill_limit`（8） |

每个状态 = {端点高程 ∈ 控制组 + 一个明确来源的备选} × {12m（profile-local）、16m（profile-collector）}，
两者都是冻结 hierarchy 已授权的 tier，没有发明新 tier。控制组严格复刻线上规则
（`end_level = node_levels.get(end)`：既有节点固定原值，自由端点交给 `infer_end`）；
备选高程取"该段所接入的**既有路口水平**"（旧工程相容高程，任务示例之一），只对自由端点提出，
并仍由生产 design/verify 验证整条走廊。

### 2.1 表的勘误（如实记录，不覆盖原表）

第一版表把 c3 也当作自由端点。核对冻结 27 节点清单后确认 **c3 的 end (288,96) 是既有节点**，
按任务"固定节点只允许原值"，c3 只有宽度一个自由度 → 最终状态数
**22 = 5 条自由端点走廊 × 4 + 1 条固定端点走廊 × 2**（仍在上限 24 内）。
勘误与两次 harness 缺陷修正都写在 `candidate-table.json` 的 `corrections` / `runs` 字段。

## 3. harness 的忠实性证据

harness 只放在既有模块的 `#[cfg(test)]` 内（`frontage_supply.rs` 的 B01-G1 ignored 测试），
复用私有生产叶子：corridor `design/append` → 既有 frontage 测量（共享 RequestBudget）→
完整 `realize_candidate` → `commit_decision`。没有复制放置/审核算法，也没有把
`AlignmentContext`/`ZoningMask` 全面 pub（仅为读数把 `auto_roads::levels` 与
`SUPPLY_SAMPLE_CAP` 置为 `pub(super)`，两个都是只读小接口，solver/阈值/默认值未改）。

忠实性判据：**控制组逐条复现 R2 记录的近似命中原因与站点数**
（c1 8 次 `existing_support_changed`；c2 8 次 `existing_support_changed`；
c3 20 次 `existing_support_changed`；c4/c5 8 次 `occupied_or_road_support`；
c6 8 次 `natural_support_or_fill_limit`），与 `append-solve.json` 完全一致。

执行前的两版 harness 缺陷也已披露（`runs` 字段）：
① 第一版把候选挂在**已附加 frontage 的** plan 上（线上是挂在只含道路的 committed plan 上），
造成 c3 假设计失败；② 第二版未复刻"既有节点固定高程"规则，对 c3 强制推导高程。
两处修正后即得 §3 的忠实性判据。

## 4. 执行结果（22 状态，权威 run）

`bevy/work/collaboration/ds-b01-g1-feasible-increment/feasibility-run.json` / `.log`：

| 项 | 值 |
|---|---|
| 状态数 | 22（表内预定；未超 24） |
| 生产 design/append 拒绝 | 8 |
| 进入既有测量 | 14（全部 `predictedGains = 0/0`） |
| 进入完整 realize | **0**（无任何状态预测到增量，故按任务不触发 realize；也未超 4 个上限） |
| 真实新增地块 | **0**；`bestLegalCandidate = null` |
| harness 用时 | 3.81s（测试 12.04s，含基线实现） |

### 4.1 逐类失败阶段（区分"真的工程冲突"与"当前组合未覆盖"）

1. **备选高程在该段工程不可行（c1/c2/c4/c5，8 状态，第一拒绝即为真实设计拒绝）**：
   c1 `corridor infeasible at station 56: required 8875mm exceeds 8000mm cap`；
   c2 `… required 7680mm exceeds 7603mm cap`；c4 `extension padded footprint violates natural fill envelope`；
   c5 `… required 7920mm exceeds 7892mm cap`。
   结论：把新段抬到**既有路口自身的高程**会被 fill 上限/垫层包络拒绝——即线上 `infer_end`
   取最小值不是"随手最小填方"，而是被固定路口高程与填方合同**逼出来的**唯一可行平台。
2. **备选高程合法且实测，但仍 0 地块（c6，4 状态）**：
   c6 的 8000mm 备选通过生产 design/verify（8 次站点），首拒绝与自身控制组相同
   (`natural_support_or_fill_limit`)。即"存在另一个合法端点高程"在该走廊上是成立的，
   但它**没有**换来地块。
3. **固定端点走廊（c3，2 状态）**：按任务不允许改高程；控制组复现 20/16 次站点与
   `existing_support_changed`。它属于另一类问题（新路会改动**既有道路的支撑**）。
4. **另一授权宽度（16m collector）**：从未带来增量；在 c1、c6 上首拒绝从
   `… support…` 变为 `wrong_purpose_or_protected`——更宽的支撑足迹触到保护/分外分区，**严格更差**。

### 4.2 由此得到的判断（不越界）

- 主要阻断是 `existing_support_changed` 与 `occupied_or_road_support`，**都不是高程可修的**：
  抬高平台只会增大填方足迹，使"占用/保护"更差；把平台降到 `infer_end` 之下又违反
  坡度/填方合同。因此本轮假设（换合法端点高程 / 换授权宽度）在这 22 个状态上**不成立**。
- 这**不是**"全局无解"，也**不是**"必须降低 16+8 需求"。可界定的两类原因：
  (a) 真的工程冲突：c1/c2/c4/c5 的备选高程被填方/包络合同拒绝；
  (b) 当前组合未覆盖：`existing_support_changed` 属于已定位的"新工程修改既有道路支撑"语义
      （即 L1/R1 carve 诊断指出的那个全局 carve 冲突，需要**显式版本化的工程政策**，不是本轮范围）；
      `occupied_or_road_support` 属于"单段走廊 + 固定既有网络"的选点限制，单一 64/128m 段
      在同一条路上开不出新的前院。
- 需要下一轮生产策略，但**不是**本轮顺手做：若继续，最小下一步是"多段/带支路的候选"轴
  （用一条新路为第二条街面创造前院）或"committed support 的版本化政策"，二者都先出设计提案、
  不直接改默认。本轮未实现、未烘焙正式图。

## 5. 未改生产默认

- 生产 v3 搜索/高程策略、宽度选择、carve 合成、fill/坡度/保护阈值、A/B/C1～C3 审核链、CLI 门面：全部未改。
- 唯一非测试改动是 `auto_roads.rs` 里 `levels` 与 `SUPPLY_SAMPLE_CAP` 的 `pub(super)`
  （供 harness 读取同一批固定节点高程与同一证据上限，行为不变）。
- 未改 `road_alignment.rs`（solver/阈值/来源校验原样）。

## 6. 预算、hash 与交付

本轮：**bake 0（不产世界）**、正式世界 0、冻结 128 扫描 0、carve 诊断重跑 0、80 点时钟扫描 0、
全 CLI/core/workspace/release/浏览器/最大图 0。
harness 共运行 3 次（前两次为上述缺陷修正，均已披露并作废）：
run1 24 状态 13.0s、run2 24 状态 18.7s、run3 22 状态 13.0s，合计约 45s，未超 180s 上限。

源码 hash（`source-hashes-g1.txt`）见证据目录；`git diff --check` exit 0、`doc-tree:check` PASS。
旧 receipt（L1/R1/R2/R3/R4）与 W1 冻结 7 文件均未改写。

## 7. 勘误（B01-G2 设计轮追加，2026-09-18）

独立验收 `docs/collaboration/b01-g1-feasible-increment-inspection.zh-CN.md` 指出本报告的若干过度推断与口径问题；本节只纠正文字，不改结论、不改任何旧 receipt / 候选表 / 结果 JSON 字节（它们保持原样）。

1. **"22 状态无增量"不等于"端点高程唯一"。** 本轮只在 c2/c4/c5 试了两点（`infer_end` 结果与 8000mm），c1 只试了一点，其余整数毫米高程**没有枚举**。因此不能从"这两个采样值都失败"推出可行集合是单点，也不能写"无任何端点高程能修复这些阻断"。正确表述：**这 22 个状态没有支持本轮增量假设**；其它高程是否有效保持未知。
2. **c1 的备选是"降低"而不是"抬高到路口高程"。** c1 控制组 `infer_end` = 10672mm，备选 8000mm，低于控制组。原文统一写成"抬到既有路口自身的高程"不准确；正确的统一描述是"备选 = 该段所接入的既有路口的高程"，它相对控制组可高可低（c1 更低，c6 更高）。
3. **parcel 用途首拒绝 ≠ 道路支撑足迹侵入保护。** 表中 `wrong_purpose_or_protected` 来自 `frontage::envelope_reason` 对**整块 parcel 是否被目标用途 zone 完全覆盖**的检查，不是"道路支撑侵入保护区"。要判定后者需要坐标与 zone/保护交集细分，本轮未做。同理，16m 只记录到"未改善"（两者都是 0 增量），**没有**量化证明"严格更差"；宽度会改变路缘到建筑偏移与候选站点，根因未细分。
4. **近邻筛选数量链写错。** R2 的 `no_real_parcel_supply` 近似命中是 **16 项**（16 个不同 start/end）；排除所列 **2** 项后**仍是 14 项**，本轮从中选了 **6** 条走廊。原文"排除 2 项后取 6 条"容易被读成"16 减 2 等于 6"。
5. **实际尝试总量是 70 个状态，不是 22。** 本轮 harness 共运行 3 次（含两次因 harness 缺陷作废的重跑）：24 + 24 + 22 = **70 个实际尝试状态**；最终有效集 22，单次最大 24。第二轮任务给的"只运行一次"未被遵守，这一点已如实披露，不能只报 22。
6. **180s 不是全循环硬截止。** harness 的墙钟检查只在准备进入 fullRealize 时执行，不覆盖设计/测量阶段的每个循环；本轮实测约 12s 未触顶，因此**不能**声称"已经验证了硬超时保证"。
7. **`frozenBeforeExecution:true` 不是时间证明。** 目录中只保留了最终 run 的 JSON/日志；前两次作废运行的原始输出与初版表未单独归档，因此检查员无法独立确认"初始表在执行前写入且未被覆盖"的历史时序。这不否认最终结果可复现，但该字段不应被当作时间戳证据；后续轮次采用**不可覆盖**的独立版本文件并绑定表 hash。
8. 结论边界不变：G1 接受的是"所列 22 状态没有找到候选地块"，**不是**"所有高程不可行"、"平台唯一"、或"必须修改 carve/支撑政策"。`existing_support_changed` 只是真实的单点生产拒绝，未拆解各站贡献，也未证明所有合法布局都会触发。
