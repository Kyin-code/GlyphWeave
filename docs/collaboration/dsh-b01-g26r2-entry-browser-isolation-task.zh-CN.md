# B01-G26R2：拒绝入口到合法入口的单次浏览器隔离复测

日期：2026-09-22。

依据：
- G26R1 独立检查：G:/difyllmwiki/GlyphWeave/docs/collaboration/b01-g26r1-inspection.zh-CN.md
- G26R1 交付：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g26r1-entry-browser-closeout/
- G26 入口任务：G:/difyllmwiki/GlyphWeave/docs/collaboration/dsh-b01-g26-context-preview-entry-task.zh-CN.md

## 0. 唯一目标

只补齐 G26R1 的一个观察缺口：在**同一次浏览器运行**中，用两个独立 browser context 完成坏包拒绝到合法入口加载的隔离证明。

本任务不是产品源码修复，也不是 G26/G25/G25R1 重跑包。

## 1. 固定场景

### Context A：坏包

使用 G26 e01 记录对应的缺 marker preview 包或等价本轮自有副本。入口必须在绑定端口前拒绝。

A context 只做一次状态读取，不做收尾重复读取。记录：

- `ERR_CONNECTION_REFUSED` 或等价连接拒绝；
- 无 `__ENTRY_STATE__`；
- 无 `__PREVIEW_STATE__`；
- 无 iframe；
- 空 body／无文档；
- 无旧模型；
- 端口不可达。

坏包没有可截图的页面，不伪造坏包页面截图。

### Context B：合法包

A context 完成后新建全新的 page 或 browser context，加载合法 G26 入口。

记录：

- 入口页和 `/status.json` 正常；
- 同源 iframe 与 G25 2D viewer 加载；
- sceneId、window、source hash、实体／道路／access 事实；
- `sourceObservationOnly`／`notReverified`；
- 合法包没有 A 的错误状态；
- 如做实体选择，必须是真实 `page.mouse.click`，否则明确不声明鼠标命中。

## 2. 驱动要求

- 不复用 `chrome-error://` 页面；A、B 必须是两个独立 context，或至少两个独立 page 且 B 新建于 A 完成后。
- 不在 A 完成后重新读取 A 页状态；不加入会造成第二次状态不一致的冗余收尾断言。
- `separateContexts=true`、A/B 创建顺序、URL、端口、进程状态全部原始记录。
- 不使用列表选择或身份选择替代入口加载；不把 curl/HTTP 观察冒充浏览器观察。
- 保留 G26R1 原始失败记录，不覆盖、不删除、不倒写。

## 3. 最小验收

一次浏览器运行、两个独立 context，至少满足：

- A：坏入口 7 项全部通过；
- B：合法入口关键事实和 viewer 正常；
- `separateContexts=true`；
- B 的模型／计数与合法包声明一致；
- A 的拒绝状态没有污染 B；
- 没有第二次 A 状态重读红项。

总检查数可少于 G26R1 的 47 项，但必须保留来源、端口、context 隔离和污染判断，不能以减少断言掩盖缺失。

## 4. 预算与范围

建议新授权：

- 浏览器：1 次；
- 辅助驱动：1 次；
- cargo：0；
- CLI：0；
- 前置合法入口进程：1 次，单独记录，不计 CLI 矩阵；
- WebGL/GPU、G23 三场景、ST18、默认世界、工程 build/route/bake/realize/release：0。

如果浏览器环境再次失败，维持 partial，不修改端口合同、不扩大拾取半径、不更改 G26 产品代码。

## 5. 交付

新目录：

`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g26r2-entry-browser-isolation/`

保存 browser driver、原始日志、browser-acceptance、report、progress、递归 manifest、receipt、只读核验。G26/G26R1 旧目录不覆盖；manifest 生成后不再修改绑定文件。

正分支只能声明“G26R2 拒绝入口到合法入口的浏览器隔离观察收口”，不能声明真实3D、WebGL、默认世界或工程安全验收。

**A 冻结、B partial、C 不推进。**
