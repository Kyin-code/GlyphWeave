# G34-R3 独立检查：重复性已修，T4 证据仍不成立

日期：2026-09-28。结论：accurate partial / needs_changes；不接受 G34 收口，G35 不启动。
本检查读取 R3 源码、任务书、a01 原始记录及报告；没有修改产品、R3目录或旧证据，没有运行 cargo、产品 CLI 或新的产品验收 Chrome。独立证据只写入本检查目录。

## 1. 已确认成立

- T0 写入守卫、依赖路径审计和本轮旧目录快照机制有实质改进。R3 报告声明的是“本轮起始快照到结束零漂移”，这个口径正确；不能扩大成旧轮历史从未漂移。
- T1 旧 stamp 反例 `[1,0,1]` 已复现，修后重复、A→B→A、probe 交错、surface/viewport 返回稳定，方向正确。
- T2 两层单形状 clip 结构是针对 R2 并集错误的正确修法方向；R3 已保存正向像素证据。由于 R3 自己的三角形红反例未复现，当前应保留为“正向像素+结构成立，边界负例仍需独立闭合”，不写成全部像素场景已闭合。
- T3 R3 的局部数据缺失、numerical-degenerate、全未决等分支较 R2 有实质改进，显示数字也不再直接用 undefined 总数。

## 2. T4 的实际问题与口径纠正

### 2.1 隐藏格四个数字没有形成可接受的原子证据

R3 报告记录同一场景出现 DOM 128、快照 hidden 128、手工复算122，并称同一快照 `cells` hidden=0。源码的 `window.__minimapVisibility3d()` 实际发布的是聚合字段、`cellCategories` 与 `hiddenCells`，并不直接发布原始 `cells` 数组；而驱动又单独调用 `__minimapVisibilityGrid3d()` 做复算。当前记录混合了不同调用/不同入口，不能把“同一对象内部矛盾”定为产品缺陷，也不能把128或122作为接受值。

这仍是阻断：最终产品、DOM和独立oracle必须绑定到**同一次渲染的同一快照**。下一轮必须在渲染出口保存不可变snapshotId、camera/canvas/footprint、完整cell records、aggregate hidden、drawn data-cell；验收只比较同一snapshotId的集合。若发现聚合与cell不一致，必须产品非零并停在partial。

### 2.2 960面积被报告错误地当作0.80失败

R3任务书写的是“默认地图面积≥80%/75%”，对应1280×800使用0.80、960×640使用0.75。a01记录960为0.7877，因此按既定双尺寸门槛**通过**，不是低于960门槛的产品缺陷。驱动却把同一场景两尺寸都按 `fraction >= 0.80` 检查，导致错误的notObserved/报告口径。该项应修驱动并拆成两个唯一场景：`layout-1280-floor` 与 `layout-960-floor`；同时继续检查扣除小地图后的交互面积。

### 2.3 坏包拒绝测试使用了无效破坏点

交付页面包含DOM内嵌模型；仅修改旁边的 `render-model-3d.json` 后仍 `modelLoaded=true`，不能证明产品拒绝了坏包，也不能证明产品有缺陷。下一轮须选择真正被页面读取的输入：损坏内嵌模型标记/HTML、viewer脚本或明确包校验字段，并先定义预期。若产品设计允许内嵌模型优先，则测试应验证“外部副本修改不影响页面 + 页面恢复干净”，不要把它命名为拒绝。

### 2.4 几何断言字段错配

a01实际：terrainIndexCount=23814、terrainTriangleCount=7938、surfaceVertexCount=4096、declaredTriangles=7938、declaredVertices=4096、solidDrawCalls=7。驱动却硬编码 `declaredVertices===8022`，所以该红项是检查方字段映射/期望错误，不是产品几何失败。8022必须回溯到明确字段或从required移除；不能用未解释的历史串替代字段合同。

### 2.5 required场景表重复

`layout-both-sizes-and-interactive-area` 在R3 required表中重复。该项违反记录器自身合同；下一轮必须启动前固定唯一场景ID，并对重复ID、同一ID多语义、缺项强制非零。

## 3. 当前接受边界

R3不能交 `delivered_for_inspection`：隐藏集合证据未原子闭合，T4存在notObserved/fail，required表重复，且坏包/几何/面积三个结果中有驱动语义错误。不能用“报告已如实披露”替代重新验证。

## 4. 执行积极性评价

没有证据断言故意造假；R3主动复现了旧stamp问题、承认clip红未复现并停止自宣，这些是积极行为。仍有执行质量问题：把不同入口的计数混写成同一帧矛盾、两尺寸门槛实现错、坏包破坏点与产品读取路径不一致、8022字段无合同来源、required重复。下一轮应先修验收器和证据模型，再决定是否需要产品改动，避免无谓重建。

## 5. 证据位置

R3原始结果：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g34-r3-repeatability-clip/runs/g34r3-a01/
未决说明：.../open-defect-hidden-count.zh-CN.md
本检查完整任务：G:/difyllmwiki/GlyphWeave/docs/collaboration/dsh-b01-g34-r4-atomic-evidence-closeout-task.zh-CN.md