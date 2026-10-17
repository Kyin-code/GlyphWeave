# DSH B01-G30-R4：详情图例换行与子项可见性收尾（不重跑整套矩阵）

日期：2026-09-25。正式批准直接执行。范围依据：`G:/difyllmwiki/GlyphWeave/docs/collaboration/b01-g30-r3-inspection.zh-CN.md`。

## 0. 已裁定与目标

R3的45px摘要和80.875%/76.09375%面积已限定通过；详情入口的鼠标/键盘、不重载和状态保持已有原始证据，不需要仅为afterClick变量名重跑。

本轮唯一产品目标：**让检查侧栏的五个图例项及色块完整换行可见，不需要横向滚动找尾项；关闭后保留R3摘要和地图面积。** 不另加读图功能、不接G31、不改世界/模型/地形/拾取/材质光照/core/G23/pathA。不接ST18/默认世界/EngineeringInput/工程链/发布。

新目录：`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g30-r4-detail-legend/`。所有旧包/driver/manifest/receipt/report/handoff冻结。R1已有历史封存变更照旧登记，不修旧目录。

## 1. 最小产品补丁

允许编辑 `G:/difyllmwiki/GlyphWeave/bevy/crates/cli/src/context_preview3d.rs` 的CSS、生产者ID与相关测试。优先**不改** `g28_viewer3d.js`，以保持R3已验行为/几何/读图计算逐字节不变。

- 将摘要专用不换行规则限定到 `#reading-bar`；或给 `#reading-details .legend-row` 明确覆盖为wrap。
- 详情的.legend-item必要时允许内部文字换行，min-width:0、max-width约束应防止右侧溢出；色块不可挤没。上下文长文本允许适当断行。不要给详情overflow:hidden裁字，也不要把全局摘要行改成wrap破坏已合格首屏。
- 新生产者ID `g30-detail-legend-3`。model必须保持922d404c…；若只有模板CSS/ID变更，viewer应仍d90923a3…，不得无关格式化。
- 新增具名守卫：摘要与详情样式作用域分离、详情允许换行、没有overflow裁切兜底；守卫不是浏览器可见性证明。

## 2. 生产链与旧证据继承

相关CLI单测一次→最终build一次，记录源码/exe/hash。使用已经实际成功的CLI命令形状，不再试错--out/--report；先从R3成功产物/源码参数定义确认。

在新目录export→audit四段pass→第二次export比较六文件确定性。新HTML/manifest/marker预期变化；model与viewer如上保持。若出现其他产品变化则本轮停下说明，不能把既有观察直接迁移。

新候选绑定在浏览器之前写好，来源为记录过的源码/build/export/audit，不在预检时把实际文件重新读成expected。几何/拾取/方向等R3既有证据可以明确引用，因为对应viewer/model未变；本次只证明新模板没有破坏相关界面路径，不虚构“又做20场景”。

## 3. 聚焦浏览器：一会话、两尺寸、两个面

沿用已明确修复标识符的R3新副本，不回写旧driver。可以单独写小型聚焦入口，复用已验证模块，不复制无关154项主流程。启动前必需项和截图key固定且唯一，参数解析/关键变量先静态审阅；脚本执行检查在aux计账。

四组合1280×800/960×640 × final/natural：

1. 默认browse实测摘要≤48px、画布宽100%、面积≥80%/75%，截图。不要重新扩大成布局探索。
2. 真实点“读图详情”进入inspect；在正常侧栏宽度下测 **每一个.legend-item及其每个.swatch** 的可见矩形、祖先裁剪和必要纵向滚动。记录文字、right相对panel内容right、scrollWidth/clientWidth/scrollLeft。
3. 五类图例完整出现、尾项2D观测及两种色块不截断；详情图例所在区域无横向溢出（≤1px舍入），只允许必要纵向滚动。父块5/5、textContent全不能替代子项检查。
4. 展开来源/限制，读取实际内容而不只测折叠summary；原始毫米、样本数、来源与长限制逐块可读（可纵向滚动）。面切换后详情值更新，不重载、不强制关掉详情。
5. 在本组开关前后记录相机/面/选择状态，关闭检查后恢复默认面积。做一次真实Enter开启确认修订driver的字段引用，引用时与原R3输入证据区别记录。
6. 首尾GL/console/page错误、索引23814/三角7938/全网8022等最小烟测，不需要重复实体/地形鼠标oracle、坏包矩阵或20次性能测试。截图须人工逐张看尾部标签/色块完整，记录before R3与after R4的同状态对照。

原始失败若仍有，准确partial；不为了一个绿色总数不断扩断言或补全整套历史矩阵。产品不变的证据可引用，但本轮CSS修复必须现场观察。

## 4. 独立额度与账务纠正

R3旧账纠正：按已披露两次参数拒绝，CLI整线至少14次/原批准12次，+2历史超支保留。缺失拒绝日志不补造；静态执行检查的aux口径说明未决，留注，不以“实质性计数”折扣。

**本轮新增工作额度单列：cargo2 / CLI3 / browser1 / aux2。** 不回收历史额度，不把+2超支偷偷覆盖成额度内。按目前可确认基数，本轮全用后实耗将是cargo12、CLI至少17、browser5、aux至少17；这是累计实际值参考，不是对旧违规追认。

aux推荐一次full前置离线检查+聚焦浏览器，另一次为必要夹具纠正（不额外给浏览器）。若白名单改动或运行失败超出余量准确停，禁止超支后再请求追认。归档G30整线≤64MiB。

每个运行实际argv、cwd、启动/退出记录留档；失败参数调用也计数。拒绝进程不产生包不等于不计CLI。使用自动汇总ledger，不再手写“6成功+2拒绝=6”。

## 5. 交付

交付新包、相关测试/build、3次CLI、聚焦浏览器原始记录与截图、R3未变viewer/model的哈希对照、新预算台账。manifest精确顶层排除、嵌套清单列入、receipt最后绑定；旧文件不动。

只交delivered_for_inspection，不自行宣布G30通过。若整体满足，可由独立检查一次裁定G30限定收口；不先启动G31。A冻结/B总体partial/C不推进。
