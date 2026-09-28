# G33-R1：小地图语义、视域与手势正确性集中修复

日期：2026-09-28。承接用户“检查并推进下一步”及此前放宽资源的要求，直接推进本收窄修复，不重新申请单次 aux/browser。G34 不启动。

## 0. 输入、输出、边界

真实仓库 G:/difyllmwiki/GlyphWeave/。
新工作证据目录 G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g33-r1-minimap-correctness/。
G33旧交付只读：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g33-overview-minimap/cli-runs/c02-export/。
回退基线 G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g32-r2-layout-closeout/cli-runs/c01-export/。
检查反例 G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g33-20260928/。

允许修改 G:/difyllmwiki/GlyphWeave/bevy/crates/cli/src/g28_viewer3d.js 的G33新增模块/事件/绘制挂钩，以及同目录 context_preview3d.rs 的小地图HTML/CSS/ID/测试。新ID g33-overview-minimap-2。
model、主图拓扑、主图ray/pick、G32 pan数学/生命周期、材质/光照/读图语义、core/G23/路径A冻结；不接ST18/默认世界/自然生成/route/bake/realize/发布；不提交Git、不派子代理。
允许继续使用右下140×140覆盖式小地图，但必须量出覆盖面积，不再称“无覆盖/零代价”。不新增其他浮层。

## 1. P0：先修验证器与基线封存

- 原字节备份与hash；保留旧记录，新目录记更正，不回写G33。
- run-id参数化、目录create-new、已存在拒绝；禁止覆盖，截图写入wx；执行前保存本次driver副本/完整hash。
- required场景集合在执行前固定；重复/缺项/fail/fatal/notObserved/未经授权N/A均使进程非零；离线自测“0 fail但缺项”必须失败。
- 每个真实cargo/产品CLI/browser启动、验证driver都记账。探针/scratch/authoring也计实际运行，区别用途而非豁免。
- cwd/参数/输出父目录用读取和静态检查确认，不用启动cargo/CLI试错；所有stdout/stderr/exit立即保存。

## 2. P1：方向与有效输入

- 北向上、X东向右、Z南向下；双向映射 py=(z-minZ)/depthM*size，逆变换对应，四角与轴增量独立断言。
- 尺寸、window原点/宽深、px/py、target严格有限数值；宽深/size必须>0；target恰好三个有限数。无效返回null且不改变状态，不用1替代无效宽深。
- 映射函数可返回窗口外投影供几何使用；交互target入口必须将有效用户单位clamp到[0,size]，明确区分“投影不截断”和“交互夹紧”。测试非零/负原点、非方形窗口、4边4角拖出、往返精度。
- 保持target.y/yaw/pitch/distance不变。

## 3. P1：真实水平参考面视域

- 在有限保存窗口矩形上，按当前相机视锥的半空间裁剪 y=target.y 平面，得到有序多边形。可以用等价且独立可验证的方法；禁止逐点clamp、60倍虚拟远点或仅finite就判通过。
- 确定near/far裁剪语义与主图投影一致；空交集显式empty、非法输入unavailable；二者不得伪造全窗。浅视、水平、仰视均不产生NaN或假覆盖。
- 画出的SVG points/clipPath必须与正确交集一致，不只测试原始四条射线。clipped标志反映实际是否发生窗口裁剪。
- 明示“水平参考面视域，非地形遮挡可见范围”；不是工程/路线证明。
- 必测默认斜视 yaw=.6、俯视 yaw=.6、yaw±90、浅视、仰视、窗口外target；两尺寸；随机相机确定seed性质测试。
- 两个检查员反例：默认2693.6466 vs4092.5945m²、俯视1323.0997 vs4092.6849m²；修复后用独立推导和DOM面积/点集检查，不硬编码这些数替代算法。
- 通用多边形反例：菱形(-10,5),(5,-10),(20,5),(5,20)与[0,10]²正确交集100，逐顶点夹紧仅50。

## 4. P1/P2：小地图事件所有权

- 活动pointer时忽略第二down；move/up/cancel/lostcapture核对owner；blur明确清理全局活动小地图手势。
- 先清理内部状态再释放capture，避免同步lostcapture误归因；记录一次结束，取消不当完成，不发生额外跳转/拾取。
- 主图和小地图不能同时抢占同一导航操作；在G33入口实施隔离，不重构G32主图冻结处理器。
- 测第二pointer的down/move/up/cancel全套、捕获拖出、失焦、取消后下一次点击、modifier释放、非有限输入、已有selection保持。

## 5. 验证与生产链

先离线反例红→修复→绿，失败可在本轮正常修，不再每错一次交partial。
最终候选执行cli bin单测、core engineering_保护、build，逐个进程记账；export→audit四段pass→第二export六文件确定性。保留最终源码/exe/包hash。
一次整合正式浏览器矩阵为目标，允许额外诊断和复跑，全部单独留run目录：
1. 两视口1280×800/960×640，主图矩形比例≥.80/.75；摘要≤48px；另报扣除小地图遮挡后的可交互面积，不偷换口径。
2. 北向刻度与四角真实点击/拖拽，独立expected；拖出四边/角、第二pointer和取消；鼠标动作与受控事件分栏。
3. 实际按钮/手势改变相机，再读取实际SVG points；斜视/俯视/±90/浅视/仰视逐一截图与独立oracle比对。不得仅page.evaluate调用函数当作DOM显示证明。
4. 先真实选实体再操作小地图，selection不变、pickCount不增；小地图外真实pan/rotate/wheel/entity+terrain pick；选点在当前canvas内且不被小地图覆盖；侧栏wheel不改相机。
5. 模式往返、两面、图层、复位；有选择/拖拽历史后加载scratch坏包，主画布/小地图/选择均无旧模型残留，干净恢复后可导航。
6. WebGL2/几何23814/7938/4096/8022/solid7，GL/console/page分类。新增覆盖件不得损害既有功能。
7. 查看截图，确认小地图方向/当前目标/视域区别清楚；纯函数、浏览器中函数求值、真实交互三类分开统计。

## 6. 资源：改为软预算，不再“单次即耗尽”

初始规划上限供估算：cargo实际命令8次、产品CLI8次、Chrome启动6次、验证driver12次、新归档64MiB。可在本轮调整：达到约75%时写实际用量、剩余任务和调整理由；仅因计数用完不得再拆一个C1/C2任务，正常修夹具/复测无需新批准。
两次连续同因失败且无新诊断，必须换方法或请检查方介入，不能原样重试。遇保护面必须变更、需删旧证据、外部付费/发布/工程接口等仍停止请求决策。
历史计数只纠正事实，不因放宽预算而删去事故。管理读文件/枚举/hash不算产品运行；调用Chrome永远算Chrome，不存在“离线浏览器”豁免。

## 7. 一次性交付

新report逐条回应本任务与检查反例；标明产品修复、驱动修复、未观测、历史不可恢复项。旧a01覆盖丢失不能补造；g33-rigor启动单列；CLI按进程校正。
所有报告/日志/budget/facts先完成，再manifest（精确顶层排除自身/receipt/delivery）→receipt→delivery；独立递归枚举包含嵌套manifest、scratch中保留文件与诊断运行；关键日志不能在封存后追加。
成功只交delivered_for_inspection，检查员决定G33采用；未满足明确partial，不宣称G34已授权。不另建泛泛范围闸门，下一步就是完成本集中修复。