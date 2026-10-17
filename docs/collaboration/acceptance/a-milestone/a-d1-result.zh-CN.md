# A-D1 结果：只读证据校验器（含 R1 修订）

日期：2026-09-08（Asia/Shanghai，本机）
taskId：**A-D1**（初版）→ **A-D1-R1**（第一轮验收修订）
状态：**ready_for_review**（供主 Agent 复验；未暂存/未提交/未推送）
基线 commit：`6020fbe2713e8c79c1b2934b244402dc93e1df14`
分支：`refactor/authoritative-baked-audit`
工作目录：`G:\difyllmwiki\GlyphWeave`

## 0. 基线核对（修订开始时）

- `git rev-parse HEAD` = `6020fbe2713e8c79c1b2934b244402dc93e1df14`，与给定 BASE_COMMIT 一致。
- `git status --short`：脏文件全部在 `bevy/crates/core|cli`（Rust）及 `docs/status.zh-CN.md`、`docs/dual-agent-collaboration.zh-CN.md`（他人成果）；另有他人新增 `docs/collaboration/a-t1-result.zh-CN.md` 与总控评审文档。以上均与 A-D1 白名单不相交，未改动。
- 未进入/未修改 `F:\modenmapbox`。

## 1. 交付物（白名单内文件；scripts/collaboration 目录为新增）

| 文件 | 类型 |
|---|---|
| `scripts/collaboration/check-evidence.mjs` | 新增（实现，R1 重写） |
| `scripts/collaboration/check-evidence.test.mjs` | 新增（测试，R1 重写为 42 项） |
| `docs/collaboration/a-d1-result.zh-CN.md` | 新增（本结果文档） |
| `bevy/work/collaboration/deepseek/*` | R1 新证据（保留初版输出） |

证据目录内容：`test-r1.log`、`adversarial-r1.log`、`check-r49-frontage-maximum.r1.json`、`check-r48-frontage-maximum-final.r1.json`，以及初版的 receipt/check/test-run.log。`bevy/work/` 在 git 忽略区内，属本地证据。

## 2. 本轮修正对应问题

主 Agent 评审 `docs/collaboration/a-d1-review-01.zh-CN.md` 判定 needs_changes，并给 11 类反例。修订逐条对应：

### R1 来源与运行证据绑定
- receipt 扩展为：`manifest{path,sha256}`、`sourceSnapshot.files[{path,sha256}]`（文件级、逐一 hash 校验）、`reports[]` 必须**绑定实际读取的三个必需报告**（world.json / generation-report.json / world-quality-gate.json），缺绑定 → incomplete。
- `runtime` hash 一致不再被单独视为“本次运行使用了它”；增加**两个独立运行回执** `generation` 与 `gateRun`（operation/command、exitCode、status、log/processReceipt 哈希绑定）。generation 必须 exitCode=0；gateRun 允许非零退出（gate 判负属正常终态），但必须有 log/processReceipt 绑定证据且非 running。
- 说明：合法 pass 需要 manifest + runtime + sourceSnapshot.files + 两个回执 + 三方报告 hash 绑定 + main-agent 授权。真实旧产物无法从现存目录获得可信源码快照/独立 gate 回执 → **r49 真实检查维持 integration_pending**，不把 demo 改 authority 过关。

### R2 授权语义
- 仅 `authority="main-agent-acceptance"` 允许 pass；demo / worker-draft / none / unknown / 缺失 → `authority.trusted` incomplete，整体非零。工具外声明信任边界：pass 表示“证据完整一致且 receipt 声称主 Agent 验收”，并不认证 receipt 真伪。

