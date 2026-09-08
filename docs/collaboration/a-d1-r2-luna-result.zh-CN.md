# A-D1-R2-L Luna独立回归结果

- 任务：`A-D1-R2-L`
- 日期：2026-09-08（Asia/Shanghai）
- 状态：`ready_for_review`
- 唯一仓库：`G:\difyllmwiki\GlyphWeave`
- 基线：`6020fbe2713e8c79c1b2934b244402dc93e1df14`
- 分支：`refactor/authoritative-baked-audit`

## 写入与隔离

实际代码测试补丁：

- `G:\difyllmwiki\GlyphWeave\scripts\collaboration\check-evidence.test.mjs`
- SHA-256：`3CA07450E2F52CA8FEEA350951E3AA688407974CB97510DAD3076A6B4DE70A63`

本轮未编辑 `G:\difyllmwiki\GlyphWeave\scripts\collaboration\check-evidence.mjs`。
当前候选 checker SHA-256 为
`4609B41B2A8C2E00B10783C496B6B63D5ABA63179274109BE8BA674498A53B64`，
本轮只编辑测试补丁，没有编辑 checker。未修改 `F:\modenmapbox`，未执行任何 Git 写操作。

为保留旧实现红测证据，复制了冻结副本到：

- `G:\difyllmwiki\GlyphWeave\bevy\work\collaboration\luna-r2\r1-baseline\check-evidence.mjs`
- `G:\difyllmwiki\GlyphWeave\bevy\work\collaboration\luna-r2\r1-baseline\check-evidence-r2.test.mjs`
- `G:\difyllmwiki\GlyphWeave\bevy\work\collaboration\luna-r2\r1-baseline\check-evidence-r1.test.mjs`

这些副本均未回写 `bevy/work/collaboration/review/day-r2-baseline/`。

## 测试覆盖

测试补丁使用完整正向 control，每个负例从 control 只改变一个事实并刷新对应
文件 hash，覆盖：

- `glyphweave-world/version=1` 真实 manifest shape、非 JSON、未知版本；
- generation/gate 结构化 `processReceipt`，operation、runtime、manifest、output、
  scenes、exitCode 绑定；
- log-only incomplete、timeout/budget/stillRunning/stopReason 与正常终态冲突；
- 正常授权 gate quality fail 保持 `worldQualityStatus=fail`，而证据状态仍 pass；
- pass/fail 的 `failures` 缺失、null、object、scalar、非字符串、空数组及未授权项；
- world 子目录与等价 dot path；demo authority；
- frontage ledger、scene stat-only、只读输出、字节预算和 CLI 回归；
- sourceSnapshot 空列表、错 hash、缺文件；三份 required report 的 hash 覆盖与错 hash；
- runtime 错 hash；各报告 format/version/revision；unknown gate；
- scene lexical/junction 逃逸；out 覆盖拒绝；目录伪装 report/receipt；oversize JSON；
- 独立补充的 process.stopReason 缺失、canonical sceneId、manifest/world seed、
  generation-report.output、Rust SceneSpec widthM=0.5。

共 75 个 Node 测试（含参数化子用例）；其中 4 个正向 control 明确要求通过，
避免“全部拒绝后声称通过”。

## 命令与证据

### 语法/静态检查

| 命令 | 退出码 | 结果 |
|---|---:|---|
| `node --check scripts/collaboration/check-evidence.test.mjs` | 0 | pass |
| `node --check scripts/collaboration/check-evidence.mjs` | 0 | pass；只读检查 |
| `pnpm doc-tree:check` | 0 | `[PASS] AGENTS.md tree is up to date` |
| `git diff --check -- scripts/collaboration/check-evidence.test.mjs` | 0 | pass |

### 当前候选 checker

命令（工作目录为 `G:\difyllmwiki\GlyphWeave`）：

```text
node --test scripts/collaboration/check-evidence.test.mjs
```

退出码：`0`。结果：`75` tests，`75` pass，`0` fail。

日志：

