# B01-T2 R4 裁定 + R5 任务（给 V41F）：执行最后一次 1km，交成果，然后修 B1

日期：2026-09-30。检查员/项目经理：Claude。执行：V41F。
上位：`dsh-b01-t2c-r4-inspection-and-task-20260930.zh-CN.md`（`1b74dff2…6b`）、`dsh-b01-t2c-r3-addendum-1km-authorization-20260930.zh-CN.md`（`a81bf972…e15b`）。未改动的禁止项全部有效。

**请严格按任务书完成任务，不得因为其他任务无关问题擅自中断或停止部分步骤；上下文压缩、会话切换、摘要丢失或可修复的驱动问题不是跳过必测步骤的理由。需要中断时先写 checkpoint，恢复后继续。**

---

## 第一部分：R4 裁定

- R-1 根因定位 **接受**：`road_network.rs` 生成 RoadsideSite 时把支撑高度写成平的 `level`（`support_level_mm: None`、`frontage_road_point: None`），三个下游读者继承同一假设。
- R-2 回退、不留半成品 **接受**，做法正确。检查员补充判断：分级套件报错 `design error 1.2m, support sample (142,107,4400,Some(4450))` 说明路边站点的垫层范围与带坡道路的路基/路肩在同一批格子上争高度，**不只是“再改一个读取点”**，可能需要调整站点垫层与路基的衔接方式。见 B 部分。
- R-3 延伸重放 **接受**，两个 notObserved（ledger、开发审计）如实标注。
- R-5 **接受**。检查员独立复算：全集 115 个文件实时 hash 0 不一致、0 漏登记；exe `5dc0515c…`；预注册 v2 `ab6d6b81…`。
- 发现 1 个 run.ps1 缺陷：它读取 `r3/cli-build-binding.json`（R3 的 exe `25ad5cac…`），而当前 exe 是 R4 的 `5dc0515c…`，会导致拒绝执行。见 A-1。

### 放行决定

项目负责人建议现在运行，检查员同意。理由：离线重放看不到开发审计与用途 ledger，只有真实世界能看到；B1 风险已在运行前写入放行文件，结果不会被事后改口。

放行文件：`bevy/work/collaboration/ds-b01-t2-production-chain-continuation-20260929/r3/1km-go.json`（放在 run.ps1 读取的位置）。

---

## A 部分：1km 运行与成果（先做）

**A 部分完成前，禁止修改任何被 `r4/full-source-hash-set.json` 覆盖的文件，也不得重新构建 exe。** 放行文件锁定的就是当前源码与 exe。

### A-1 修 run.ps1（不改源码）

1. `$binding` 改为 `r4/cli-build-binding.json`。
2. 读取并校验 `1km-go.json` 的 `sourceSetSha256`、`exeSha256`、`preregistrationSha256`、`manifestSha256`。
3. 峰值内存：当前在进程刚启动时读取 `PeakWorkingSet64`，数值没有意义。改为在等待期间轮询（例如每 200ms）取最大值，或标 `not_instrumented`，不能记录无意义数值。
4. 记录 run.ps1 修改前后的 hash。

### A-2 运行

按 R3 授权附件 §2 执行：一次生成 + scale-audit + quality-report，同时记录 exit 与 JSON status。额度计数规则不变：exe 未启动的失败不算消耗；exe 一旦启动即消耗，无论结果都不得第二次生成。输出目录沿用 run.ps1 的 `r3/1km-run/`。

### A-3 判定

按 R3 授权附件 §3：compare.json、verdict.json（严格按预注册 v2）、V5/R-3 预测值与实际值对照。

额外要求：
- 列出新世界所有 roadside sites 的院落实现状态，与 b8 对照，并标注哪些未实现是 B1 造成的（依据：站点所邻基础路带 profile，且站点声明高度 ≠ 该处路面 profile 高度）；
- 开发审计与 ledger 的每一个失败原因，标注 b8 是否已有；
- 如果判为回归，写明回归的量有多少来自 B1；不得改判。

### A-4 成果展示（直接交给项目负责人）

按 R3 授权附件 §4 执行：对比 SVG（b8 与新世界并排，新增的临街建筑高亮，被 B1 影响的路边院落用醒目颜色标出）、局部放大 SVG、4 张预览截图、中文结果卡。浏览器启动不超过 6 次。结果卡里要有一句话告诉项目负责人：这次多出了什么、坏了什么、坏的原因是否已知、下一步怎么修。

**A 部分完成后，把成果展示直接交给项目负责人，然后继续 B 部分，不必等待。**

