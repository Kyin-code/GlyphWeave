# DSH B01-G32：相机水平平移与导航恢复

日期：2026-09-27。**正式批准开工，G31已限定验收，不再等旧补证，也不再调光照参数。** 本任务只让现有地图更方便浏览，不生成新地图或新世界。

## 0. 目标与基线

当前viewer有左键拖拽旋转、滚轮缩放、预设/聚焦，但没有明确的地图平移操作。新增 **Shift+左键拖拽水平平移**：用户可以在不改变观察角度的情况下把感兴趣区域移到画布中央；恢复默认可回到保存窗口中心。

基线包：`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g31-r1-entity-layering/cli-runs/c01-export/`（g31-r1-entity-layering-1，viewer265f42c6…，model922d404c…）。旧G31/G30等包及全部旧记录冻结。

新目录：`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g32-camera-pan/`。

允许修改：
- `G:/difyllmwiki/GlyphWeave/bevy/crates/cli/src/g28_viewer3d.js` 的pointer事件状态机、相机目标水平位移、默认恢复及相应状态公开；
- `G:/difyllmwiki/GlyphWeave/bevy/crates/cli/src/context_preview3d.rs` 的已有帮助区域短文案、生产者版本、相关测试。

与G31“相机冻结”不同，本任务明确授权以上导航变化；**相机投影公式、默认FOV、yaw/pitch定义、拾取三角求交、地形拓扑/保存事实、光照材质、读图计算、模型与审核链仍冻结**。不改core/G23/pathA。ST18/默认世界/生成/EngineeringInput/工程build/route/bake/realize/发布均0。

## 1. 用户操作合同

1. 普通左键拖动仍旋转；滚轮仍缩放；没有拖动的普通左键点击仍按原规则选取/清空。
2. Shift+左键从画布内部按下时锁定本次手势为pan，直到释放/取消；中途放开Shift不转换成旋转，不能拖一半跳模式。Shift点击但未超过拖动阈值不得误选对象。
3. 平移只改变 `camera.target.x/z`，保持target.y、yaw、pitch、distance不变；eye由现有cameraEye跟随目标计算。它是导航参考平面，不是补地形高度或保存新world值。
4. 位移从pointerdown时的target和累计dx/dy求得，不能每move把累计delta反复累加。使用实际canvas CSS高/宽与现有FOV/distance换算速度；正反同幅手势应回原处，resize后的相同归一化拖拽应保持一致尺度。
5. 拖动对象应视觉上跟随手势方向，俯视/斜视/旋转90°均验证。允许明确的相机朝向水平基映射，不声称它是鼠标与地形点锁定的精确地表拖拽；不得使用近地平线射线交平面导致无限位移。规则在实现前短说明中写定。
6. 仅一个活动pointer；pointercancel/lostpointercapture/窗口失焦都清理手势，之后普通点击可用。捕获期间pointer移出画布可以完成/取消，不留幽灵拖动。
7. 不劫持侧栏/工具栏/输入框手势。Shift拖拽后不触发pickCount递增、不清空已选对象；面/图层/详情和模式切换保持选择语义。
8. “恢复默认/默认斜视”应恢复窗口初始target、yaw/pitch/distance；不是只复位角度。若原按钮存在不同语义，明确哪一个负责完全恢复并保持文案准确；不新增重复恢复按钮。
9. 帮助写入已有详情/工具说明，例如“拖动旋转；Shift+拖动平移；滚轮缩放”。不加新的常驻大条，不破坏45px摘要与面积下限。无emoji/外部依赖。

平移后可以看到窗口外空场，必须仍只画已保存窗口，不自动加载/生成数据。不能把移到空场解释成模型丢失；恢复默认始终可达。非法/非有限位移安全拒绝并保留原相机，不能通过篡改world坐标补偿。

## 2. 实现前的短合同和数学测试

先在新目录写不超过一页的 `navigation-contract.zh-CN.md`：确定dx/dy到世界XZ的符号、单位、缩放因子、手势阈值、modifier锁定与取消规则。不得根据浏览器结果临场换语义。

优先提取纯函数：给定相机、canvas CSS尺寸、pointer累计delta，返回目标XZ。独立expected不能调用同一个产品函数生成。

必需反例：yaw0/±90°/非零yaw，俯视/斜视，负/非零窗口原点，正反对称，同一归一化drag在两尺寸的尺度关系，极小/零尺寸、NaN/Infinity，不改变targetY/角度/距离，不累计重复位移。

