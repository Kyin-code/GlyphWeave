# B01-G26R1：可选上下文入口的拒绝后重新加载浏览器收口

日期：2026-09-22。

依据：
- G26 独立检查：G:/difyllmwiki/GlyphWeave/docs/collaboration/b01-g26-inspection.zh-CN.md
- G26 交付：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g26-context-preview-entry/

## 0. 唯一目标

只补测 G26 n02 的真实浏览器序列：

1. 访问一个会在绑定端口前被拒绝的坏 preview 包；
2. 确认没有可加载旧页面/旧模型；
3. 在**全新 page 或全新 browser context**中打开合法 G26 入口；
4. 确认入口页、status、同源 iframe 和 G25 canvas 2D viewer 正常加载；
5. 确认坏包拒绝不会污染随后合法入口。

这不是产品源码修复包。默认不得修改 G26、G25、G25R1、G24 产品源码或旧证据。

## 1. 驱动要求

- 使用两个独立页面或两个独立 browser context；拒绝导航失败后不得复用 `chrome-error://` 页面继续断言。
- 记录每一步实际 URL、HTTP/console/page error、入口进程 PID/exit、端口是否监听和页面模型状态。
- 坏包因服务不启动，优先使用进程/端口/HTTP refusal 断言，不制造“坏包截图”；若浏览器未得到页面，明确记录为 no-document。
- 合法入口必须用新页面加载，检查：sceneId、window、source hash、path A 合同、iframe `modelLoaded`、entity/road/access 计数和 observation-only 标签。
- 不使用列表选择替代浏览器入口加载；若做实体选择，需记录 `page.mouse.click` 或明确声明未做。
- 保留原 n02 崩溃日志，不覆盖、不改写；新运行使用新名称和新目录。

## 2. 最小验收

建议一次浏览器运行包含两个独立上下文，至少通过：

- bad entry：无页面或 refusal，端口不可达，旧模型不存在；
- good entry：入口加载，HTTP/status/iframe 正常；
- bad→good 隔离：good 页面没有残留 bad 状态或旧模型；
- 包内 2D viewer 事实显示和合同文本仍在。

如浏览器环境再次失败，准确列出 failure；不得扩大拾取半径、修改端口逻辑或把 HTTP 证据写成浏览器通过。

## 3. 范围和预算

建议新授权：

- 浏览器：1 次；
- 辅助驱动：1 次；
- cargo：0；
- CLI：0；
- 归档≤8 MiB；
- 不接 G23 三场景、WebGL/GPU、ST18、默认世界、工程 build/route/bake/realize/release。

如果只读复测通过，G26R1即可收口；如果仍失败，维持 partial，不为一项驱动观察修改产品合同。

## 4. 交付顺序

新目录：

`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g26r1-entry-browser-closeout/`

保存 driver、原始 browser log、browser acceptance、report、progress、递归 manifest、receipt 和只读核验。旧 G26 目录不覆盖，manifest 生成后不再改绑定文件。

正分支只能声明“G26 可选入口的拒绝后重新加载浏览器观察收口”，不能声明真实 3D、默认世界或工程安全验收。

**A 冻结、B partial、C 不推进。**
