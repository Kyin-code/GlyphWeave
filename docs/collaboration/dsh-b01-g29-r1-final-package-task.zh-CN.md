# DSH G29-R1：生产者版本、缺失浏览器项与不可覆盖记录器收口

日期：2026-09-24。**正式批准立即执行，不需再确认；同一G29范围收尾，不是G30。**

依据：G:/difyllmwiki/GlyphWeave/docs/collaboration/b01-g29-initial-inspection.zh-CN.md。
原任务G:/difyllmwiki/GlyphWeave/docs/collaboration/dsh-b01-g29-map-first-view-task.zh-CN.md的来源/几何/体验合同保持。

## 0. 当前状态

已接受浏览模式默认、同一模型双模式、选择卡与HUD同步的真实部分进展。剩余是build id仍为H1、坏包/恢复未跑到、检查模式控件测量未定位，以及run目录复用导致历史覆盖。不要继续重做已有布局，不先做美术/晕渲，也不要先复测浏览器再改变生产者版本。

## 1. 一张有效预算表

| 类别 | G29已用 | 本次新增 | 累计上限 | 现在可用 |
| --- | ---: | ---: | ---: | ---: |
| cargo | 4 | 0 | 6 | 2 |
| 产品CLI/服务入口 | 6 | 3 | 9 | 3 |
| 浏览器启动尝试 | 2 | 0 | 4 | 2 |
| 辅助驱动执行 | 4 | 3 | 7 | 3 |

G29-R1归档≤64MiB，旧证据引用不大量复制。预算从现有真实计数续算，不能重置；新上限不用于追溯抹平G28旧超支。每次意图记录先写run-id和余额，再执行，失败也计。

建议：辅助1用于VM/记录器/驱动预检，辅助2合并最终CLI与浏览器，辅助3只作重试余量。浏览器优先一次会话多page覆盖，第二次只作明确失败后的余量。cargo两次用于最终单测及build；CLI至少export+audit，第三项保留确定性/必要服务或失败重试。若外部浏览器不支持file，应按其政策允许的正常方式安排入口；不得绕过明确URL/权限禁止。

## 2. 隔离输出，先修记录器

新目录：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g29-r1-final-package/，确认不存在后创建。原G29/H1/G28/R1/G27B所有包和记录只读。

- 驱动必须显式接收`--run-id`与`--output-root`或等价参数，不能按vmOnly或browser模式固定目录。
- 每次执行原子创建全新run目录；若已存在立即拒绝，绝不recursive:true复用、清空、删除或替换旧内容。可复用同一逻辑模式，但不可复用路径。
- intent、stdout/stderr、exit、VM/browser记录、截图全部在该run目录；一次正式执行的最终记录只写一次，不同尝试互不覆盖。
- 脚本捕获异常后保留已获得actual与错误栈，明确每项pass/fail/notObserved；finally只关闭本次进程/context，不清空证据。
- 预检中用隔离临时夹具验证“相同run-id拒绝且哨兵不变”，不要用旧G29目录测拒绝。
- VM-only允许尚未导出包：读取冻结H1模型和当前JS，包hash缺省记notAvailable，而非读取不存在路径中止。正式CLI成功导出后，再做严格全资源/binary/hash预检，缺包就不启动浏览器。
- 在浏览器启动前验证相机辅助字段、输入参数、旧H1可选状态字段、坏包段page变量作用域和结果分类器；不再出现散落未定义`page.evaluate()`。

新增记录器检查并非文档工作；实际测试其失败安全行为后再进行产品运行，不再仅在注释声称“每次独立目录”。

## 3. 产品改动只做必要项

允许改G:/difyllmwiki/GlyphWeave/bevy/crates/cli/src/context_preview3d.rs的VIEWER_BUILD_ID及测试，改为明确G29专属值（建议g29-map-first-view-1）；其他资源标签同步由生产者生成。只改版本不改变model schema或场景事实。

如控件可达实测证明有真实布局/交互缺陷，允许修G:/difyllmwiki/GlyphWeave/bevy/crates/cli/src/g28_viewer3d.js或页面模板的相关局部；不得在证据不足时预先断言只是驱动错。

保持模型hash922d404c…与H1字节一致，地形/几何/材质/拾取算法不变，窗口256,32,64,64不变。固定world/context/v1沿用原G29任务。不修改G23、core或默认消费者，不派子代理、不Git写入。