手势测试：普通click、rotate、shift-pan、途中modifier释放、超过阈值后回到起点仍按drag不pick、pointercancel/lostcapture、多pointer无串扰、UI区域无事件泄漏。可用已有VM/事件工具，不再另造泛化浏览器平台。

## 3. 生产与绑定

新buildId `g32-camera-pan-1`；只在白名单源码改动。模型必须逐字节922d404c…，几何/拾取求交/光照参数与G31验收版逐函数/常量比对；新camera target改变属于本任务明确差异，不伪称所有相机代码没改。

相关单测→最终build→新export→audit四段pass→重复export六文件确定性。每个成功/失败/参数拒绝进程都入台账；使用已有已成功的命令形状，不预创建export目标目录、不在失败export后继续audit空目录。

写源码前用显式严格UTF-8读取，原字节备份到本轮目录并确认成功/hash，再局部补丁；备份失败立即停止。禁用依赖宿主默认编码的全文件读写链。新包生成后冻结源码/exe/包绑定，预检比对，不把实际文件现场重算成expected。

## 4. 真浏览器：同会话聚焦新导航与必要防退化

两尺寸1280×800、960×640，final/natural；真实page.mouse配合keyboard按住Shift完成拖拽，不用直接写camera.target冒充用户平移。

至少验证：
- 默认斜视与俯视各一次pan，记录before/after target、角度/距离不变、画布内固定几何点屏幕位移方向、拾取计数不变及截图；独立数学expected与实际位移吻合。
- yaw约90°后平移，说明符号仍正确；正反手势恢复，完全默认复位恢复原构图。
- 已选实体后pan，选择保留；pan后在新屏幕位置真实点实体/地形仍返回正确kind/源字段与三角信息。求交逻辑不改但必须验证输入坐标仍一致。
- 一次普通旋转、普通点击、滚轮回归；真实面板滚轮/点击不影响地图相机；modifier在非画布输入区不被劫持。
- pointercancel与失焦清理用明确受控事件测试并标注，不伪称全部是物理鼠标动作；随后真实点击/drag验证恢复。
- 模式往返、切面后目标保持或按预定合同变化，摘要≤48px/面积≥80%或75%、详情图例无溢出，GL/console/page错误0。
- 最小几何烟测23814/7938/4096/8022/solid7、两面可见；坏包拒绝/恢复一次。保存事实、光照色带不重新追C1-C7视觉阈值。

截图少而具名（默认、pan后、reset后，两尺寸至少各组），不得用列表选择代替鼠标。普通行为不重复G31全套矩阵。功能完整与易用性分别说明，不靠assertion总数宣传进度。

## 5. 独立预算与停点

G32独立额度：**cargo4 / CLI6 / browser3 / aux4 / 归档32MiB**，开工均0。不复用G31/G30余额，不抵扣历史超支。建议2cargo+3CLI完成首候选，保留一次修复链；首次full可在同一固定driver版本内先预检再浏览器。

每次启动前记录已用/上限；任一类别余额不足不得启动，不允许先运行后只作披露。执行夹具/统计/语法检查按事先分类留台账，不以“工具自检”无上限豁免。纯读取、哈希、目录枚举、文档/manifest编排属于管理日志，不需要为了重写manifest再开验证driver。

正常白名单内连续做至交付或明确阻断；不再每改一个函数交checkpoint请求新批示。若需会话恢复，记录下一具体动作与余额，授权不重置。

## 6. 交付顺序与报告质量

1. 所有运行停止后，先生成事实台账和报告；所有数字从原始records计算，不手填比例、文件数和run-id。
2. 最后枚举目录生成manifest，仅精确排除本目录顶层manifest/receipt；报告、预算、日志、nested manifest、handoff（若有）都应列入。
3. 写receipt并做最终只读闭集核对；报告若后续改动，在尚未交付封存前通过纯管理归档更新对应绑定，记录原因。封存交付后任何补充去新目录，不把postManifestDocumentation当新增排除权。
4. 新报告只陈述新导航行为、真实截图/expected、未改事实、失败/预算与未观测。历史测试崩溃/缺日志/超支不复制成已修，旧G31限定通过保持有效。

交delivered_for_inspection或准确partial；不自行宣布工程B完成。A冻结/B总体partial/C不推进。
