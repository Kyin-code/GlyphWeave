# DSH B01-G33-R2-C1：最终证据闭集收口

日期：2026-09-28。依据 `b01-g33-r2-independent-inspection.zh-CN.md`。

**只修证据闭集，不修产品。**G33-R2 产品候选、c06 包和 a05 浏览器记录冻结；G34 不启动。

## 0. 冻结输入

只读固定：

- `bevy/work/collaboration/ds-b01-g33-r2-minimap-clip-closure/cli-runs/c06-export/`；
- `runs/g33r2-a05/`；
- 产品源码、exe、model、HTML；
- G33/G33-R1/G32-R2/检查方旧目录。

禁止：修改产品源码、c06 包、旧记录、旧 manifest；禁止 cargo、产品 CLI、浏览器。

## 1. 允许的管理修改

只允许新建收口目录：

`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g33-r2-c1-evidence-closure/`

允许复制/引用只读证据，并新建：

- `closure-contract.zh-CN.md`；
- 修正版 verify 工具；
- `accounting-ledger.json` 的只读快照引用；
- 新 report/facts/budget/command-log；
- manifest/receipt/delivery。

不得在旧 R2 目录中修补 `verify-delivery.json` 或重写旧 manifest。

## 2. 必须解决

二选一，但必须在新目录实现并证明：

A. 将核验结果文件排除在 manifest 的最终自验证闭集之外，并在 manifest/receipt 中明确精确排除路径；或

B. 在最终 manifest 生成前生成 verify 结果，manifest 后只读运行不再覆盖该文件，另把只读核验 stdout 保存为未绑定日志。

禁止把 verify-delivery.json 同时作为“被验证输入”和“会被验证脚本重写的输出”。

必须证明：

- manifest 每个列出文件存在、bytes/hash 一致；
- manifest/receipt/delivery 双向绑定；
- verify 返回 `ok=true`、`problems=[]`；
- 运行 verify 后再次枚举仍零漂移；
- 旧 G33/R1/R2/G32 快照零漂移；
- a05 包路径和 35/35 记录仍明确绑定 c06；
- 149 个 JSON 等历史数量差异按实际新目录说明，不复制错误摘要。

## 3. 账目口径

不重新运行产品，因此不产生 cargo/CLI/browser 额度；管理脚本运行单列为 verification/management，不得伪装成产品进程。

报告必须同时列出：

- R2 原始实际账：cargo 8、CLI 33、Chrome 5、driver 10；
- R2 计划额度：4/6/2/6；
- C1 管理核验调用次数；
- a04 35/35 为非接受诊断，a05 35/35 为接受会话。

## 4. 交付

成功：交 `delivered_for_inspection`，仍不自宣 G33 通过；
若 verify 仍有漂移、排除范围含糊、receipt 绑定不完整，则交准确 partial/needs_changes；
不追加 cargo/CLI/browser，不进入 G34。