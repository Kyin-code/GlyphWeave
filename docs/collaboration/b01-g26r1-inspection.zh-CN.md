# B01-G26R1 独立检查（2026-09-22）

## 1. 裁定

**G26R1 的实质浏览器要求已取得证据，但因收尾冗余重读产生一项红项，不能写成完全闭合；维持 partial。** 没有发现产品入口、端口拒绝、来源显示或隔离逻辑的新缺陷。

## 2. 独立只读核验

- G26R1 清单：**11/11** bytes+SHA-256 匹配，0 个未列文件。
- receipt 对 manifest、report、progress、read-only-verification、command-log、browser-acceptance 六项绑定成立。
- G26 产品源码 0 drift；G26/G25/G25R1/G24 旧目录和证据未被修改。
- source-binding 中列出的入口源码、main、G25 preview、engineering_context 与 core 源码均匹配。

## 3. 浏览器结果判定

### Context A：坏包入口

首次检查记录为 **7/7**：

- 导航连接被拒；
- 无入口状态；
- 无 viewer 状态；
- 无 iframe；
- 空 body；
- 无旧模型；
- 端口不可达。

该证据与 G26 e01 的缺 marker、未绑定端口记录一致。

### Context B：合法入口

A 之后新建独立 context，记录 **37/37**：

- 入口页；
- `/status.json`；
- 同源 iframe；
- G25 2D viewer；
- 来源事实、计数、schema drift、observation-only 标签；
- `page.mouse.click` 真实命中；
- 合法入口未携带坏包状态。

### 唯一红项

收尾时重新读取 A 页的 `refusedContextStillEmpty` 得到 `actual=false`，与 A 首次 7/7 记录不一致。worker 已如实保留该失败，没有复跑或删除。

该红项属于驱动收尾重读异常，不能改写为通过；但也不能用 HTTP 观察直接替代完整浏览器序列。因此 G26R1 仍为 partial。

坏包截图是浏览器自身连接错误页，不是包页面；没有伪造坏包页面截图，处理正确。

## 4. 预算与停点

worker：浏览器1/1、辅助1/1、cargo0、CLI0；另启动1个合法入口进程作为浏览器前置，不计入 CLI 矩阵，但已披露。检查员本轮只做绑定核验，未运行 cargo、CLI 或浏览器。

G26R1 不修改任何产品源码、G26/G25/G25R1/G24证据或固定来源。

## 5. 结论

接受：坏包入口确实在端口绑定前拒绝；合法入口在独立 context 中正常加载；G25 2D viewer、来源事实和真实鼠标命中成立。

未接受：拒绝后隔离序列的收尾重读已完全闭合。

**A 冻结、B partial、C 不推进。**
