# G32 独立检查：纯数学与首次手势成立，但产品手势生命周期阻断

日期：2026-09-27（北京时间）。结论：**G32 当前 `partial_blocked`，候选 `g32-camera-pan-1` 不采用、不作为基线；下一步只修手势生命周期并复验，不进入 G33。**

## 1. 独立核验范围

检查目录：`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g32-independent-20260927/`。

本次只读源码、原始results/intent/receipt/budget、包/源码哈希和截图；未运行cargo、产品CLI、浏览器或worker驱动，未修改G32目录、旧证据或Git状态。

已确认：

- navigation contract 在实现前写入；两个源文件原始备份与冻结R1哈希匹配。
- G32 包的 model、HTML、材质、shader结构及41项保护函数证据与任务边界相符；export/audit/determinism链成功的记录存在。
- `panTargetFromDrag` 独立数学与纯函数case覆盖 yaw0/±90、斜视/俯视、正负delta、两尺寸、对称回位及退化输入；该部分可保留。
- 源码 `g28_viewer3d.js` 的canvas闭包变量 `pointerStart` 在pointerdown赋值后，pointermove/pointerup继续读取；结束函数只清 `state.pointerGesture`，没有清 `pointerStart`。

## 2. 产品阻断成立

G32 a03的27项VM fixture中 **15 pass / 12 fail**。失败包括普通点击、普通旋转、Shift平移、释放modifier、cancel/lostcapture/blur后恢复、第二pointer、非有限拒绝、panCount及新手势等；11项可由同一陈旧`pointerStart`解释，另1项是`panCount`按move而不是完成手势计数。

这不是仅仅“夹具没写好”：代码控制流本身留下旧手势，后续手势无法建立。因为viewer通过`include_str!`编入exe，修复必须重新cargo build、重新export/audit，不能用旧exe或离线fixture绿灯冒充产品修复。

本轮浏览器只走到隐藏“更多视角”按钮处就中止：未展开details导致超时；26个计划项只登记10项、7张图只写4张。3个pick/selection失败已识别为驱动坐标/签名缺陷，不能作为产品失败；其余未观测不得补写为通过。

## 3. 证据和账目口径

G32全局数字必须拆开：

- `g32-a01` browser session真实启动，但只完成部分导航矩阵；
- `g32-a02`/`g32-a03`是无浏览器VM/preflight迭代；a03 27 fixture = 15/27 pass；
- cargo 4/4，其中 `--lib`无目标拒绝和为干净退出码的重跑是执行者自耗；修复链因此没有cargo余额；
- aux 4/4，包含一个参数检查拒绝和两次夹具/fixture迭代；browser 1/3，CLI 3/6；历史G31/G30余额不转入。

不能说G32“实现且基本完成”，也不能说browser覆盖完整。production package `c01-export` 保持不采用。

## 4. 执行质量与“是否不积极”判断

没有证据推断主观懈怠；有真实产品进展，且按cargo耗尽停止是正确的。

但客观存在可避免的执行质量问题：

1. 在执行前没有用正确的bin target，消耗一次cargo；
2. 为了取得干净退出码重复cargo测试，消耗一次cargo；
3. driver没有在真实浏览器前展开默认折叠的“更多视角”，导致大段计划永远未运行；
4. 3个浏览器失败来自自己的坐标偏移/参数签名夹具错误；
5. 余额字段嵌套造成一次参数检查拒绝。

这些不是主观证据，但说明当前协作方式仍需要“先静态检查选择器/余额/fixture输入，再启动Chrome”的硬闸门。不能用“披露了失败”替代执行前质量控制。

## 5. 下一任务与额度

G32产品范围仍授权，发布：`G:/difyllmwiki/GlyphWeave/docs/collaboration/dsh-b01-g32-r1-pan-lifecycle-repair-task.zh-CN.md`。

新增独立额度：**cargo1 / CLI2 / browser1 / aux1 / 归档16MiB**。G32旧cargo4/4、aux4/4和历史失败不重置；不复用G31/G30余额。修复链顺序固定：

1. 只修`pointerStart`生命周期和`panCount/panMoves`语义；
2. cargo build → 新包export/audit → 确定性（CLI最多2）；
3. aux先跑27 fixture，必须先检查隐藏details、选择器、expectedScreen和余额；
4. 一次browser session补齐剩余导航场景、两尺寸、取消/失焦、选择/滚轮/模式/坏包/几何最小烟测；
5. 只读闭集完成后交独立检查。

若cargo修复后仍有产品真实失败，按额度准确partial；不追加第二次调参，不进入G33。

G31限定通过与G30限定通过保持有效；A冻结/B总体partial/C不推进。
