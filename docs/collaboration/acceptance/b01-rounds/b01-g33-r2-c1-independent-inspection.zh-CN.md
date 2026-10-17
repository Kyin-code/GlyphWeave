# G33-R2-C1 独立检查：闭集核心成立，保留三处管理文档更正

日期：2026-09-28。结论：**C1 的证据闭集核心成立；需做一次零产品资源的文档更正后，才能把 G33-R2-C1 作为完整管理收口。**

## 1. 已独立复核成立

对新闭集 `ds-b01-g33-r2-c1-evidence-closure` 进行只读核验：

- manifest 绑定 16 项文件；实际目录 20 项文件，另外 4 项正是声明的排除项：manifest、receipt、delivery、verify-log.stdout；
- manifest 每项 bytes/hash 一致，missing=0、drift=0；
- receipt→manifest、delivery→manifest、delivery→receipt 均成立，状态一致；
- `node verify-delivery.mjs .` 只输出 stdout，返回 `ok=true`、`problems=[]`；
- verify 前后 manifest hash 相同：`bfc0f9c8…`；
- G33 首轮 42、G33-R1 216、G32-R2 81、检查方 6 项旧快照 drift 均为 0；
- a05 35/35、exit 0、packageUsed 指向 c06，a04 明确为非接受回退包诊断；
- C1 没有 cargo、产品 CLI、Chrome 或产品 driver 进程；管理调用单独记账。

我实际运行了新 verifier，输出与绑定的 stdout 内容一致，且没有改变闭集文件。

## 2. 三处管理文档问题

### 2.1 报告引用不存在的文件

`closure-report.zh-CN.md` 在表格中写“见 `closure-verification.json`”，但该文件不在目录中，也不在 manifest 中。实际对应证据是未绑定的 `verify-log.stdout.txt`。

应改为：

```text
见 verify-log.stdout.txt 的 sections.manifestEntries
```

或把实际存在的证据文件名称写清楚，不能保留不存在的路径引用。

### 2.2 management-ledger 缺少实际 verifier exit code

`management-ledger.json` 的第三条 verification entry 使用：

```json
"exitCode": null
```

但实际 verifier 返回 exit 0，且 stdout 的 `ok=true` 已证明。管理账必须记录真实的：

```json
"exitCode": 0
```

不能用 null 代替已知结果。

### 2.3 verifier-snapshot 的文字口径不准确

`verifier-snapshot.json` 写成源码“没有 `writeFileSync` / `appendFileSync` / `mkdirSync` / `copyFileSync` 出现”。但这些名称出现在 verifier 的注释中。

独立检查确认的是更准确的事实：

- 控制流中没有这些写文件调用；
- verifier 只使用读取/枚举/哈希操作；
- verifier 的实际输出只有 stdout；
- 源码注释中可以提到被禁止的 API 名称，但不能说字符串完全不存在。

这不是写入行为缺陷，而是静态证据文字过度表述。

## 3. 裁定

- G33-R2 的产品候选与 a05/c06 功能证据保持有效；
- C1 的闭集分离方案 A+B 已经技术上成立；
- 当前只保留管理文档 `needs_changes`，不重新打开产品验证；
- G34 继续不启动；
- 不修改 G33-R2 旧目录、旧 manifest、旧 receipt 或产品文件。

下一步是 C1-R1 文档修正，完成后即可对 G33 做限定接受裁定。