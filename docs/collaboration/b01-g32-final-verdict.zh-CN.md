# G32 独立最终裁定：限定接受，保留流程例外，不自动进入 G33

日期：2026-09-28（北京时间）。

## 1. 裁定

**G32 在声明范围内接受为 `accepted_scoped_with_process_exceptions`。**

接受的是 `g32-camera-pan-3-layout-1` 及其 G32-R2-C1 证据闭环，不是对未声明能力的扩展验收。

- A：接受并冻结；
- B：G32-R2-C1 `delivered_for_inspection` 转为限定接受；
- C：**不自动进入 G33**，等待新的明确产品目标和任务书。

推荐保存窗口：

`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g32-r2-layout-closeout/cli-runs/c01-export/`

G32-R2-C1 目录：

`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g32-r2-c1-final-browser/`

## 2. 接受依据

独立只读检查确认：

- C1 真实浏览器会话 97/97 通过，96 项 required 无失败、无未观测；
- controls：browse 12/12、inspect 13/13、展开组 5/5，真实点击改变 yaw/preset；
- terrain pick：点击点在当前 canvas 内且边界 ≥2px，独立光线投射最近命中 terrain，真实点击返回 `terrainTriangle`，pickCount 恰增 1；
- 默认布局：960×640 = 0.7877，1280×800 = 0.8310，摘要 43px/1 行，溢出 0/0；
- 既有回归保持：23814/7938/4096/8022/solid7、真实 pan/实体与地形拾取、取消/失焦/第二 pointer、模式/图层/双 surface、坏包拒绝与恢复；
- GL/console/page error 均为 0；截图闭集唯一且 hash 一致；
- 输入包启动前后漂移 0，G32-R2 77 文件和 G32-R1 75 文件漂移 0；
- 独立 `verify-delivery.mjs` 返回 `ok=true`、22 个 JSON 可解析、manifest/receipt/delivery 绑定成立。

## 3. `pickMethod` 口径裁定

接受本轮的口径修正，不视为放宽：

- viewer 的 `state.pickMethod` 在 `pick()` 中固定写为 `"page.mouse.click"`；
- 该字段表达输入来源，不表达内部求交算法；
- 因此 `pickMethod === "rayTriangle"` 对冻结 viewer 永远不可能成立；
- 本轮使用 `pickObject === "terrainTriangle"`、独立 Möller–Trumbore 最近命中、真实鼠标点击和 pickCount +1 共同证明地形三角拾取。

这比强行要求错误的内部标签更准确，且没有削弱真实运行证据。

## 4. 流程例外

限定接受不抹去历史问题：

- G32 初轮 cargo 误用/重复消耗仍保留；
- G32-R1 aux 2/1（+1）保留；
- G32-R2 aux 2/1（+1）保留；
- G31-R1 历史 `build-archive.cjs` 缺失仍登记为既有缺口；
- 本轮 C1 自身 aux 1/1、browser 1/1，未超支。

这些是过程与历史证据例外，不改变当前 G32 产品范围内的功能结论，也不扩展验收到真实城市、ST18、工程安全、全城可达、route/bake/realize 或发布能力。

## 5. 下一步

G32 不再追加修复或重复复测。下一步应先单独写出 G33 产品目标/范围/验收合同，再决定是否授权实现。

在 G33 任务书出现前：

- G32 包保持只读基线；
- 不改 G32/R2-C1 证据；
- 不启动 cargo、CLI 或浏览器；
- 不把 G33 设计意图写成已授权实现；
- G31/G32 仍不代表工程生产链已经接通。
