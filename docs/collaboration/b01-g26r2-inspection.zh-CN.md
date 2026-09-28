# B01-G26R2 独立检查（2026-09-22）

## 1. 裁定

**G26R2 浏览器隔离收尾成立，达到 scoped delivered_for_inspection；G26/G26R1 的路径 A 只读2D入口合同可冻结。**

本次没有发现新的产品逻辑阻断。R1 的红项已通过“单次A读取、立即关闭A、再创建B、禁止收尾重读A”的结构化方案消除。整体仍不宣布真实3D/WebGL、默认世界、ST18、工程可行性或路线安全验收。

## 2. 独立只读核验

- G26R2 manifest：**11/11** bytes+SHA-256匹配，0未列文件。
- receipt对manifest、report、progress、read-only-verification、command-log、browser-acceptance六项绑定成立。
- G26、G26R1、G25、G25R1、G24目录与旧证据保持存在且未被本轮修改。
- G26入口及相关G25/G24源码绑定匹配；本次未修改任何产品源码、旧证据或固定来源。

## 3. 浏览器观察判定

### Context A

坏包入口来自G26 e01：缺`COMPLETE.json`，入口在绑定端口前拒绝。A context对拒绝页只执行一次读取；由于连接错误页没有执行上下文，`page.evaluate`抛出异常，驱动按确定性的无页面值记录：

- 导航拒绝；
- 连接错误；
- 无入口状态；
- 无viewer状态；
- iframe数0；
- body长度0；
- 无旧模型。

该异常没有被第二次读取补齐，`aReadFailed=true`和错误文本均保留。

### Context B

A关闭后才创建B，B独立加载合法G26入口，记录37/37：入口页、`/status.json`、同源iframe、G25 2D viewer、来源事实、计数、schema drift、observation-only标签和真实`page.mouse.click`命中。

### 隔离

结构化隔离组4/4：contexts独立、A先关闭、B计数与模型声明一致、没有执行A的收尾重读。总计**48/48**，0失败。HTTP仅作旁证，不替代浏览器事实；坏包没有可截图的包页面，不伪造截图。

## 4. 预算和边界

worker：浏览器1/1、辅助1/1、cargo0、CLI0；另有1个合法入口前置进程，单独披露、不计CLI矩阵，观察后已终止。

未做：G23三场景浏览器矩阵、WebGL/GPU、路径B迁移、ST18、默认世界、工程build/route/bake/realize/release。

## 5. 下一阶段分叉

路径A的保存场景只读2D入口已经完成收口。若继续走路径A，应做项目侧显式交接/选择入口，但不得扫描工作区或改变默认世界。

若产品要求真实三维，应走独立路径B：重新定义数据映射、3D几何合同、浏览器验收和预算；不得把G25/G26 canvas2D结果改名为3D通过。

证据：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g26r2-20260922/。

**A冻结、B partial、C不推进。**