顺序：完成所有计划内代码/驱动更改→VM及预检→最终CLI单测→最终build→新输出目录export→new audit→浏览器。不能用旧exe导出后再更新build id，不能手改旧包manifest。

## 4. 最终包CLI证据

必须由最终exe执行：
1. 显式world/context/v1导出本R1新包，先确保新包父目录存在；
2. 新包audit四段pass，schema/producer资源/hash绑定正确；
3. 可用余量用于二次确定性导出或必要重试/服务，不为凑数运行。

记录最终源码/JS/exe/model/html/viewer/manifest/marker的bytes/hash与build id，证明build id不同于H1且源码与包一致。既有两个负例按原run引用，必要的生产者版本回归放单测，不要求重复消耗CLI6项。旧G29审核记录是历史，不冒充新包audit。

## 5. 一次合并浏览器：缺口必须闭合

### 5.1 逐控件可见与可达

在1280×800与960×640分别进入浏览/检查模式，保存逐控件：id/role/name、computed display/visibility、是否hidden、祖先details是否open、布局框、视口交集、滚动容器裁剪、判定与原因。

规则：
- 折叠details内部控件不参与当前可点击断言，但记录为collapsed—not applicable；必须实际展开后再测其子控件并点击。
- 检查面板内可滚动内容不要求全部同时落在视口；应逐个scroll into view后验证可达，记录滚动前后矩形。标题和主工具栏应始终可达。
- offsetWidth/getBoundingClientRect单一条件不是最终可见性证明；结合祖先状态与实际点击/命中，必要时中心点elementFromPoint检查覆盖（子元素命中可接受）。
- 不把任何false都当失败；已隐藏overlay=false是正常状态。列出必需断言及预期值，区分fail与notApplicable。
- 失败必须写具体控件和actual，不得只记录`controlsInsideViewport=false`。若真实UI有问题，修复并按剩余额度重测；不能换过滤器把失败控件忽略。

### 5.2 坏包真实拒绝/干净恢复

从本R1新包在本次scratch受控复制构造截断内嵌模型，使用明确的坏包page实例导航并记录modelLoaded=false、错误文字可见、无旧模型/选择残留、画布已清空。**当时截图**，然后恢复新干净包确认browse首屏和几何正常。不要访问空端口代替坏包；不要在恢复后补拍坏状态图；辅助catch不得把不可读取状态填写为预期零值。

浏览器错误分类：区分预期产品拒绝消息与未捕获脚本/page错误，不要求隐藏正常报错来“0 errors”。记录全部console/page错误原文。

### 5.3 新版本最小正向回归

不重新完整跑历史G28矩阵，但在同一最终新包会话内验证：
- 默认browse、两尺寸地图宽度比例≥75%、检查模式打开/收起；同时记录画布width/height及面积比例（100%宽度不是100%面积）；
- 一次真实实体及一次真实地形鼠标命中，卡片/HUD/runtime一致；
- 两表面与实体/道路/overlay显隐，23814/7938/4096与全显示8022、单地形7938；
- 模式往返三轮，surface/selection/camera与模型不变；三次重载无重复；
- 至少新包首屏、检查控件展开、坏包、恢复四类截图，记录相机/视口/包hash。

已交付10张图作为旧版进展引用，不把它们重新标成新build观测。对于尚未观察的项一律notObserved；浏览器启动后不得为了补完而超过余额。

## 6. 证据与退出

本R1记录器用新run-id，截图相对路径按run目录解析；所有内部引用与文件hash再交叉核验，清单自身0 stale不代替browser-record内部引用一致。嵌套包manifest正常列入、排除只用精确相对路径。

原G29 b01证据被b02覆盖的事实保留，不伪造恢复、不回写原run-provenance。统一总账：G29原cargo4/CLI6/browser2/aux4＋R1真实新增；历史G28超支不转成G29余额。

产品/预算/证据分开写结论：三项缺口均闭合且新包验证成立时交delivered_for_inspection；独立检查裁定最终验收。没有真实验证不写“假阴性已修”；只有文档改进也不算产品完成。

**立即按“记录器与版本→测试/build→export/audit→最终浏览器→新证据闭集”执行，不另等批准，不进入G30或美化任务。**
