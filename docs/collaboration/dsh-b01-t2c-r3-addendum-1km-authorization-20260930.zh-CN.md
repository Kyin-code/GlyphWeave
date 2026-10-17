# R3 附件：最后一次 1km 运行授权 + 成果展示

日期：2026-09-30。下发：检查员/项目经理 Claude。执行：V41F。
授权来源：项目负责人 2026-09-30 明确授权使用保留的最后一次 1km 世界运行额度，并要求运行后给出可直接查看的成果。
上位：`dsh-b01-t2c-r3-inspection-and-task-20260929.zh-CN.md`（SHA-256 `7406e946…914d`）。本附件只追加 R3 之后的步骤，不改变 R3 的 V1–V8 内容与禁止项。

**请严格按任务书完成任务，不得因为其他任务无关问题擅自中断或停止部分步骤；上下文压缩、会话切换、摘要丢失或可修复的驱动问题不是跳过必测步骤的理由。需要中断时先写 checkpoint，恢复后继续。**

---

## 1. 运行闸门（全部满足才能运行）

| 闸门 | 条件 | 谁判定 |
|---|---|---|
| G1 | R3 状态为 `delivered_for_inspection`（全库 0 failed、V1 逐格连续性已恢复、预注册 v2 已冻结） | V41F 自检 |
| G2 | V5 阶段重放显示：至少一条**基础路**产生了 frontage stations 与 candidate site | V41F 自检 |
| G3 | 检查员复核通过，并写出放行文件 `bevy/work/collaboration/ds-b01-t2-production-chain-continuation-20260929/r3/1km-go.json`，其中写明允许的 exe sha256、源码 hash 集、预注册 v2 的 sha256 | 检查员 |

**做完 R3 后停下，等 G3。** 不得自己写 `1km-go.json`。原因：这是最后一次额度，上轮的接缝放宽这类问题只有独立复核能发现，运行后无法重来。

若 G2 不满足（基础路没有带出任何站点），不运行，交 V5 断点诊断；这是省下额度，不是失败。

## 2. 运行规则

1. `run.ps1` 运行前校验：exe sha256、源码 hash 集、预注册 v2 sha256 均与 `1km-go.json` 一致，任何一项不符则拒绝执行。
2. 输入：`r2/1km-candidate/frozen-input/b01-1km-probe.manifest.json`（sha256 `bc43a8f9…`），与 b8 相同，唯一变量是源码。
3. 输出到全新目录 `r3/1km-run/world/`，进程记账（start、end、exit、pid、耗时、峰值内存可测则测）。
4. 然后运行 scale-audit 与 quality-report（这是产品 CLI 的审计运行，本附件授权这两次），**同时记录 exit code 与 JSON status**。
5. **额度计数规则：**
   - exe 未启动就失败（路径错误、hash 校验不通过、脚本语法错误）→ 不消耗额度，修正脚本后可再启动，须在账目中记录；
   - exe 已启动 → 额度即消耗。无论 exit 0 还是非 0，都不得第二次生成。非 0 时交完整 stderr 与诊断；
   - 审计命令失败可修脚本重跑审计（不重新生成世界）。
6. 禁止：参数扫描、改 manifest/zoning/需求、换 seed、第二次生成、512m、6×10km。

## 3. 判定

1. `compare.mjs` 对 b8 与新世界输出 `r3/1km-run/compare.json`。
2. 按预注册 v2 判定为 正增量 / 空增量 / 回归，写 `r3/1km-run/verdict.json`，逐条引用 compare.json 的字段。
3. 同时列出 V5 阶段重放的预测值与实际值的差异，差异大的给出原因。
4. 空增量或回归：不重跑，交断点诊断（链条断在 stations、candidate、site、building 还是 ledger 哪一环，以及对应的具体拒绝原因计数）。

## 4. 成果展示（项目负责人要求可直接查看）

无论判定结果如何都要做；结果不好也照实展示。

### 4.1 数据对比图（必做，不需要浏览器）

由 compare.mjs 或独立脚本从两个世界的权威 JSON 直接生成 SVG（不读预览渲染结果）：

- `r3/1km-run/showcase/b8-vs-candidate.svg`：左右并排，同比例 1000×1000m；
- 画出：道路（基础路与扩展/自动路用不同颜色；带 profile 的基础路用实线，被拒的基础路用虚线并标注拒绝类别）、frontage stations、地块/sites、建筑占地（住宅、商铺、其他用不同颜色）；
- **新世界相对 b8 新增的 sites/建筑高亮显示**；
- 图例、比例尺、指北（X 东 / Z 南）、标题中写明两边的源码版本与世界 revision；
- 另出一张 `r3/1km-run/showcase/candidate-detail.svg`：选新增临街建筑最集中的约 200×200m 区域放大，标出道路 profile 高程（端点与中点 mm）、站点与入口位置。

### 4.2 预览页截图（授权浏览器）

1. 用新构建的 exe 执行 `preview` 分别打开 b8 与新世界的 `preview/`（本地端口，用完关闭进程）。
2. 同一相机位置各截一张全图、一张与 4.1 局部图相同区域的近景，共 4 张 PNG，存 `r3/1km-run/showcase/`。
3. 浏览器预算：启动不超过 6 次；截图脚本失败可修复重试，但要记账。截图只作展示，**不作为生成正确性证据**。

### 4.3 中文结果卡

`r3/1km-run/showcase/RESULT.zh-CN.md`，面向项目负责人，一页以内：

- 一句话结论（正增量 / 空增量 / 回归）；
- 对比表：基础路带 profile 数、基础路拒绝数、临街道路数（其中基础路几条）、frontage stations、sites、住宅 实现/需求、商铺 实现/需求、realized buildings、realized/unresolved m²、scale-audit 与 quality-report 的 exit 与 status；
- 嵌入 4.1 两张 SVG 与 4.2 截图的相对路径；
- “还不能说明什么”一节：不代表 B01 通过、不代表 B 或 C 通过、scale/quality 若仍为 fail 要写明原因类别。

## 5. 交付与账目

- `r3/1km-run/`：run 记录、世界目录、审计 JSON 与 stdout/stderr、compare.json、verdict.json、showcase/；
- 更新 `r3/checkpoint.json` 与 `r3/runs.jsonl`（世界 1、CLI 审计次数、preview 次数、浏览器次数分列）；
- 1km 额度状态改为 `consumed`（或按 §2-5 规则说明未消耗）；
- 交付后等待检查员独立核验：检查员会重新运行 compare.mjs、复算审计 JSON 状态、抽查 SVG 与世界数据是否一致。
