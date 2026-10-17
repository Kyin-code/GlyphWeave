# DSH G29-R1-C1：控件展开与可达性聚焦补证

日期：2026-09-24。**正式批准立即执行，仅驱动与证据收尾，产品冻结；不重建、不重新导出、不重跑整套CLI。**

检查依据：G:/difyllmwiki/GlyphWeave/docs/collaboration/b01-g29-r1-controls-inspection.zh-CN.md。

## 0. 已接受与本次只做什么

已接受G29-R1生产者build id、新包/模型绑定、坏包当时拒绝和恢复、原子run目录及隔离自检。保留已有几何/HUD/模式往返/重载证据，不再以另一份全量176项脚本重新调试它们。

本次唯一产品验证缺口：两尺寸两模式下，折叠控件真实展开后的可达/点击、检查面板滚动后的控件、完整选择详情。先修驱动D1—D4及检查员发现的三处问题，再用冻结新包执行一次聚焦会话。

## 1. 明确预算（不是申请草案）

G29累计已用cargo6、CLI9、browser3、aux7；原累计上限6/9/4/7。

本次只新增aux3，其他不新增。现在可用：**cargo0、CLI0、浏览器1、辅助3**；新累计上限6/9/4/10。新证据≤32MiB；历史预算与失败记录不回改。

建议最多：预检1次、需要时修正预检1次、最终浏览器驱动1次。预检/browser分开启动就各算一次aux；不能把两个调用合并记1。第二次预检仍不通过就停止，不消耗最后浏览器去试错。

浏览器驱动可在同一进程开多个page/context，仍只1次Chrome；真实启动失败也计1，超限停。按实际工具能力/权限运行，不绕过安全拒绝。若发现产品真实缺陷，先给原始复现和最小修改位置，不擅自改产品后使用旧exe/包验证。

## 2. 新目录与版本隔离

新证据目录：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g29-r1-controls-closeout/。

只读被测包：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g29-r1-final-package/packages/g29-r1-map-first/。

产品绑定：build id g29-map-first-view-1、Rust2018242f…、JS6812141c…、exe99fd458d…、model922d404c…。当前源/包/旧记录不写入。把r2复制到新目录作新revision修复，不修改冻结r2，也不回写a03的14fail。

每次显式新run-id，新目录原子创建；intent先写、结果最后写、stdout/stderr/exit/截图归同一run；已存在拒绝，失败不可覆盖。旧同名截图/运行不得复用路径。

## 3. 启动Chrome前必须修并测试驱动

保留r2对D1—D4的修正，但不能直接拿它当已验证版本。

1. runPrecheck实际会抛`selfTest is not defined`：正确返回harness.selfTest/harness.driverTests，确保precheck能够完整写退出码并exit0。
2. spec输入做形状断言：每项必须是对象且sel为非空字符串。展开段不能传字符串数组，非法输入在启动浏览器前给具体原因。
3. 分类器的visible/visible-clickable/reachable-after-scroll统一；滚动后有效可达不再被展开汇总判失败。保存滚动前后矩形/裁剪/命中对象；原先屏外控件先滚动，不能把夹到屏幕内的假中心用于遮挡结论。
4. 隐藏检查面板在browse为notApplicable；折叠详情标为collapsed待展开。不可把真正的inspect控件或展开子项记notApplicable逃避测量。
5. 快照明确包含selectionDetailsOpen，并复制值，不拿同一个活对象与自身比较。
6. 汇总采用明确requiredChecks集合：缺必需项、重复ID、partError/fatal、必需notObserved、fail都必须非零退出；只有全部执行且满足才exit0。notApplicable仅允许模式规定的隐藏项。不得靠“fail计数0”宣布完整。
7. 以上增加纯夹具用例：异常中止但尚无fail、必需notObserved、漏项、重复项、合法滚动后可达、隐藏面板、输入错误，均验证预期退出/分类。不要靠扫描驱动自身注释里的marker证明变量作用域正确。

执行一次预检完成记录器/分类器/汇总/真实JS VM；包已经存在，严格记录源码与六文件hash。任何预检非零，不启动Chrome。允许一次aux预留用于修复复测；不新增全量产品测试。

## 4. 浏览器只聚焦这些覆盖

基准1280×800、960×640；每个尺寸先进入browse，再进入inspect。每次测量前明确滚动起点，使用真实UI展开，不直接把open设true冒充用户操作。

### A. 更多视角

四种组合均验证`#more-camera`的summary可见、真实点击展开；五个相机子控件逐项测量并实际点击（在需要时恢复基准视角），保存对应相机变化或明确不变原因。不能只记录childrenCount>0或“点过一个”就宣称全部可用。

### B. 检查面板来源详情

browse下隐藏的`#evidence-details`明确notApplicable，不去点击。inspect两尺寸下真实展开；来源/限制属于信息文本，按可读/可滚动验收，不要求非按钮的limitations“真实点击”。需要长内容滚动时保留滚动前后矩形及可视文本片段，不要求长列表一次完全塞进视口。

### C. 面板顶部收起按钮

在两种inspect尺寸下先有意滚动面板，再让`#hud-toggle`scroll into view，实际点击收起，确认回browse，mode与画布宽度符合。不能靠每次重置scrollTop后跳过此回归。

### D. 选择详情

冻结包真实鼠标选实体后，通过卡片summary展开完整字段/派生交点；分别核对details.open、state.selectionDetailsOpen及DOM可见字段。收起/展开不误触pick、选中ID不变。需隐藏/滚动的元素记录具体原因，不拍空卡片当已验证详情。

每项输出selector、role/name、mode/viewport、祖先details状态、computed visibility、矩形/滚动裁剪/命中目标、操作和actual、expected、verdict。对不可点击必须截图和具体元素诊断，不能只一个false。

可在同一会话首尾做小控制：build id/模型hash正确，default browse宽度、23814/7938/8022、HUD最新。已绿的坏包/三次重载等不要求本轮重做，只引用原新包证据；旧证据不是新观察。

## 5. 最终报告与不倒写纠正

用“原14红项→本次检查ID→原始actual/新actual→结论”映射，完整保留原fail。要么实测通过，要么明确产品缺陷/notObserved，不能在运行前保证全是假阴性。

a02原记录是exit1/launched=false、驱动自检marker失败；同run-id拒绝由隔离recorder-selftest证明，不是a02顶层exit2。新报告纠正引用，旧报告/ledger不改。116项里包含旧hud-toggle红项，不再称旧116全部通过。

输出新驱动源码/hash、预检和浏览器run意图/退出码/日志、逐控件原始记录、聚焦截图、原新包只读绑定、report/progress/manifest/receipt。截图内部引用与manifest都核验，不能只清单hash正确而引用错误。小范围≤32MiB，不复制整套旧证据。

## 6. 退出规则

产品源/包不变且所有必需展开/滚动/选择详情均有真实成功证据→delivered_for_inspection，由检查员决定G29限定收口。若仍有fail，指出具体产品/驱动事实并准确partial；无余额则停，不开第2个浏览器或第4个aux。

**立即先修驱动，不改产品；预检过后一次聚焦浏览器补证。不要再提交只有r3代码但未执行却声称“只差一次”的结论。**
