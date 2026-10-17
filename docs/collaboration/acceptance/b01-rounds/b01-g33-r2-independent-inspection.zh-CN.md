# G33-R2 独立检查：产品语义修复成立，但最终证据闭集仍有一项漂移

日期：2026-09-28。结论：**G33-R2 当前为 needs_changes（证据收口），不进入 G34。**

## 1. 产品与运行证据

只读检查确认：

- 接受候选是 `cli-runs/c06-export`，不是诊断会话 a04 的回退包；
- a05 真实 Chrome 会话 35/35、exit 0、packageUsed 明确指向 c06；
- clipped 正类/负类均有真实 SVG 与独立 oracle 对照；
- `clippedToWindow` 不再恒为 true，内含相机返回 false；
- points/status/面积保持；
- 方向、映射、交互 clamp、六半空间裁剪、pointer ownership、选择、主图 pan/rotate/wheel/pick、坏包恢复和几何/GL/console/page 底线均有记录；
- c05/c06 确定性链和 model `922d404c…` 绑定成立；
- G33 首轮、G33-R1、G32-R2、检查方快照在当前记录中报告为零漂移。

因此，**产品修复本身可以保留为候选**，不需要再为 clipped 语义重跑 cargo/CLI/browser。

## 2. 阻断：manifest 包含被事后改写的 verify-delivery.json

当前只读执行：

```powershell
node bevy/work/collaboration/ds-b01-g33-r2-minimap-clip-closure/verify-delivery.mjs bevy/work/collaboration/ds-b01-g33-r2-minimap-clip-closure
```

结果：

- JSON 检查与源断言通过；
- receipt/delivery/manifest 绑定通过；
- 旧轮快照检查通过；
- 唯一问题：`hash drift: verify-delivery.json`；
- 最终 `ok=false`。

原因已由脚本控制流确认：

1. `verify-delivery.json` 已被列入最终 manifest；
2. `verify-delivery.mjs` 读取 manifest 并计算问题；
3. 脚本最后又写回 `verify-delivery.json`；
4. 写回后的 bytes/hash 不再等于 manifest 中的条目。

这不是产品失败，但它直接违反本轮“manifest 后只读核验、零漂移”的交付合同。不能同时声称 `ok=true` 和保留当前 manifest 条目。

## 3. 超预算记录

本轮账目确实区分了尝试数、成功数、Chrome 启动数和 driver 次数；但实际超过软预算：

- cargo 8/4；
- CLI 33/6；
- Chrome 5/2；
- driver 10/6。

原因主要是脚本参数、BOM、路径、旧包绑定等执行缺陷，另有诊断带来的方法修正。用户允许继续使用资源不等于这些超支消失；本报告保留为流程例外，不用它否定 a05 的产品行为证据，但也不将本轮称为预算内完成。

## 4. 独立裁定

- G32：继续 `accepted_scoped_with_process_exceptions`；
- G33-R1：历史 `needs_changes`，不回写；
- G33-R2：产品修复候选成立，但证据闭集 `needs_changes`；
- G34：不启动。

下一步仅做 manifest/receipt/delivery/verify-delivery 的管理收口，不修改产品，不重建，不重新 export/audit，不再启动 Chrome。