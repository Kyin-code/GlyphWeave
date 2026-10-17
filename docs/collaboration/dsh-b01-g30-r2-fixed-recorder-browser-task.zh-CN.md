# DSH B01-G30-R2：集成已验证记录器，补齐遗漏并取得首次真实浏览器验收

日期：2026-09-24。**正式授权，立即连续执行；不再研究未知stub，不索要下一步确认，不进入G31。** 建议由新的验证负责人/新验证任务承接；若原worker继续，也必须按下列具名项实施，而非再写三选一。

独立检查：`G:/difyllmwiki/GlyphWeave/docs/collaboration/b01-g30-r1-inspection.zh-CN.md`。

## 0. 冻结面与目录

候选仍为 `G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g30-map-reading/packages/g30-map-reading-fixed/`，输入期望仍为检查员reviewed-input-bindings，model922d404c… / viewer cd4e0366… / Rust300e151e… / exe7380630f…。所有G30/R1旧包/驱动/日志/manifest/receipt冻结。

新目录：`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g30-r2-browser-acceptance/`。只复制必需的单入口驱动及检查模块，不复制旧全部证据。保持显式--preview/--bindings、run-id与run-root，无扫描/旧包默认值。

允许在真实浏览器发现G30新增UI缺陷后，沿用G30-R1白名单最小修复viewer/template/版本/相关测试、重建新包并复验；几何/拾取/模型/材质/光照/core/G23/pathA不改，ST18/默认世界/自然生成/工程链/发布全0。不为了测试通过改颜色。

## 1. T1：集成已经验证的draw-time快照

复制：
- `G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g30-r1-20260924/draw-snapshot.mjs`
- 对应 `snapshot-fixture.mjs`（调整本轮输出路径，不改断言含义）。

worker driver中drawElements/drawArrays两处用snapshotAttributeBuffers替代Object.assign浅拷贝。**不要复制检查员禁用了浏览器的offline脚本当正式驱动。** 参考检查员snapshot-fixed-offline，仅取这两个记录点的改动；保留正常受授权的浏览器入口。

检查员已实测：原driver离线78/80；只改两处记录快照即80/80，VM27/27；fixture10/10。不是要求再猜测GPU绑定或再加长度筛选。

新增最小记录器回归：同一pos/col对象先上传地形12288 floats并draw，再重传实体108 floats并draw，第一条记录必须仍12288；属性重绑、源数组修改不能改变历史记录。最终面校验只采用该次surface渲染发生的真实indexed draw，保留draw序号/属性buffer id/当时数据长度；禁止无indexed匹配时退回“任意数组长度相同”的draw。

## 2. T2：把色值正负例做实，不将现有80绿当充分证明

固定候选已审核材质常量作为独立expected，CPU图例值与Float32上传值分开比较；上传用明确Float32量化模型，不能任意±1RGB或CPU/GPU两者随便命中即通过。

提取一个清晰的色值校验函数，以同一函数测试：合法final/natural各自通过；把错误面颜色替入、把aPos数据替入aColor、改单个上传颜色分量为明显错误值，都必须拒绝并指出失配项。不是写“错误值不等于预期”一句assert，而要把损坏数据送进真实校验器。0.90的CPU230/Float32229正例保留。不要扩大为新GPU模拟框架。

基础材质色与最终受光照/雾影响的像素分开。拒绝用readPixels像素等于CPU端点作为绕过方案。

## 3. T3：浏览器前一次补齐原任务遗漏

新报告给出T1–T7代码位置/必需check-id/证据路径对应，不以“已修”一句替代。

1. **输出唯一**：1280→960→1280使用step-01/02/03。所有截图先确定唯一键，写文件采用create-new语义；同键或路径已存在即拒绝且旧bytes不变。截图目录原子创建不等于单文件无覆盖。截图索引由实际文件生成。
2. **必需项**：browser required从运行前固定的场景ID完整填充，包括方向DOM、图例DOM、地形点击、真实panel-wheel、坏包无残留、错误汇总。缺项、必需notObserved、重复id、fatal、任何fail均非零。用删除一个场景结果的夹具证实会失败；不能只声明scenarioPlan后required仍空。
3. **合同门槛**：两尺寸浏览地图宽度100%（最多1px舍入）；面积≥80%/≥75%。删除未经批准的“比G29少≤2个百分点”门槛，但照实记录差值。任何新阈值先写明依据，不能临场调成通过。
4. **DOM真值**：从SVG线段端点、标签、退化轴是否省略/提示计算实际方向，与独立相机投影比较；图例的可见节点、隐藏样式/文字与实际图层状态比较。保存范围逐块可见、文本/单位与保存面独立min/max相符。不用state-vs-state或固定说明文字代替画出来的结果。

