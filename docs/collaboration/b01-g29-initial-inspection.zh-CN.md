# G29首轮独立检查与续作裁定

日期：2026-09-24（Asia/Shanghai）。**裁定：接受真实部分交付，G29保持needs_changes；不把本轮有纪律停点当成没有推进，也不自称验收。**

## 1. 本轮独立证据

检查目录：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g29-20260924/。

- G29 manifest80条bytes/hash匹配，无未列；receipt关键报告绑定一致；10张当前截图与browser-record/browser-index引用全部匹配。
- 当前源码context_preview3d.rs 181049B/53fa2f97…；JS67878B/6812141c…；exe42323968B/f9cb9a48…与交付绑定一致，viewer资源与源码一致。
- model与H1逐字节相同，922d404c…；H1/G28/R1/G27B-R1四套旧清单43/102/60/114条均匹配，15项固定/保护文件未漂移。
- 检查员独立执行真实viewer JS的VM Part A 40/40；该DOM布局是夹具模拟，不冒充真实布局测量。
- 检查员独立CLI单测93/93；用已绑定exe对交付包新写独立audit报告，exit0、四段pass。未重建exe、未改产品、未重跑core或浏览器；worker core82项与浏览器记录按原来源引用。
- 比较H1冻结viewer与G29源码，terrainTopology/terrainIndicesFor/drawIndexedTriangles/drawSolidTriangles/surfacePickData/rayTriangle六个函数及材质/光照/着色器块逐字节一致。

## 2. 已确认的用户体验进展

实际查看1280浏览首屏与960检查截图。工具栏、地图、选择卡分区清楚；默认浏览模式不再用固定400px侧栏挤占地图；检查模式保留事实。worker真实浏览器记录支持两尺寸地图宽度100%、实体/地形点击、选择卡/高亮、模式往返三轮、实时HUD、几何底线和重载稳定。

这里的100%是**视口宽度比例**，不是整个视口面积百分比；工具栏78px及底部卡片仍占高度。后续报告同时给width/height/area，避免把宽度改善扩大成所有面积可交互。模板没有z-index也不独立证明没有遮挡；应以布局、实际命中/可达验证为准。

## 3. 三项剩余阻断

### P1 生产者build id未更新

源码与G29 manifest仍为g28r1h1-viewer-3d-4，H1也是同值；实际viewer字节不同。当前audit依靠具体生产者字节检查仍能区分脚本，但标识本身不能再表达唯一构建。原任务已要求更新，必须在正式最终包前修为G29专属版本。不能只手改包manifest，必须改生产者常量→测试→最终构建→新导出→新audit。

### P1 合并矩阵未到达退出条件

browser-record.partBError为`ReferenceError: page is not defined`，发生在坏包段。未证明G29页面坏包拒绝及恢复，不能借H1坏包证据替代。

检查模式1280记录controlsInsideViewport=false，但没有逐控件失败快照。worker关于折叠details布局框的解释是候选原因，不是已证实根因。仅把过滤改成offsetWidth也不足以保证可见与可点击：需要考虑hidden/display/visibility、祖先details开关、滚动裁剪、视口交集和遮挡。折叠子控件不应当作当前可点击控件，也不能永久排除而跳过展开后的验证。

### P1 运行证据覆盖

实际驱动runDir按vmOnly选择固定a01/b01目录，b02确实会复用b01。这不是偶发工具问题，是记录器设计缺陷。当前保留的10图与b02记录相符，但b01中止时的两图及部分记录不可恢复；清单0 stale不代表历史失败证据完整。

本轮遵守cargo4/6、CLI6/6、browser2/4、aux4/4停止，不新增超支，予以明确认可。仍不接受“每次重跑先覆盖旧文件”的做法；运行目录唯一性和已有目标拒绝必须先于下一轮任何CLI/浏览器运行验证。

## 4. 续作范围和预算

不重做已通过的浏览优先布局，不重开全套H1/G28，不推进G30或地图生成。G29-R1一次完成：记录器隔离与预检→build id与回归→最终构建→新包export/audit→两项缺失浏览器验证及最小既有功能回归→统一交付。

原余额cargo2、CLI0、browser2、aux0。正式追加CLI3、aux3，cargo/browser不追加；新累计上限cargo6/CLI9/browser4/aux7，本次可用cargo2/CLI3/browser2/aux3。两个额外CLI/aux之外的余量是为了允许一个受控重试，不要求用满，不倒改旧账。历史G28超支和G29证据覆盖永久保留。

任务：G:/difyllmwiki/GlyphWeave/docs/collaboration/dsh-b01-g29-r1-final-package-task.zh-CN.md。

## 5. 检查员成本与结论

本轮检查员cargo1、产品CLI1、VM回放1；新浏览器运行0。仅创建检查/协调文件，产品源码和worker证据未写入。

**A冻结；G27B/G28原限定验收不倒写；G29部分进展成立但仍needs_changes；G29-R1立即执行；B整体partial，C不推进。**
