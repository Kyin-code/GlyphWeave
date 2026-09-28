# DSH B01-G27B-R1：最终证据闭集修复任务

日期：2026-09-23。状态：**立即执行，不新增产品范围，不需要重新批准。**

## 1. 目的

continuation-3已经完成主要产品实现与运行验证。本R1只修复独立检查发现的递归证据闭集问题，不改3D产品源码、不改viewer、不重跑旧矩阵、不把旧包改名为新包。

独立检查报告：
`G:/difyllmwiki/GlyphWeave/docs/collaboration/b01-g27b-continuation3-inspection.zh-CN.md`

## 2. 必须完成

1. 新建后续证据目录，不覆盖：
   `G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g27b-context-preview-3d/continuation-3-r1/`
2. 从continuation-3现有内容读取并递归列出所有文件。必须处理：
   `packages/clean-3d/manifest.json`
   - 要么作为正常manifest条目列入；
   - 要么在顶层manifest的`excluded`中以精确相对路径和理由明确排除。
   默认优先列入，避免隐藏嵌套文件。
3. 新manifest逐项检查bytes/SHA-256、0 stale、0 unlisted；receipt绑定manifest、R1报告、progress、命令日志和独立检查引用。
4. 报告必须明确：
   - continuation-3原目录未改；
   - 原报告“0 unlisted”被本R1纠正；
   - 产品源码、最终exe、21项CLI结果和浏览器证据均是引用，不可重写旧哈希；
   - R1没有新增产品运行，或如确需运行必须单独计数。
5. 只读核验旧G27B/G27A/G26/G25/G24及固定来源哈希不变。

## 3. 不得做的事

- 不因检查发现嵌套manifest漏列而修改continuation-3原manifest/receipt；
- 不修改`context_preview3d.rs`或viewer修UI；
- 不把`#selection`空面板或HUD计数标签写成已修复；
- 不把资产清单纠正扩大成G27B新矩阵或浏览器验收。

## 4. 后续体验任务

selection面板和HUD语义应转入独立的G28地图呈现/交互体验任务：修复后重新构建、重新导出包，并至少做实体命中显示、观测计数显示、三视角截图和基础操作复验。不得使用G27B旧浏览器截图证明修复后的体验。

本R1完成后状态仍为`delivered_for_inspection`，最终是否验收由独立检查裁定。预算不重置、不追加；本任务建议不消耗cargo/CLI/browser/aux。