- `G:\difyllmwiki\GlyphWeave\bevy\work\collaboration\luna-r2\after-terra-small-fix-node-test.log`
- SHA-256：`4F4968AEFD1A01C5C3B7B63425FEADB79E99FDD2502A869B67D0C2391D56D007`

34 项既有 R2 回归、恢复的 R1 安全回归，以及主 Agent 提供的 5 项独立复现
均通过。该次复跑使用的 checker hash 为最终观察到的
`4609B41B2A8C2E00B10783C496B6B63D5ABA63179274109BE8BA674498A53B64`。

### 冻结 R1 checker 副本红测（历史证据）

命令：

```text
node --test bevy/work/collaboration/luna-r2/r1-baseline/check-evidence-r2.test.mjs
```

退出码：`1`；同样为 `34` tests，`16` pass，`18` fail。

日志：

- `G:\difyllmwiki\GlyphWeave\bevy\work\collaboration\luna-r2\r1-baseline-node-test.log`
- SHA-256：`F39BE5B011CF96EBB43ADD6B703BD4715AAE8F15693237B1D5BF3E77B067994F`

这证明 R2 测试不是依赖 Terra 何时修改共享 checker 才产生红测；旧实现对
R2 负例确实有可观测误放行，同时真实 shape control 能通过。

## 新增五项最小复现与 Terra 小修复验收

主 Agent 提供并已读取的独立复现材料：

- `G:\difyllmwiki\GlyphWeave\bevy\work\collaboration\review\a-d1-r2-independent.mjs`
- SHA-256：`3B67DD177C779C1FD83A33B7FFBA2D62FA79E1E7B2F5DABD909F0D432CE78A18`
- `G:\difyllmwiki\GlyphWeave\bevy\work\collaboration\review\r2-independent-Nr5jvr\summary.json`
- SHA-256：`2E2FF2A0EAAD6BA1184B3EBBD8A797E537C00EB676A51E989F242B1C9ABBC9DA`

在 Terra 小修前的 checker hash
`1A958B7442A652E057C145F08D5856EBE9895057A1B450103B3392200F221C3B` 下，control pass；下列单变量错误均曾得到
`status=pass, exitCode=0`：

1. gate process 删除 `stopReason`；
2. manifest 与两个 process 的 sceneId 同为错误值，但不匹配 world canonical
   scene path 的 sceneId；
3. manifest.world.seed 与 world.json.seed 不一致；
4. generation-report.output 指向 world 子目录而非 world 根；
5. Rust SceneSpec 语义下 `widthM=0.5`。

测试补丁对五项均只改一个事实，并刷新 manifest/process/report 的相应绑定 hash。最终观察到的 Terra 小修 checker hash `4609B41B2A8C2E00B10783C496B6B63D5ABA63179274109BE8BA674498A53B64` 已使这五项均按具体 reason 拒绝；75/75 复跑通过。
没有扩展成全 manifest schema validator；没有对 Rust `u32/i32` 默认 0 做臆测，
也没有声称能精确核验超 JS 安全整数范围的 `u64`。
## 待总控验收

1. 总控固定最终 checker hash `4609B41B2A8C2E00B10783C496B6B63D5ABA63179274109BE8BA674498A53B64` 后，串行复跑已完成。
2. 已确认 75 个测试（含参数化子用例）全部通过，包含 4 个 control、
   恢复的 R1 安全回归和五项独立复现的具体 reason。
3. 本 worker 未跑 Rust、全 workspace、最大图、preview/server 或后台任务；未开展
   D2；无运行中句柄。
4. 本轮没有 runtime/manifest 真实运行回执；测试 fixture 是合成的，不能替代
   总控对 authority、源码到二进制来源及真实 r49 过程的外部认证。

## Git声明

本轮未执行 `git add`、`commit`、`push`、`merge`、`rebase`、`reset`、`clean`、
`checkout`、`switch`、`stash` 或其他 Git 写操作；未暂存、未提交、未推送。
仓库中原有 Rust/docs 脏文件均保留未触碰。






