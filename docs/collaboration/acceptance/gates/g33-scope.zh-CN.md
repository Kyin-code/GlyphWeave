# G33 范围与验收合同（范围闸门输出，非实现授权）

日期：2026-09-28（北京时间）。
依据：`handoff-g32-g33.zh-CN.md`、`b01-g32-final-verdict.zh-CN.md`、`dsh-b01-g33-scope-gate-task.zh-CN.md`。
状态：**G33 = `not_started`**。本文件只是范围闸门的输出，描述范围与计划，
**不声称 G33 已实现、已构建或已验收**，也不构成任何实现授权。
配套文件：`g33-baseline-binding.json`、`g33-budget-proposal.json`、`g33-stop-rules.zh-CN.md`。

## 0. 冻结与工作区

- 真实仓库根目录：`G:/difyllmwiki/GlyphWeave/`（`F:/modenmapbox/` 是 Codex 工作目录，**不是**本批协作证据根目录）。
- 冻结只读：G32 推荐包、G32-R2-C1 全部证据、G32-R2 / G32-R1 / G31-R1 目录、G31/G32 产品源码与构建产物。
- 本闸门不修改产品源码、旧包、旧报告或旧清单；不启动 cargo/CLI/浏览器；不生成新的产品包。
- 本闸门本轮只执行只读操作：目录枚举、`Get-FileHash`，以及既有 `verify-delivery.mjs`。全部绑定见 `g33-baseline-binding.json`。

## 1. 推荐用户可见目标（待用户确认）

**在现有 3D 上下文预览中新增一个只读的导航总览小地图（overview minimap）。**

它显示：

1. 已保存窗口的水平范围（world XZ 的范围或等价的保存范围）；
2. 当前相机在水平面上的可视覆盖与朝向（footprint）；
3. 当前相机 target 的位置标记。

用户可以在小地图上做真实点击/拖拽，把相机 target 移到对应位置。语义与 G32 的
Shift+拖拽 pan **完全一致**：只改 `target.x/z`，保持 `target.y`、yaw、pitch、distance 不变；
不加载、不生成、不扩展任何世界数据。

选它的理由：

- 它直接补足 G32 留下的缺口——现在能平移，但用户平移之后会迷失方位；
- 它不需要改动生成模型、几何、拾取求交、材质或光照；
- 它可离线证明（world XZ ↔ 小地图矩形的双向纯函数、相机 footprint 多边形）；
- 它可真实浏览器证明（真实点击/拖拽改变 target、footprint 同步、复位恢复、与 pan 一致、不劫持旋转/缩放/拾取）。

> 该目标**尚未获用户确认**。在用户确认之前，G33 保持 `not_started`，不得据本文件开工。
> 若用户选择备选目标，本文件的 §2–§10 必须重写并重新提交范围闸门。

## 2. 与 G32 的边界（保持冻结、逐字节不变）

除非 G33 新任务书逐项列出新的产品白名单，以下全部冻结：

- **生成模型**：闭集 v1，`render-model-3d.json` = `922d404c…`；字段完整、来源绑定、有限数值、拓扑合法。
- **几何/拓扑**：terrain 4096 vertices / 7938 triangles / 23814 indices；总绘制 8022 triangles；solid draw 7。
- **拾取求交**：terrain/entity/road 使用真实索引三角与保存几何的拾取路径不变。`pickMethod` 仍是**输入来源**字段
  （冻结常量 `"page.mouse.click"`，见交接与 C1 合同 §3），不得拿它当求交算法证明。
- **相机数学**：投影公式、默认 FOV、yaw/pitch/distance 定义、G32 的 pan 数学（`panTargetFromDrag` 等）不变。
- **pointer 生命周期**：`state.pointerGesture` 单一真源、pointerdown 锁 mode、
  pointerup / pointercancel / lostpointercapture / blur 清理、`panCount`/`panMoves` 语义不变。
- **材质/光照/shader**：结构、常量、色带不变（41 个受保护函数与 G31-R1 逐字节相同，shader 结构相同）。
- **读图语义**：方向、图例、保存高程范围保持 G30 收口状态。
- **布局下限**：960×640 ≥0.75、1280×800 ≥0.80、摘要 ≤48px 且 ≤1 行、页面溢出 ≤1px。
- **产品源码绑定**：`g28_viewer3d.js` = `11a2a954…`、`context_preview3d.rs` = `4202ccc3…`；本闸门不修改它们。

## 3. 明确不做

- 不把 ST18、默认世界、自然生成、route/bake/realize、工程安全、全城可达、发布链默认纳入本轮；
- 不改生成模型 schema、不新增世界数据、不新增自然/工程内容；
- 不改 terrain/entity/road 几何、拓扑、材质、光照、shader；
- 不改拾取求交算法，不重解 pan 数学；
- 不新增产品包、不重建 exe（除非新任务书的 build 链明确要求）；
- 不用隐藏控件、删除信息、`notApplicable` 或浮层遮挡地图来换取布局面积。

## 4. 成功 / 失败 / 未观测

