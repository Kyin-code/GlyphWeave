# B01-G27A 独立检查（2026-09-22）

## 1. 裁定

**G27A 路径A显式只读项目交接入口实现成立，但浏览器证据仍partial；不宣布G27A浏览器观察完全闭合，也不启动G27B。**

入口合同、入口进程和HTTP合同没有发现新的生产逻辑阻断；两个浏览器失败已合理归因为驱动查询/导航问题，但修正后的驱动未复跑，因此不能把它们写成“本轮已测通过”。

## 2. 独立核验

- G27A manifest：**67/67** bytes+SHA-256匹配，0未列文件。
- receipt对manifest、report、progress、read-only-verification、command-log、browser-acceptance、http-observations七项绑定成立。
- 本次复跑 `cargo test -p glyphweave-cli --offline --bin glyphweave`：**50 passed / 0 failed**。
- G26/G26R2/G25/G25R1/G24旧目录和证据存在，未被本轮修改。
- G27A本轮声明的产品变更只有 `context_preview_entry.rs` 追加交接函数与 `main.rs` 注册；这两个文件相对G26基线发生漂移是**已声明改动**，不能误报为未声明漂移。其余 G25/G26 核心预览源码保持匹配。

## 3. 已接受的入口合同

- `--preview`必须显式给出；无工作区扫描、无最近包发现、未知扫描参数拒绝；
- 六必需文件、常规文件、受限路径、marker/manifest/model哈希链和schema版本校验；
- 坏包在绑定端口前拒绝；同包同端口不启动第二个服务；异包不静默切换既有显示；
- `--report`落在包内时服务前拒绝且不写包；
- HTTP入口/status/package路由和路径穿越规则符合合同；
- 页面继续明确路径A：只读2D投影，无WebGL/3D、无工程可行性、无路线安全证明。

## 4. 浏览器证据边界

### p01

**18/18通过**：入口事实、哈希、schema drift、natural/final、计数、观测标签、iframe viewer和真实`page.mouse.click`命中均成立。

### p02

**0/1，驱动缺陷**：坏包拒绝后复用同一页面，页面进入`chrome-error://`，后续合法入口导航中断。驱动已经改为新页面，但浏览器额度耗尽，未复跑。本轮不把“旧模型已清空”写成浏览器已测，只引用既有G26/G26R2和入口拒绝证据作为旁证。

### p03

**4/5，驱动缺陷**：同包两次选择的实质断言——模型哈希、实体/access/标签计数稳定且无重复对象——通过；唯一失败是徽标选择器查错文档，徽标位于iframe内。驱动已修正但未复跑。

所以浏览器统计如实为 **22/24**，不能折算成24/24。

## 5. G27B状态

G27B真实3D/WebGL任务**未开始**。G27A没有修改G23 viewer，也没有将路径A canvas 2D改称3D。

## 6. 预算与停点

worker自报cargo2/4、入口进程8/8、浏览器3/3、辅助3/4；检查员本轮cargo1、入口CLI0、浏览器0。未接ST18、默认世界、自然生成、工程build/route/bake/realize/release。

G27A维持partial，产品入口实现可冻结；下一步只做修正驱动的最小浏览器补测，不修改入口产品逻辑。

证据：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g27a-20260922/。

**A冻结、Bpartial、C不推进。**
