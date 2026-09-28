# DSH B01-G33-R2-C1-R1：管理文档三处更正

日期：2026-09-28。依据 `b01-g33-r2-c1-independent-inspection.zh-CN.md`。

**本任务只修新 C1 闭集的管理文档，不运行任何产品进程。**

## 冻结

只读冻结：

- G33 首轮、G33-R1、G33-R2、原检查方目录；
- G33-R2-C1 当前 16 项 manifest 绑定文件；
- 产品源码、exe、model、c06 包、a05/a04 记录。

禁止 cargo、产品 CLI、Chrome、产品 driver；禁止回写已封存的 C1 旧 manifest/receipt。

## 允许操作

在新目录中建立：

`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g33-r2-c1-r1-doc-correction/`

复制只读 C1 证据，并修正新目录中的：

1. 把不存在的 `closure-verification.json` 引用改为实际存在的 `verify-log.stdout.txt` 或明确的实际证据文件；
2. management ledger 的 verifier `exitCode` 改为真实 `0`；
3. verifier snapshot 将“没有这些字符串”改为“控制流没有这些写文件调用；注释可包含 API 名称”；
4. 新 report/facts/budget/command-log 说明旧 C1 原文不回写；
5. 重新生成 manifest/receipt/delivery。

## 必须证明

- 新目录 manifest 条目与实际文件 bytes/hash 全部一致；
- 精确排除仍恰好四项；
- verifier stdout `ok=true`、`problems=[]`；
- 核验前后新目录零漂移；
- 旧 C1、G33-R2、G33-R1、G33 首轮不漂移；
- C1 产品进程仍为 cargo/CLI/Chrome/driver = 0；
- 新报告没有引用不存在的文件；
- 旧 C1 目录保持只读，不改写。

## 交付

完成后交 `delivered_for_inspection`，不自宣 G33 无条件通过；随后由独立检查方完成 G33 最终限定裁定。G34 在裁定前不启动。