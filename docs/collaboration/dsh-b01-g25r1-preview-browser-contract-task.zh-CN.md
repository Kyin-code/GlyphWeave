# B01-G25R1：上下文预览浏览器观察与 viewer 合同裁定

日期：2026-09-22。

依据：
- G25 独立检查：G:/difyllmwiki/GlyphWeave/docs/collaboration/b01-g25-inspection.zh-CN.md
- G25 交付：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g25-context-preview-consumer/

本任务不修改 G24/G25 已绑定源码和证据，先解决浏览器观察口径，再决定当前自包含 2D viewer 是否符合产品合同。

## 1. 两条明确决策路径

在任务报告开头明确选择其一，不得混写：

### 路径 A：接受“只读2D上下文投影”

接受当前自包含 canvas 2D viewer 作为保存场景事实检查器，不称为 G23 的真实3D/WebGL替代。只补正确的浏览器观察和合同文字：

- natural/final 双层切换；
- 实体、道路、节点、access observation 标签；
- 选择结果逐字段绑定源 context；
- 坏包拒绝、重载无重复；
- 明确页面不提供 WebGL、工程可行性或路线安全证明。

### 路径 B：产品合同要求真实3D/WebGL

不得用现有 canvas 2D 结果冒充通过。另行提出最小迁移任务，说明需要复用哪些 G23 viewer/vendor、具体几何和预算；本轮不得顺手重写 G23 viewer。

默认建议先走路径 A，因为 G25 的命题是“保存上下文可核对消费”，不是三维工程可行性。

## 2. 修正浏览器驱动，不改产品语义

只修改本轮新目录中的 driver/assertion：

- JSON 深比较改为对象键顺序无关、数值字段精确比较；不得用 epsilon；
- 禁止声明检测必须排除否定披露，如 `not re-verified`，不能把否定语句误报为 verified claim；
- 鼠标拾取使用已知实体屏幕投影中心或固定身份标记，记录真实 `page.mouse.click`，不得用 `selectById` 或列表选择冒充鼠标命中；
- 记录命中对象的 entityId、kind、worldXYZ、rotation、savedBounds 和 pickMethod；
- 页面重载、图层切换和坏包拒绝保持现有断言。

如果拾取半径仍导致真实点击未命中，准确记录为未完成，不扩大半径、不改几何、不改安全语义。

## 3. 最小浏览器矩阵

在获得新浏览器额度后执行最多 3 次：

1. clean package：双表面切换、对象计数、道路/节点/access observation 标签；
2. 真实鼠标命中：已知实体投影点 `page.mouse.click`，不得走身份选择后备路径；
3. 坏包与重载：坏包空视口/错误条、干净包重复重载无重复对象。

无需重跑 G23 三个工程楼梯场景，不运行 ST18、默认世界或 WebGL GPU矩阵，除非路径 B 获得单独授权。

## 4. 交付与预算建议

本任务仅允许浏览器/辅助驱动和文档更新；默认不改 core/CLI/viewer 产品代码。

建议新授权：浏览器 **3**、辅助驱动 **3**、cargo **0**、CLI **0**。若路径 B 选择真实3D，必须另立产品任务和新预算，不得在本包内改写 G23/G25。

交付顺序：driver → 原始截图/观测 → browser-acceptance → report/progress → 递归 manifest → receipt → 只读核验。旧 G25 目录不覆盖；本轮失败记录保留。

## 5. 停点

正分支：路径 A 的2D只读合同明确，修正驱动后 clean/命中/坏包观察完成 → G25R1 delivered_for_inspection；不宣布G23真实3D或工程安全验收。

负分支：仍为partial，列清未命中/未观察项；不通过扩大点击半径、删除失败记录、改写旧浏览器证据或把列表选择冒充鼠标命中。

**A冻结、B partial、C不推进。**