这些已在上轮明确要求，不是本轮新增产品功能。先通读下一阶段全部场景再启动，不能只修一个错误就消耗一次full调用。

## 4. T4–T7：首次真实浏览器完整观察

同一full调用先冻结绑定→helper/快照/VM验证→真实浏览器；预检全绿后立即继续，不单独交“准备好了”。两个视口/多个页面可在一次会话完成，失败启动也计数。

- **T4 读图/布局**：1280×800和960×640，G29与G30同相机对照，实际矩形/截图；final/natural/斜视/俯视与左右旋转，真实SVG方向、退化标记、图例层状态、保存高程范围。人工查看截图记可读/遮挡/裁剪，不只查textContent。
- **T5 状态与交互**：1280 inspect→960 inspect→1280 inspect，独立step截图和真实展开点击。实体与地形均须page.mouse.click且核对选中类型；地形三角与命中信息不是仅有绘制计数。先选中再拍截图，清空后另拍，不能把已清空截图命名为选中。三次模式往返、选择详情、清空、HUD同步、两面地形单显和既有索引底线保留。
- **T6 滚轮与性能**：用DOM矩形定位面板区域，真实mouse.wheel使scrollTop变化且相机不变；再定位地图内部wheel证明相机缩放。不得用DOM赋scrollTop冒充用户滚轮。记录20次固定操作render耗时/median/p90，readPixels包含则说明，不称FPS。
- **T7 拒绝恢复**：同一页面先加载干净包并真实选中对象→坏包入口→断言modelLoaded=false、drawCalls=0、选择为空、画布清空/拒绝提示可见、坏态截图→干净恢复。新空白页拒绝不能代替已有模型清除。只在新run scratch造坏包，不改冻结包。console/page/GL分类，产品拒绝单列。

浏览器原始记录逐条绑定实际运行driver、产品包与截图。没有完成的项不得写成passed，出现UI真问题走下面修复分支。

## 5. 产品分支与终点

**产品不变**：沿用冻结最终exe、95/95/core82/82/c05 audit历史证据，不重跑cargo。用剩余CLI在新目录重复导出最终fixed版本，与冻结候选model/viewer/html比较确定性（首包旧viewer的确定性不能代替）。

**出现真实G30 UI缺陷**：在剩余cargo3/CLI4内完成相关单测/构建、新版本ID、新包export/audit/必要确定性、浏览器复验。旧包不换文件。不得将不合格布局称“可选优化”绕过合同。

全部必需观察成立与闭集完成后只到delivered_for_inspection，独立裁定另行做。预算不足准确partial并列具体未观测；不承诺“一次必闭合”。不再出现仅修recording然后停止而浏览器额度完全不用的自选停点；真实失败/额度耗尽除外。

## 6. 预算与验证归属

本次只新增aux3，其他类别不追加：

| 类别 | 累计上限 | 累计已用 | 本次可用 |
| --- | ---: | ---: | ---: |
| cargo | 9 | 6 | 3 |
| CLI/服务入口 | 9 | 5 | 4 |
| 浏览器启动尝试 | 3 | 0 | 3 |
| aux | 12 | 9 | 3 |

G30整线归档仍≤64MiB；旧账、旧失败与两次partial不重置。

根因定位与最小记录器修法由检查员已完成，执行者负责集成、补齐明确遗漏并实际运行；若有条件，交给另一验证负责人接手，不必更换产品实现。禁止又列“研究调用序/改像素比较/换工具”三选一要求用户裁决。记录器若出现新异常，先对照提供的10项fixture与实际快照记录，不再仅改筛选猜测。

推荐第一次full同时完成预检与首轮浏览器，余两次用于真实缺陷修复复验。一次aux调用固定一个脚本版本，不可运行中自改再重跑不计数；产品CLI/浏览器进程另计。失败日志/已有截图不删，记录run-id和余额后再启动。

## 7. 交付

新报告自动生成分类计数（harness/helper/package-preflight/VM/browser分别列），提供T1–T7对应表、现场截图、实际面积、失败原因及版本。不要再称53为纯包预检、80为VM或a04只有一红。精确路径manifest包含嵌套清单，receipt绑定最终采用来源。旧报告只在新文档纠正不回写。

A冻结/B总体partial/C不推进。G30 needs_changes，G31未授权。
