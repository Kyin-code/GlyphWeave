# 验收与检验文档归档（acceptance/）

整理日期：2026-10-18。本目录集中存放 A/B 里程碑的**测试、检验、复核、裁定、签收**类文档。
需求、合同、任务书、设计、交接、事故记录仍在上一级 `docs/collaboration/`。

本目录只做**位置整理**：文件内容一个字没改，也没有做任何 git add / commit。

## 1. 归档规则

移入本目录 = 满足下列任一条：

1. `a-*`：A 底座系列的测试/复核/签收（`a-t1`～`a-t3`、`a-d1`～`a-d3`、`a-final-*`、`a-main-acceptance`）；
2. 文件名含 `result` / `inspection` / `review` / `verdict` / `acceptance` / `closeout` / `signoff` / `accounting` 的 B 系列逐轮检验与裁定；
3. `g33-*` 闸门定义与预算、`r2-w1-closeout-*`。

留在上一级不动的：

| 类别 | 例子 |
|---|---|
| 需求与合同 | `acceptance-contract`、`milestone-b-plan`、`milestone-b-acceptance`、`abc-goals-realignment-and-roadmap` |
| 任务书 / prompt | `dsh-*`、`ds-v41*`、`deepseek-*`、`day-*` |
| 设计 / 提案 / 规格（即使带 G 编号） | `b01-g2-design`、`b01-g10-connection-design`、`b01-g7-offline-next-increment-proposal`、`b01-g10-r1-g1-execution-spec`、`b01-g10-r1-g1b-geometry-card` |
| 交接 / 事故 / 外部意见 | `project-handoff-*`、`handover-to-next-agent-*`、`handoff-g32-g33`、`status.zh-CN.md`、`model-routing-incident`、`external-review-*` |
| 里程碑级报告 | `milestone-a-completion-report`、`milestone-a-publication`、`milestone-1-continuation` |

> 注意：`b01-g*-design` / `-proposal` / `-spec` 是**任务侧**文档，留在上一级与对应任务书相邻；
> 因此同一个 G 轮的「设计稿」与「检验报告」现在分处两个目录，按轮号（如 `g19`、`g30`）检索即可配对。

## 2. 目录结构

| 目录 | 内容 | 文件数 |
|---|---|---|
| `a-milestone/` | A 底座：T1/T2/T3、D1/D2/D3、FINAL-R1～R3、最终签收 | 29 |
| `a-milestone/evidence/` | A 的机器可读证据（candidate/runtime/manifest/quality/measurements JSON、日志文本） | 24 |
| `b01-supply/` | B-01 道路供给链：`b01-l1-supply*`、`b01-supply-r1`～`r4` 的检验与结果 | 10 |
| `b01-rounds/` | B-01 各轮（G1～G34）的 inspection / review / verdict / result，**本目录最大宗** | 117 |
| `b01-t2/` | T2「基础路 profile」独立检查：`b01-t2-opt-b-independent-review`、`b01-mainline-independent-inspection`、`b01-result` | 3 |
| `b02-b05/` | B-02～B-05 批次结果 + `b-review-r1`～`r6` 六轮复核 + `b-first-inspection` | 23 |
| `gates/` | G33 闸门文件（`g33-scope`、`g33-stop-rules`、`g33-baseline-binding.json`、`g33-budget-proposal.json`）+ `r2-w1-closeout-*` | 6 |
| 合计 | | **212** |

各目录当前结论（一句话速查）：

- `a-milestone/`：A 于 2026-09-09 `milestone-A-accepted`，但 `worldQualityStatus = fail`，六条内容例外保留 → 看 `a-main-acceptance-20260909.zh-CN.md`。
- `b01-rounds/`：G1～G34 逐轮收口，绝大多数裁定为 `accepted_scoped_with_process_exceptions`；G28～G34 主要是只读 3D 消费者 / 小地图 / 相机 / 读图，**不构成 B 通过**。
- `b01-supply/`：供给链（L1 与 supply r1～r4）结果，含 area/ledger 口径证据。
- `b01-t2/`：T2 独立检查只跑了一次测试编译，`exit 101`、5 处 `profile` 缺字段 → 当时未通过。
- `b02-b05/`：B 批次结果与六轮复核；B 整体仍 `partial`。
- `gates/`：G33 闸门基线/预算/停机规则，供回溯「当时额度是多少」。

## 3. 怎么按旧路径找文件

`moved-files.json` 是机器可读映射（212 条，含 `bytes` 与 `sha256`，用于校验/回滚）：

```json
{ "oldPath": "docs/collaboration/a-t1-result.zh-CN.md",
  "newPath": "docs/collaboration/acceptance/a-milestone/a-t1-result.zh-CN.md",
  "group": "a-milestone", "bytes": 5987, "sha256": "..." }
```

PowerShell 反查：

```powershell
$m = Get-Content docs/collaboration/acceptance/moved-files.json -Raw | ConvertFrom-Json
$m | Where-Object { $_.oldPath -like '*a-t1*' } | Format-Table oldPath, newPath
```

## 4. 已知副作用（重要）

1. **旧文档里的绝对路径引用会失效**。历史文档大量写有
   `G:/difyllmwiki/GlyphWeave/docs/collaboration/a-*.md` 这类路径。按本项目「不倒写旧证据」的纪律，
   **没有**去改这些历史文档；要定位文件请用 `moved-files.json`。
2. **源码里的注释路径失效**：`bevy/crates/core/src/preview_stream_audit.rs` 顶部注释引用
   `docs/collaboration/a-d2-parity-contract.zh-CN.md`（现位于 `acceptance/a-milestone/`）。
   本次**未改动任何源码**，因为该项目用「源码全集指纹」做轮次绑定（例如 T-0013 验收项 AC-8 要求
   before 指纹为 `62dc5407…`），改注释会让指纹漂移。该注释留待下一次正式任务顺手修正。
3. **Git 视图**：本次只用文件移动，未执行 git add/commit。工作树里表现为
   「53 个已跟踪文件显示为删除 + 135 个原本未跟踪文件变成新路径未跟踪」；
   提交时 Git 会自动识别为重命名（内容未变，`sha256` 可核对）。
4. **未移动的部分**：`bevy/work/collaboration/` 下的逐轮证据目录（世界、截图、日志、`rk/T-00xx`）
   体量大且被 `.gitignore` 管理，不属于文档整理范围，保持原位。

## 5. 回滚

按 `moved-files.json` 反向 `Move-Item` 即可完全恢复；`sha256` 可用于确认内容未被改动。
需要恢复成「旧路径 + 新目录」双份的话，也可以用它做硬链接或副本，但会产生重复文件。

## 6. 相关入口

- 里程碑与需求总览：`../requirements-and-progress-master.zh-CN.md`（19 个域、139 条需求条目、进度与当前卡点）
- 追溯式需求：`../abc-goals-realignment-and-roadmap-20260929.zh-CN.md`（R01～R16）、
  `../acceptance-contract.zh-CN.md`（A-01～A-10）、`../milestone-b-acceptance.zh-CN.md`（B-G1～B-G8）、
  `../../milestone-c-agent-task.zh-CN.md`（C-N1～C-G1）
- 当前状态与交接：`../project-handoff-abc-b-mainline-20260929.zh-CN.md`、`../status.zh-CN.md`
- 最新一轮（未执行）：`bevy/work/collaboration/rk/T-0013/`