---

## B 部分：修 B1（A 部分完成后做）

### B-1 定位垫层冲突

1. 以分级套件的失败为入口，逐格解释 (142,107) 这类冲突：该格属于哪个站点的垫层范围、哪条道路的路面/路肩/graded band、各自期望的高程、最终地形由谁决定。写 `r5/b1-pad-conflict.json`。
2. 找出生成站点垫层/院落砍削的生产代码（文件:行），以及决定路基与站点重叠优先级的代码。

### B-2 修复（声明与砍削同一次落地）

目标：带 profile 基础路旁的路边站点，声明高度、垫层砍削、院落实现、入口对接四者一致，且不低于 T2 前的院落实现水平。

允许：
- RoadsideSite 声明改为读取所邻道路权威 profile 在该站点临街点的高程，写 `support_level_mm: Some(..)` 与 `frontage_road_point: Some(..)`，与 `frontage.rs:310-312` 的临街站点做法一致；
- 站点垫层砍削读取同一声明；
- 站点与路基重叠处：站点垫层应退出道路的路面 + 路肩 + graded band 范围，或按统一规则让二者在边界上连续；具体规则由你设计，须在报告中写明，并有正负 fixture；
- 下游读者 `frontage_supply.rs:819`、`:2615`、`auto_roads.rs:5051`、`:8101` 同步：`unwrap_or_default()` 把 None 静默读成 0 的写法必须改为显式处理（None 时按旧的 `support_level_m` 或具名报错），不得继续静默为 0；
- 本任务授权修改上述文件及 `settlement/landcover.rs`、`settlement/development.rs` 中与路边站点院落/垫层相关的生产代码，逐处登记。

禁止：压平或局部压平道路 profile；删除、移动、缩小路边站点；放宽支撑、坡度、填方、院落面积、入口规则或审计精度；把未实现改名；改 zoning/需求/尺寸/比例。

如果站点必须挪位或缩小才能与路基兼容：停在该项，给出具体站点、所需位移/尺寸与冲突数值，交检查员裁定。

### B-3 验证

- W1 冻结基线：院落未实现回到 51 m² 量级（R-2 实验值），发展审计不再因路边院落失败；
- 分级套件全绿（不改断言）；
- 新 fixture：非平坦地形 + 带 profile 基础路 + 路边站点，声明/砍削/院落/入口一致；生产代码突变（声明改回平 level）使其变红；
- 1km 冻结输入离线重放（R-3 全链）：院落与地表实现相对 A 部分的真实 1km 世界应改善，逐项对照（真实世界数据作基准，重放是预测）；
- 全库 0 failed、15 模块、ignored 冻结套件、确定性、黄金快照（合法变化只能用 `GLYPHWEAVE_UPDATE_GOLDEN=1` 更新并列差异）；
- CLI 重建，全集绑定（新的 full-source-hash-set，不覆盖 r4 的）。

### B-4 下一次 1km

B 部分完成后**不运行世界**。修复后的真实验证需要项目负责人另外授予 1km 额度；在报告中给出申请：固定输入（同一 manifest）、唯一变量（B 部分源码）、预期指标（路边院落恢复、realizedM2 不低于 b8、临街数量不低于 A 部分）。

---

## 预算与硬边界

- 世界生成：**1 次**（A-2，最后一次 1km）；审计：scale-audit、quality-report 各 1 次，审计命令本身失败可只重跑审计；preview 与浏览器仅用于 A-4（浏览器 ≤ 6 次）。
- cargo：软计划 80 次（B 部分），达 60 次汇报。
- 512m 已耗尽；6×10km 禁止；B 部分完成后世界 0。
- 不改 `road_alignment.rs`、`road_profile.rs`；不做 Git 写；不改 `F:\modenmapbox`；不回写历史证据（r3/1km-run 生成后即冻结，B 部分不得改动它）。

## 交付

- A 部分：`r3/1km-run/`（run-summary、run-accounting、世界、审计 JSON、compare.json、verdict.json、showcase/）；成果直接交项目负责人。
- B 部分：`r5/report.json`（按报告协议，逐条回应 A-1～A-4、B-1～B-4）、`r5/checkpoint.json`、`r5/write-set.json`、`r5/runs.jsonl`、`r5/b1-pad-conflict.json`、`r5/cli-build-binding.json`、`r5/full-source-hash-set.json`、完整源码差异。
- 状态：`delivered_for_inspection` 或 `partial + 具名阻塞`。不得宣称 B01 通过。
