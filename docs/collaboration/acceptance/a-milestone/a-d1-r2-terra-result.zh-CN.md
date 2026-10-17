# A-D1-R2-T（Terra）结果

- **任务 ID：** `A-D1-R2-T`
- **状态：** `ready_for_review`
- **基线 HEAD（只读核对）：** `6020fbe2713e8c79c1b2934b244402dc93e1df14`
- **日期：** 2026-09-08（Asia/Shanghai）

## 本轮冻结合同实现（review-02 F1–F5）

本任务仅修改：

`G:\difyllmwiki\GlyphWeave\scripts\collaboration\check-evidence.mjs`

1. `generation` 与 `gateRun` 现在都必须提供各自独立、hash 绑定的
   `glyphweave.process-benchmark` / version `1` `processReceipt`。`log` 仅为
   补充证据；log-only 收据明确得到 `incomplete`。
2. 使用结构化 `operation` 固定区分 `generate-world` 和
   `world-quality-gate`，不从展示字段 `command` 作子串推断。过程记录会核对
   operation、终态、exitCode、runtime、manifest、output 和 scenes。
3. generation 只接受成功终态和 exit code `0`；gate 只接受完整成功 `0`，或
   正常质量失败 `failed` / `1` 且 `stopReason=null`。timeout、budget、abort、
   crash、running 及内外层终态/exitCode 冲突均拒绝，不能被授权失败基线抵消。
4. receipt manifest 以受限 JSON 读取验证真实输入 `glyphweave-world` /
   version `1`。场景 ID 唯一，尺寸正且有限，`originX` / `originZ` 有限，
   `seedOffset` 使用实际 Rust schema 的缺省值 `0`。两个过程记录均按
   sceneId 比对这些字段；不会读取 scene payload。
5. `receipt.world.path` 改为 realpath 的**精确世界根**身份检查（同路径别名
   合法，子目录不合法）。无论 pass/fail，gate `failures` 均必须是字符串数组；
   pass 必为空，fail 必为非空并逐项比较授权基线。

保持现有导出、CLI、`runReceipt -> {result, exitCode}`、结果字段、只读输出保护
以及 JSON/hash 字节预算。没有新增依赖、签名体系或源码→二进制来源宣称。

## 候选文件与 SHA-256

| 文件 | SHA-256 |
|---|---|
| `G:\difyllmwiki\GlyphWeave\scripts\collaboration\check-evidence.mjs` | `1A958B7442A652E057C145F08D5856EBE9895057A1B450103B3392200F221C3B` |
| `G:\difyllmwiki\GlyphWeave\scripts\collaboration\check-evidence.test.mjs`（Luna 所有，只读核对） | `D981202F3BAF0D4B21798BE9FF0D76C2592DE8D9A10E964DFF1DADC829383E68` |
| `G:\difyllmwiki\GlyphWeave\bevy\work\collaboration\terra-r2\smoke-r2.mjs` | `005DC3E7141C23FD063E55B70CA6BD51D43049DED71405E7B6F5485F1E513C28` |
| `G:\difyllmwiki\GlyphWeave\bevy\work\collaboration\terra-r2\smoke-20260908\summary.json` | `E5225ACE6F0A202CED25A9FCAD4A5C13358DA6428BA7DCDA4EDBF9FC3AC29A1B` |
| `G:\difyllmwiki\GlyphWeave\bevy\work\collaboration\terra-r2\receipt.json` | `4D3342E087B1613DAF03CC6F684A0161E8152EC830290FCBBD7CD82D7F022176` |

## 验证命令与退出码

| 命令 | 退出码 | 结果 |
|---|---:|---|
| `node --check scripts/collaboration/check-evidence.mjs` | 0 | checker 语法通过 |
| `node --check scripts/collaboration/check-evidence.test.mjs` | 0 | Luna 测试语法通过；未编辑该文件 |
| `node --test scripts/collaboration/check-evidence.test.mjs` | 0 | 当前冻结测试 hash 下 **34/34** 通过；包含 4 个完整正向 control |
| `node bevy/work/collaboration/terra-r2/smoke-r2.mjs` | 0 | 完整 control `pass/0`；log-only `incomplete/1`；世界子目录 `fail/1` |
| `pnpm doc-tree:check` | 0 | `AGENTS.md` tree 最新 |
| `git diff --check -- scripts/collaboration/check-evidence.mjs docs/collaboration/a-d1-r2-terra-result.zh-CN.md bevy/work/collaboration/terra-r2` | 0 | 无 whitespace 错误 |

Luna 的 `16 pass / 18 fail` 记录针对其交付时仍为 R1 的 checker hash
`D09A27C416AC0394B5D6637AD35A3C21B4A99AC310966AF953DC9848E20FFF0B`；当前
Terra 候选 hash 如上，已串行复跑 Luna 当前冻结测试并得到 34/34 通过。

小 fixture 的机器可读结果：
`G:\difyllmwiki\GlyphWeave\bevy\work\collaboration\terra-r2\smoke-20260908\summary.json`。

