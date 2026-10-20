# HTML preview smoke + loading overlay (2026-10-20)

## 1. 问题

切到 2.5D 后，页面在 chunk 加载完成前显示纯黑 canvas：512m 约 3s，1km 的
2×2 chunks 约 8s。console 没有 error/warn，但“黑屏”会被误认为空世界或渲染
错乱。

## 2. 修复

- `adapters/html/index.html` 增加 `#loading` 覆盖层和确定性 spinner；
- `app.js` 在 `requestNearDraw` 时显示，`drawNear` ready 或失败时隐藏；
- 切回战略图时立即隐藏，避免覆盖层残留。

这只是预览 UX，不改变 Rust 权威数据、chunk 索引或场景内容。

## 3. 浏览器实测

| 世界 | 视图 | 结果 |
|---|---|---|
| `b01-joint-plains-complete-512` | 战略图 / 2.5D | 2.5D 初帧显示加载层；3s 后出现道路、建筑、树木、田块；无 console error/warn |
| `b01-1km-complete`（2×2 chunks） | 战略图 | 四块连续，无 chunk 接缝/空洞 |
| `b01-1km-complete`（2×2 chunks） | 2.5D | 初帧加载层；约 8s 后出现连续近景；无 console error/warn |

## 4. 注意

`glyphweave preview` 从 **world 目录内的 `preview/` 副本**读取 HTML。修改
`adapters/html` 后必须重新烘焙世界，或对测试世界显式复制新的
`index.html`/`app.js`；否则旧 world 仍会显示旧 UI。生产路径会在
`preview_stream::write` 时复制。
