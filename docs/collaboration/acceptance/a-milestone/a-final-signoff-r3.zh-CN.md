# A-FINAL-R3 测量闭包小修（待主 Agent 复验）

日期：2026-09-09。状态：`ready_for_review`。本文件不是自行签收，最终结论由主 Agent
检查员复验后决定。

## 1. R3-M1 修复

`scripts/collaboration/check-final-acceptance.mjs` 的 `checkMeasurement` 现完成：

- 固定角色：generation 必须 `generate-world`，gate 必须 `world-quality-gate`。
- measurement/process 的 executable 必须等于 candidate.runtime；manifest 必须等于
  candidate.manifest；output 必须 realpath 等于 candidate.world 所属世界根。
- process 的 `samplesJsonl` 路径/hash 必须绑定本次读取的 samples 文件。
- process format/version、PID、终态、exitCode、stopReason 显式检查。
- generation 仅 `baked/exit0/null`；gate 允许 `completed/exit0/null` 或
  `failed/exit1/null`，后者必须由已绑定的 quality fail 报告支撑。
- 候选摘要与 process 的 elapsed/private/working-set/sampleCount/timeout/maxPrivate/exitCode
  必须完全一致。
- 逐样本重算：行数、PID、UTC、非负数值、时间/CPU 不倒退、elapsed 不超进程总时长、
  private 峰值等于报告、OS working-set 峰值不低于样本最大值。

不重跑生成、gate、浏览器巡检、Rust 编译或线性大测。

## 2. 测试与反例

- 定向：`node --test scripts/collaboration/check-final-acceptance.test.mjs`
  → 36 passed / 0 failed。
- 未改 R2 真实候选复验：
  `bevy/work/collaboration/ds-final-r3/r2-candidate-check-r3.json`
  → closureStatus pass / readiness ready_for_review。
- 检查员三项反例副本：
  `bevy/work/collaboration/ds-final-r3/counterexamples-r3.json`
  - summary-elapsed-1s → `generation.elapsedSeconds-mismatch`，exit1；
  - summary-private-1byte → `generation.sampledPeakPrivateBytes-mismatch`，exit1；
  - sample-wrong-pid（同步刷新 samples/process 绑定）→ `generation.sample-pid`，exit1。

## 3. 披露纠正

### 3.1 最终目录差异

R2 的“唯一差异是 generation-report.json.output”是**替换 preview/app.js 之前**的权威
世界比较结论。补采后的最终目录各自 1781 个文件，其中 1779 个相同，差异为：

1. `generation-report.json` 的 `output` 绝对路径；
2. `preview/app.js` 的观测适配器。

scene/height/surface/terrain/实体/计划等权威世界语义文件相同。A-05 复用建立在权威输入
与相关代码相同，不要求旧/新 observer 字节相同。

### 3.2 实际执行了两轮大图测量

R2 任务授权一次生成和一次 gate，并规定遇权威差异停止。实际第一轮因测量脚本把 manifest
传成绝对路径，使 `rulesDir`/worldRevision 差异；第一轮数据为生成 266.9902462s、
gate 38.1974344s。该证据保留，未被删除或伪称不存在。随后修正脚本副本为传递相对
manifest 并执行第二轮纠正测量。此流程偏差如实登记。

## 4. 保留边界

- 六类原 gate 内容失败保留，worldQualityStatus 仍为 fail。
- Quaternius building 精确来源、`@cloudflare/workerd-windows-64@1.20260701.1`
  许可证元数据仍未解析。
- A-05 linear/indexed 为字节等价复用，本轮未重跑线性大测。
- 未修改 R2/R1/检查员旧文件、原世界、原 runtime、原始测量、旧 observation。
- 未启动其他 agent；未 git add/commit/push/reset/clean/checkout/switch/stash。
- 历史 8099 preview 未触碰；本轮未启动任何 preview。