### R3 报告形状 / 版本 / 矛盾状态
- 每份必需报告：root 必须是 object；`format` 必须与文件名匹配（缺失 → fail）；`version` 必须为整数 1（缺失 → incomplete；其它 → **unsupported**）；`receipt.receiptFormat/Version` 同规则。
- 每份报告必须有独立 revision（world.json.revision、gen/gate 的 worldRevision），三者缺一 → incomplete，三者不等 → fail（不再“剩一份就算一致”）。
- 场景：world.scenes 必须非空且长度 == generation-report.sceneCount == claimedScope.sceneCount。
- gate：`status` 仅识别 pass/fail（其它 → incomplete、quality=unknown）；`pass + failures 非空` 是矛盾 → fail，即使 failures 全在允许列表（授权不能抵消矛盾）；`fail + 空/缺 failures` → incomplete；failures 必须纯字符串数组，非法项不静默丢弃。
- generation：running/timeout/非 0 exit/未记录 exit → fail 或 incomplete；未知状态（如 banana）且报告无终态 → incomplete。

### R4 frontage 对接真实合同
- 读取 `quality-report.scenes[i].frontageAudit` 的真实字段 `requested/planned/realizedWithAccess/unfilled`（按 sceneId 定位；可经 `auditPath` 指向自定义审计，否则用 `<world>/quality-report.json`）。
- 正确关系：`requested = planned + unfilled`（unfilled=规划缺额）；`0 <= realizedWithAccess <= planned`；`planned - realizedWithAccess` = 实现/入口缺额。**删除了旧版错误的 `realized + unfilled == requested` 公式**。规划缺额与实现缺额分开输出。
- 审计须带 worldRevision 且与报告 revision 一致（不一致 → fail；无 → incomplete）；几何仍恒标 unverified，不凭计数认定几何正确。quality-report 若同时出现在 reports[] 则一并 hash 绑定。

### P2 安全/鲁棒
- scene 路径：词法 + realpath 双层 containment，拒绝绝对/`..` 逃逸及 symlink/junction 逃逸；不做仅小写字符串比较。
- `--out`：不得位于 world 只读树内（主流程前置检查 + writeResult 词法/realpath 双保险），使用 `wx` 排他创建实现原子拒覆盖（修掉先 stat 后写的 TOCTOU）。
- 读/哈希遇到目录、权限、读错误均返回结构化 status（`not_a_file/read_error/too_large/empty/invalid`），不产生未捕获崩溃。
- 字节预算必须是有限正整数；`runReceipt` 缺省用安全默认（64 MiB JSON / 1 GiB hash），超大 JSON 拒绝整读。

## 3. CLI 与 receipt schema（v1）

```
node scripts/collaboration/check-evidence.mjs \
  --world ABS_WORLD_DIR --receipt ABS_RECEIPT_PATH --out ABS_NEW_OUTPUT_PATH \
  [--max-json-bytes N] [--max-hash-bytes N]
```
退出码：0 = pass；1 = incomplete/fail/unsupported；2 = 用法错误。完整 schema 注释于 `check-evidence.mjs` 头部（receiptVersion=1 支持，其余 unsupported；manifest/sourceSnapshot.files/generation/gateRun 见上）。

输出 JSON 字段不变地保留 `format/version/taskId/authority/status/worldQualityStatus/checks[]/missingEvidence/unexpectedFailures/resolvedFailures/unverified/summary/exitCode`，并新增 `out`。`status` 与 `worldQualityStatus` 严格分离。

## 4. 命令、退出码与证据

| 命令 | 退出码 | 证据 |
|---|---|---|
| `node --test scripts/collaboration/check-evidence.test.mjs` | 0（42 pass / 0 fail） | `bevy/work/collaboration/deepseek/test-r1.log` |
| `node --check scripts/collaboration/check-evidence.mjs` | 0 | — |
| `node --check scripts/collaboration/check-evidence.test.mjs` | 0 | — |
| 总控反例 `node bevy/work/collaboration/review/a-d1-adversarial.mjs` | 0（脚本自身正常） | `adversarial-r1.log` + review 目录 summary.json |
| `node scripts/generate-tree.mjs --check`（doc-tree:check 本体） | 0 [PASS] | — |
| `git diff --check` | 0 | — |
| 真实 r49 紧凑核对（demo receipt，新 checker） | 1（预期） | `check-r49-frontage-maximum.r1.json` |
| 真实 r48 不完整目录拒绝 | 1（预期） | `check-r48-frontage-maximum-final.r1.json` |

