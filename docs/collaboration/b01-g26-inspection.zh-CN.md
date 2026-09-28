# B01-G26 独立检查（2026-09-22）

## 1. 裁定

**G26 显式可选只读项目入口合同成立，可作 scoped delivered_for_inspection；整体仍按 partial 停留，唯一未闭合的是修正驱动后的 n02 浏览器序列未重新测量。**

没有发现入口源码、包完整性、端口绑定前拒绝或 HTTP 路由方面的新生产逻辑阻断。不要把 n02 的驱动崩溃改写成通过，也不要把 HTTP 拒绝观察冒充完整浏览器序列。

## 2. 独立核验

- G26 递归清单：**72/72** bytes+SHA-256 匹配，0 个未列文件。
- receipt 对 manifest、report、progress、read-only-verification、command-log、browser-acceptance、http-observations 七项绑定成立。
- G26 六项产品源码绑定一致；G25/G25R1/G24 旧目录和证据存在且未被本轮修改。
- 当前源码 `cargo test -p glyphweave-cli --offline --bin glyphweave`：**46 passed / 0 failed**。
- 入口路径明确要求显式 `--preview`，没有工作区扫描；校验失败发生在端口绑定前；报告路径不能写入包内；HTTP 路由有入口页、status、受限 package 文件和路径穿越拒绝。

## 3. 入口合同判断

已接受：

- 六个必需包文件、marker/manifest/model 哈希链和 v1 schema 独立校验；
- 额外文件、未知版本、缺 marker、错误 hash、缺 `--preview`、包内 report 路径拒绝；
- 页面展示 scene/window、来源哈希、schema drift、natural/final 比较、实体/道路/access 计数、referenceOnly 和 observation-only 标签；
- 同源 iframe 嵌入 G25 canvas 2D viewer；不改 G23 Three/WebGL viewer；
- 校验失败不绑定端口，因此不会显示旧模型；
- 只读包和来源，不生成 EngineeringInput、不重新求路、不升格 `connected`/`walkable_route`。

## 4. 浏览器证据边界

worker 报告：

- n01 clean entry：16/16；
- n03 surfaces/mouse hit：15/15；
- n02 refused-entry-then-good：0/1，原因是驱动复用同一页面，拒绝导航后停留 `chrome-error://`，后续导航未执行。

驱动已修正，但浏览器额度已用尽，故**不能把修正后的 n02 写成已测通过**。HTTP 入口拒绝、不可达端口和 n01/n03 的独立加载证据只能作为替代的部分证据，不能等价替代该序列。

无坏包截图不是缺陷：拒绝发生在服务绑定前，正确证据应是无端口/无页面和 HTTP/进程结果，不应伪造截图。

## 5. 范围边界

G23 三场景浏览器矩阵、WebGL/GPU、真实3D、ST18、默认世界、SceneIndex 默认消费者、工程 build/route/bake/realize/release 均未做，也不属于 G26。

## 6. 预算与停点

worker 本轮 cargo2/4、入口进程8/8、浏览器3/3、辅助3/4；本次检查员运行 cargo1，未运行生产CLI或浏览器。未修改 G26/G25/G24 代码、证据或固定来源。

## 7. 证据

检查员目录：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g26-20260922/。

**A 冻结、B partial、C 不推进。**
