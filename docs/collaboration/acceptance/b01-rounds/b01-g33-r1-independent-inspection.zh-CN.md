# G33-R1 独立检查：主体修复成立，但 clippedToWindow 仍有合同缺陷

日期：2026-09-28。结论：**G33-R1 暂不采用，状态 needs_changes；不进入 G34。**

## 1. 已确认成立的部分

只读复核确认：

- G33-R1 最终交付目录 manifest 212 项，逐项 hash/bytes 无漂移；receipt/delivery/manifest 绑定成立；`verify-delivery.mjs` 返回 `ok=true`；
- model 仍为 `922d404c…`；41 个保护函数、主图几何/拾取、G32 pan 数学与 pointer 生命周期、材质/光照/shader 保持冻结；
- 北向方向、有限输入、交互 clamp 与投影不截断已修复；
- 默认/俯视/yaw±90/浅视/level/looking-up 的 footprint 顶点与面积和独立半空间 oracle 一致；
- 第二 pointer、foreign up/cancel/lostcapture/blur 的归属修复已在离线与真实会话中覆盖；
- `g33r1-a04` 真实浏览器 35/35、exit 0；布局、真实 SVG 读取、选择保持、主图 pan/rotate/wheel/pick、坏包恢复、几何和 GL/console/page 错误均有记录。

这些证据足以说明 R1 的三组主要缺陷已经大体修好，但不足以接受当前包。

## 2. 新发现的真实产品缺陷：clippedToWindow 语义错误

产品文件：

`G:/difyllmwiki/GlyphWeave/bevy/crates/cli/src/g28_viewer3d.js`

当前 `minimapFootprint` 在完成窗口四侧裁剪后直接返回：

```js
return { status: "ok", points: windowPoly,
  frustumPoints: poly, clippedToWindow: true };
```

也就是说，只要结果非空，`clippedToWindow` 永远为 `true`，无论视锥多边形是否真正被保存窗口裁剪。

这违反 G33-R1 合同 §3 与 `g33r1-contract.zh-CN.md` §3.7：

> `clippedToWindow` 反映是否真的发生了窗口裁剪。

独立 VM 复现使用最终交付源码：

- yaw = -90°：视锥与窗口结果为完整 64×64 窗口，但产品返回 `clippedToWindow=true`；
- yaw = +90°：同样返回 `true`；
- 视锥完全落在窗口内部的小距离相机：产品仍返回 `true`，而实际未发生窗口裁剪；
- 独立 oracle 对相应场景判定 `clipped=false`。

R1 的 35/35 主要比较了 status、顶点数、面积和 SVG points，没有对 `clippedToWindow` 在“无需裁剪”场景做负例断言；因此这是测试遗漏，不是矛盾证据。

## 3. 账目摘要存在不一致

本轮不同文件的实际数字不一致，需更正但不应回写旧证据：

- `budget.json`：cargo 2、CLI 6、Chrome 6、driver 9；
- `report.zh-CN.md` 某处写成 cargo 1、CLI 4、Chrome 3；
- receipt/facts ledger 也分别使用了不同的摘要口径。

命令日志列出了 2 次 cargo 链、8 个 CLI 进程（4 export + 4 audit）和 4 次正式 Chrome 会话，不能继续用较小数字作为“本轮实用量”。下一轮必须从命令日志/exit 文件生成单一账本，明确区分“调用次数”和“成功产品进程次数”。

这不是产品功能缺陷，但属于证据质量问题，必须修复记录生成方式。

## 4. 独立裁定

- G32：保持 `accepted_scoped_with_process_exceptions` 基线；
- G33 首轮：保持历史记录，不改写；
- G33-R1：`needs_changes`，不采用当前 `g33-overview-minimap-2`；
- G34：不启动。

下一步只修正 `clippedToWindow` 语义与账目/测试闭环，不重新设计小地图，不重跑无关的 G32/G31 全线。

### G33-R1 clipped 反例口径更正（2026-09-28）

补充复核发现：yaw±90 的旧 oracle `clipPolygonToBox` 只比较顶点数量，不能单独作为 clipped=false 的可靠反例；它可能漏掉“顶点数不变但坐标已被裁剪”。真正确定的产品反例是小距离、视锥完全落在窗口内的相机：产品frustumPoints与最终points逐点相同仍返回clippedToWindow=true。R2任务已同步要求修正oracle的坐标变化判定，避免把oracle缺陷带入修复。