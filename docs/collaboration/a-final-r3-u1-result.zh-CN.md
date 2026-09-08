# A-FINAL-R3-U1 结果：补 UTC 相邻顺序检查

日期：2026-09-09（Asia/Shanghai）。状态：`ready_for_review`。本结果等待主 Agent/检查员复验，不自行签署里程碑 A，不推进 B/C。

## 1. 范围与改动

仅修改白名单内的两个文件：

- `scripts/collaboration/check-final-acceptance.mjs`
- `scripts/collaboration/check-final-acceptance.test.mjs`

`checkMeasurement` 的 samples 循环新增 `lastUtc`，要求当前样本 `Date.parse(row.utc)` 不小于上一条已解析 UTC；倒退时给出 `${label}.sample-utc-order`、CLI exit 1。相等 UTC 允许，未改成严格递增，也未把 `process.timestampUtc` 当作开始时间或扩展其他时间认证逻辑。

测试新增 generation/gate 的 UTC 倒退负例和 UTC 相等正例，并继续保留原有正常 UTC 正例。

增量哈希：

| 文件 | bytes | SHA256 |
| --- | ---: | --- |
| `scripts/collaboration/check-final-acceptance.mjs` | 21347 | `1891dde0f5e9b215ba5d45d2d7220ee6239ffb7fd211ea940b8eddbb01e299e8` |
| `scripts/collaboration/check-final-acceptance.test.mjs` | 18470 | `e7c2e68f5abbfd26daa3583fa23aa86482d2d5cf1d526c3a549f0835dcf99b4c` |

## 2. 测试与检查

| 命令 | exit | 结果 | 日志 |
| --- | ---: | --- | --- |
| `node --test scripts/collaboration/check-final-acceptance.test.mjs` | 0 | 40 passed / 0 failed | `bevy/work/collaboration/ds-final-r3-u1/check-final-acceptance-test-u1.log` |
| `node --test scripts/collaboration/*.test.mjs` | 0 | 252 passed / 0 failed | `bevy/work/collaboration/ds-final-r3-u1/all-collaboration-tests-u1.log` |
| 两个 mjs 的 `node --check` | 0 / 0 | 通过 | `bevy/work/collaboration/ds-final-r3-u1/node-check-u1.log` |
| `corepack pnpm doc-tree:check` | 0 | PASS | `bevy/work/collaboration/ds-final-r3-u1/doc-tree-u1.log` |
| `git diff --check` | 0 | 仅有既有 CRLF 提示，无 whitespace error | `bevy/work/collaboration/ds-final-r3-u1/git-diff-check-u1.log` |

## 3. 原 R3 候选复验

原候选保持只读，路径与 SHA256 未变：

- `G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-final-r3/final-acceptance-r3.json`
- 21382 bytes；`d5b1aa9fb2725a96692b99045bc0423a85b227661418416c58dc02cc3379d433`

修复后的 checker 直接读取原文件时，唯一失败是候选自带的脚本证据绑定 `R3.checker` 仍记录旧 checker 的 21185 bytes / `dcc56c...`，实际修复后为 21347 bytes / `1891dd...`；退出码 1，检查 ID 为 `evidence.R3.checker`。这是脚本增量的自绑定预期变化，不是世界、采样、测量或 runtime 内容失败。直接结果保存在 `r3-candidate-check-u1.json`。

为完成只读候选的增量复验，U1 新增 `rebind-script-bindings-u1.mjs`，只把 `evidence[33/34]` 与顶层 `checker`/`checkerTest` 的 bytes/SHA256 更新为本轮脚本，其他字段不改，原始 R3 候选不覆盖。重绑副本：

- `bevy/work/collaboration/ds-final-r3-u1/final-acceptance-r3-u1-rebound.json`
- 21382 bytes；`d7689f9c9ebd6b6a34c9952cc741a165f26946f4016e59202bdb965d2dd907c9`

修复后 checker 复验该增量绑定副本：exit 0，`closureStatus=pass`、`readiness=ready_for_review`，38 evidence / 18 logs。结果保存在 `r3-candidate-rebound-check-u1.json`。

## 4. 检查员 UTC 反例复验

检查员原文件只读且未改：

- `G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-r3-20260909/backward-utc-rebound-candidate.json`
- 21372 bytes；`33bf9f4bd7f43b41e8870cdab72cb9945900c8a811e14f3e77e8ccfd1df5b6cf`

直接读取原文件同样先被其旧脚本自绑定 `evidence.R3.checker` 挡住。用同一最小重绑工具生成仅更新脚本绑定的 `backward-utc-rebound-u1.json` 后，修复后的 checker exit 1，唯一失败为：

```text
id=generation.sample-utc-order
reason=generation sample UTC moves backwards at row 1
```

结果保存在 `backward-utc-rebound-check-u1.json`；该反例的第二条样本 UTC 已早于第一条，elapsed/CPU/PID 保持正常。

## 5. 约束与边界

本轮未运行 `generate-world`、`world-quality-gate`、Cargo 编译/测试、浏览器巡检、linear 大测或预览服务；未修改原始世界、采样、runtime、旧候选、旧回执或检查员证据；未执行 Git 写操作；未启动其他 agent。原历史 8099 preview 未触碰。

本轮只交付增量脚本、测试、复验证据和回执。里程碑 A 仍待主 Agent/检查员复验，状态仅 `ready_for_review`。