三态严格分开，互不替代：

- **成功**：§5 的浏览器场景全部有真实观察记录、§6 的离线断言全绿、几何/错误底线保持、额度未超；
  只交 `delivered_for_inspection`，**不自宣通过**。
- **失败**：出现真实产品缺陷（小地图与 pan 语义不一致、footprint 错误、劫持旋转/缩放/拾取、
  布局或几何回归、错误计数非 0）。交准确 partial，保留失败证据。
- **未观测**：预算耗尽或驱动缺陷导致某项从未被真实观察。必须单列，
  **不得**用离线 fixture、只读探针、旧记录或“已解释”冒充已观测。

## 5. 必须真实浏览器验证的场景

一次真实 Chrome 会话内至少覆盖（两视口 1280×800 与 960×640）：

1. 小地图存在、可见，且不遮挡/不移动画布关键区域；布局下限仍成立；
2. 小地图上**真实鼠标点击**把 target 移到对应位置，位移与独立数学一致；
3. 小地图上**真实拖拽**连续改变 target，松手后 gesture 干净；
4. footprint 随 target/yaw/pitch/distance/canvas 尺寸正确更新（斜视/俯视/yaw≈90°）；
5. 复位后小地图与画布同时回到默认构图；
6. 小地图交互**不触发拾取**、不清空已选对象、不劫持普通旋转/滚轮/侧栏手势；
7. 坏包拒绝与同页恢复；GL/console/page 错误分类记录（均应为 0）；
8. 具名截图（约 9 张）+ 元数据，key 唯一、hash 一致；
9. 几何烟测 23814 / 7938 / 4096 / 8022 / solid7。

## 6. 可离线证明的纯函数与结构合同

优先提取纯函数并单独测试（独立 expected **不得**调用同一个产品函数生成）：

- world XZ ↔ 小地图像素矩形的双向映射（含非零原点、两个尺寸、clamp 与越界）；
- 由相机参数与 canvas 尺寸计算 footprint 多边形/包围盒；
- 小地图点击点 → 新 target 的换算，并逐条满足“只改 x/z，不动 y/yaw/pitch/distance”；
- 退化输入（零尺寸、NaN/Infinity、越界）安全拒绝，不产生伪造 target；
- 结构合同：小地图只做只读叠加，事件只挂在小地图自身，不改变画布的 pointer 监听集合。

## 7. 额度

见 `g33-budget-proposal.json`。建议：cargo ≥2、CLI ≥3、browser ≥2、aux ≥3、归档 32 MiB。
**尚未授予**；授予前不得运行任何产品进程。

## 8. 停止规则

见 `g33-stop-rules.zh-CN.md`。核心：预算耗尽 / 真实产品失败 / 驱动失败三类分开；
驱动失败不得伪装成产品失败，也不得改夹具充当通过；允许一次有明确目标的复测，禁止无限微调到绿色。

## 9. G32 基线绑定

见 `g33-baseline-binding.json`。生产 ID 为 `g32-camera-pan-3-layout-1`；
包含 6 个包文件 hash、源码/exe hash、以及旧轮零漂移期望。

只读核验命令：

```powershell
node G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g32-r2-c1-final-browser/verify-delivery.mjs G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g32-r2-c1-final-browser
```

本闸门实测：`ok: true`、`problems: []`。

## 10. 是否需要新的视觉 intent 与基线截图

**需要。** 小地图是用户可见的观感改动：

- 先写 `visual-intent.zh-CN.md`（观感目标、必须保留的既有观感、明确不做）；
- 以 G32 推荐包为基线，截一组默认/斜视/俯视截图（两个尺寸）作为对照；
- 新包截图必须具名、唯一、带元数据；不得用旧截图证明新版本；
- 截图只作观感对照，不替代几何/行为断言。

## 11. 备选目标（若用户不采纳 §1）

- **比例尺 + 两点测距**：屏幕上拾取两点、给出米制距离与比例尺；离线可证（投影↔世界换算），
  浏览器可证（真实点击两点）。风险：可能与拾取模式冲突。
- **命名视角书签的保存/恢复**：保存/命名/恢复多个相机视角；离线可证（序列化/反序列化），
  浏览器可证（真实按钮）。风险：价值较低，接近已有“复位/预设”。

选择任一备选目标都必须重写本文件 §2–§10 并重新提交范围闸门。

## 12. 授权状态

- G33：`not_started`，仅完成范围闸门；
- 实现、构建、浏览器验收：**未授权**；
- 下一步：用户确认目标与预算后，才可撰写 G33 实现任务书（阶段 B：合同与备份）。

### 更新（2026-09-28）

用户已明确**采纳推荐目标与预算**。随后发布实现任务书
`dsh-b01-g33-overview-minimap-task.zh-CN.md`，实现与验收在
`bevy/work/collaboration/ds-b01-g33-overview-minimap/` 内完成：
真实浏览器 38/38 通过，`delivered_for_inspection`；cargo/CLI 超支作为流程例外单列。
本文件其余部分作为范围闸门的原始记录保持不改。