## 未验证项与边界

- 未运行 Rust 编译、全 workspace/Rust 测试、大图生成、preview/new server 或模型调用。
- 真实 r49 仍为 `integration_pending`：未获得可在工具内验证的当前源码→二进制→
  真实执行完整来源回执。本工具只验证材料内部绑定一致性，不认证其来源。
- 未修改 `F:\modenmapbox`；未触碰 Rust 文件、既有脏 docs 或 review 目录。
- `scripts/collaboration/check-evidence.test.mjs` 是 Luna 独占写集；Terra 未编辑或
  更改其预期。
- 无后台进程；未执行 `git add`、`commit`、`push`、`merge`、`rebase`、`reset`、
  `clean`、`checkout`、`switch`、`stash` 或其他 Git 写操作；未暂存、未提交、未推送。

## 收尾修订：review 独立边界（2026-09-08）

总控提供的只读独立单变量复现指出 5 个“绑定收尾”缺口。本次仅在 checker
作小范围补齐，并已读取 `bevy/crates/core/src/worldgen.rs` 的 `WorldSpec`、
`SceneSpec` 与 `WorldIndex` 定义；未扩展为完整 Rust manifest validator。

- 过程 JSON 的 `stopReason` 现必须**显式存在且等于 `null`**；外层 receipt
  仍允许该可选字段省略。
- 从已发布的规范路径 `scenes/<sceneId>/scene.json` 提取 sceneId 并同 manifest
  及两个过程记录比对；无法可靠提取时为 `incomplete`，不读取大型 scene JSON。
- 核对 manifest/world 的紧凑 `seed` 与 `renderMode` 身份。
- `generation-report.output` 现在必须 realpath 精确等于被检查的世界根。
- `SceneSpec.widthM`/`depthM` 按 Rust `u32`、`originX`/`originZ` 按 Rust `i32`
  检查。`seed`/`seedOffset` 是 Rust `u64`，超过 JavaScript 安全整数范围时结果为
  `unsupported`，不假装可精确核对；缺省 `seedOffset=0`、`originX=0`、
  `originZ=0` 保持不变。

### 收尾候选和证据

| 文件 | SHA-256 |
|---|---|
| `G:\difyllmwiki\GlyphWeave\scripts\collaboration\check-evidence.mjs` | `4609B41B2A8C2E00B10783C496B6B63D5ABA63179274109BE8BA674498A53B64` |
| `G:\difyllmwiki\GlyphWeave\scripts\collaboration\check-evidence.test.mjs`（Luna 所有，只读） | `3CA07450E2F52CA8FEEA350951E3AA688407974CB97510DAD3076A6B4DE70A63` |
| `G:\difyllmwiki\GlyphWeave\bevy\work\collaboration\terra-r2\binding-tail-smoke.mjs` | `3B67DD177C779C1FD83A33B7FFBA2D62FA79E1E7B2F5DABD909F0D432CE78A18` |
| `G:\difyllmwiki\GlyphWeave\bevy\work\collaboration\terra-r2\r2-independent-4piWl4\summary.json` | `305924D2816CF77C441AB5396EA2864FE170CDB648C9D1F35EDABAFBAF5341E9` |
| `G:\difyllmwiki\GlyphWeave\bevy\work\collaboration\terra-r2\luna-r2-final-node-test.log` | `4DC784BCFFB3283FF823387CF2FB28B5412F9FFDB5C8D2236BF843A93B509656` |

| 命令 | 退出码 | 结果 |
|---|---:|---|
| `node bevy/work/collaboration/terra-r2/binding-tail-smoke.mjs` | 0 | control `pass/0`，5 个单变量边界均非零且原因特定 |
| `node --test scripts/collaboration/check-evidence.test.mjs` | 0 | Luna 当前冻结测试 **75/75** 通过 |
| `node --check scripts/collaboration/check-evidence.mjs` | 0 | 语法通过 |
| `pnpm doc-tree:check` | 0 | 目录树通过 |
| `git diff --check -- scripts/collaboration/check-evidence.mjs docs/collaboration/a-d1-r2-terra-result.zh-CN.md bevy/work/collaboration/terra-r2` | 0 | 无 whitespace 错误 |

### 边界与偏差记录

- 真实 r49 继续 `integration_pending`；未运行 Rust 编译、全 workspace、最大图、
  preview/server 或模型调用。
- Luna 测试文件未编辑，也未改变任何测试预期。
- **偏差：** 在读取总控的只读独立脚本后，误执行了
  `G:\difyllmwiki\GlyphWeave\bevy\work\collaboration\review\a-d1-r2-independent.mjs`，
  它按自身设计新建了
  `G:\difyllmwiki\GlyphWeave\bevy\work\collaboration\review\r2-independent-SHvGTd\`。
  未改写任何已有 review 文件；为避免第二次违反“review 只读”约束，未删除或再修改
  该目录。后续独立验证只在 Terra work 目录运行。
- 无后台进程；未进行 Git 写操作、暂存、提交或推送。
