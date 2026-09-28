# G32-R1 独立检查：生命周期修复成立，但布局门槛阻断收口

日期：2026-09-27（北京时间）。
结论：**G32-R1 为准确 partial，不接受为通过，不进入 G33。**
G31 的 `g31-r1-entity-layering-1` 继续作为已验保存窗口基线；`g32-camera-pan-1` 继续拒绝采用。

## 1. 检查范围

本次只读检查：

- 产品交付目录：`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g32-r1-pan-lifecycle-repair/`；
- `report.zh-CN.md`、`results.json`、`browser-record.json`、`facts-ledger.json`、`budget.json`、`delivery.json`；
- 当前 viewer/Rust 源码和 R1 生产包绑定；
- 未修改产品源码、旧 G32/G31 证据、Git，也未启动 cargo、CLI 或浏览器。

## 2. 可以接受的部分

以下事实成立，可保留为 R1 的部分交付证据：

- `pointerStart` 闭包状态已删除，源码引用数为 0；活动手势统一由 `state.pointerGesture` 管理；
- `panCount` 在完成的 pan 手势出口计数，`panMoves` 按实际应用步数计数，拒绝手势最多计一次；
- 改前字节备份、41 个保护函数、shader 结构、model、HTML、材质和拾取/几何保护均有绑定；
- cargo 测试 98/98，构建成功；export 成功；audit 四段 `totalPass=true`；
- 28 项离线 VM fixture 与 preflight 通过；
- 唯一真实 Chrome 会话完成 80 项检查中的 74 项，GL/console/page 错误为 0；核心生命周期、取消/失焦、第二 pointer、坏包恢复、几何底线均有真实证据。

这些只能证明“生命周期修复本身基本成立”，不能覆盖以下失败项。

## 3. 六项失败的独立分类

### 3.1 四项是驱动夹具缺陷，不是产品结论

以下项目使用了会话初始 canvas rect。更多视角展开、布局稳定后实际 canvas 高度为 639.8px，但夹具仍使用约 647.03px 的旧 rect，导致约 1.4% 的投影尺度误差：

- `terrain-pick-after-pan`；
- `top-pan-matches-independent-math`；
- `pan-after-rotate-ninety`。

`old-position-does-not-hit-the-entity` 的负对照位移约 9.4 CSS px，而实体在该深度约占 22px，旧点仍落在同一实体内。这个负对照太弱，不能作为产品失败，但也不能保留为通过。

### 3.2 selection-survives-pan 是夹具断言错误

驱动在第 577 行先保存 `selectedBefore`，随后第 578 行又真实点击实体，增加一次 pick，再保存 `reselected`；第 585 行却将平移后的 pickCount 与点击前的 `selectedBefore.pickCount` 比较。

因此记录的 `2 → 3` 与该驱动序列一致，不能推导出产品在 pan 期间额外 pick。正确断言应比较：

- selection key：`reselected.selection === selectedAfter.selection`；
- pan 不产生额外 pick：`selectedAfter.pickCount === reselected.pickCount`；
- panCount：加一。

该项当前仍记为未通过，直到修正版驱动实际执行；不能把离线解释冒充浏览器通过。

### 3.3 layout-floors 是真实产品缺口

实际浏览器记录为：

- 960×640：visible canvas area fraction = **0.7046**，合同要求 ≥ **0.75**；
- 1280×800：visible canvas area fraction = **0.7997**，合同要求 ≥ **0.80**；
- summary 高度 45px、单行、无页面溢出；
- G31-R1 冻结参考为 0.761 / 0.8088。

因此本项不能归因于驱动，也不能以“比其他版本略好”替代合同。G32 的导航界面使现有顶部布局在 960 宽发生额外换行；这是实际可见回归/缺口。

## 4. 流程与预算

R1 记录了 aux **2/1（+1）**。该超额已披露，但不能被视为按授权预算全绿，也不能用“只有一次真实 Chrome 启动”抵消 aux 超支。

本次交付目录的 manifest/receipt/闭集可以作为管理证据；但它们不能把 74/80 改写为通过，也不能把夹具解释改写为浏览器观察。

## 5. 独立裁定

- A：生命周期修复部分成立，保持冻结候选，不直接采纳为最终 G32；
- B：总体 partial，布局门槛仍未满足；
- C：不推进 G33。

下一轮必须是一个收窄的“布局修复 + 证据夹具修正”任务，不能重新设计 pan，也不能重跑无关的几何/材质工作。