pnpm 不在 PATH：`pnpm doc-tree:check` 以 `node scripts/generate-tree.mjs --check` 等价执行并已通过（corepack pnpm 亦曾验证 [PASS]）。

## 5. 11 类反例结果（新 checker 实测，均非零）

| 反例 | 结果 | exitCode | worldQualityStatus |
|---|---|---|---|
| authority=demo | incomplete | 1 | pass |
| sourceSnapshot={} | incomplete | 1 | pass |
| manifest 指向不存在文件+fake hash | fail | 1 | pass |
| reports 只 hash runtime 不覆盖必需三报告 | incomplete | 1 | pass |
| world.scenes=[] 仍声称 sceneCount=1 | fail | 1 | pass |
| world/gen/gate/receipt version=999 | unsupported | 1 | pass |
| gate.status=banana | incomplete | 1 | unknown |
| gate.status=pass 但 failures 非空且已“允许” | fail（矛盾，授权不能抵消） | 1 | pass |
| 删除 format + gen/gate revision | fail | 1 | pass |
| scene 路径逃出 world 目录 | fail | 1 | pass |
| generation.status=banana 且报告无 status | incomplete | 1 | pass |

另：总控 base `original-positive`（无 manifest / sourceSnapshot.files / gateRun / generation log）按新标准为 incomplete——该 fixture 本身不是可信收据，符合评审要求“不能以 fixture 当批准依据”。以上结果不是对条件文件名的硬编码，而是通用规则的自然结果。

## 6. 测试覆盖（42 项，全部通过）

- 正向：完整可信格式（manifest+runtime+sourceSnapshot.files+generation/gateRun 回执+三报告 hash 绑定+main-agent 授权）→ pass/0；授权 gate fail 场景 → status=pass 而 worldQualityStatus 保持 fail；gateRun success 与 gate fail 矛盾 → fail；frontage 真实四字段正例（含实现缺额=8 的断言）；scene 载荷只 stat 不读不 hash。
- 负向（覆盖 R1-R4/P2）：authority demo/缺失；sourceSnapshot {} / 假 hash / 引用缺失文件；manifest 缺失/假文件/fake hash/错 hash；reports 不覆盖必需三报告/为空/错 hash；runtime 错 hash/缺失；generation 非 0 退出/running/未知状态/报告 interrupted/无 log 证据；gateRun 缺失/gateRun 矛盾；gate banana/`pass+failures`/failures 非法项/无授权基线/新增失败差异；缺必需报告/“final”目录缺报告/scale-audit 冒充；版本 999 → unsupported；格式/rev 缺失；空 scenes/数量矛盾；scene 绝对路径逃逸与 junction 逃逸；out 在 world 内/已存在（wx 拒覆盖）；world 目录缺失/out 目录权限等用法错误；报告文件是目录、receipt 路径是目录（结构化 incomplete，不崩溃）；字节预算非正整数报错、runReceipt 缺省安全默认；resolveStatus 优先级。

## 7. 未验证项与已知限制

- **integration_pending（真实 r49）**：r49 demo receipt 由 worker 生成（authority=demo、无 runtime/manifest/sourceSnapshot.files/generation exitCode/log、无 gateRun），故真实紧凑核对输出 fail/incomplete——正确表达“无总控受信收据时不得整体通过”。不会为制造通过而修改 receipt 为假主 Agent 授权。
- frontage 几何（实体支撑/入口/路由）未独立核对；计数不是几何真值，需 `scripts/check-frontage.mjs` 等受信回执，几何恒标 unverified。
- sourceSnapshot 的 commit 真实性与主 Agent 验收发生在工具外；工具只做文件级 hash 绑定与完整性校验。
- 本机无 pnpm；doc-tree:check 用其本体 Node 命令等价执行。费用无账单数据记 unknown；未跑大图、未跑完整 Rust 测试、无后台进程/服务、未新增任何依赖、未调用其它模型。

## 8. 声明

**未暂存、未提交、未推送。** 全部文件保持工作区未提交状态，待主 Agent 复验后统一处理。跨机器交付需另附新增文件（git diff 不含 untracked）。
