# DSH B01-G33：导航总览小地图（实现任务书）

日期：2026-09-28。依据用户 2026-09-28 明确采纳 `g33-scope.zh-CN.md` §1 的推荐目标与
`g33-budget-proposal.json` 的预算。**本任务书是实现授权**；在此之前的范围闸门四份文件已完成。

## 0. 冻结与基线

- 唯一基线包：`bevy/work/collaboration/ds-b01-g32-r2-layout-closeout/cli-runs/c01-export/`
  （生产 ID `g32-camera-pan-3-layout-1`，6 文件 hash 见 `g33-baseline-binding.json`）。
- G31/G32/G32-R1/G32-R2/G32-R2-C1 全部目录只读，不得回写。
- 新证据目录（不得复用旧目录）：`bevy/work/collaboration/ds-b01-g33-overview-minimap/`。

## 1. 目标与合同

实现导航总览小地图。产品合同、白名单、规格、纯函数规格、验收判据见
`bevy/work/collaboration/ds-b01-g33-overview-minimap/g33-contract.zh-CN.md`；
观感目标见同目录 `visual-intent.zh-CN.md`。

## 2. 允许修改

只允许：

- `bevy/crates/cli/src/g28_viewer3d.js`（新增小地图纯函数/绘制/指针处理/只读导出；41 个受保护函数与 shader 不动）；
- `bevy/crates/cli/src/context_preview3d.rs`（新增 `#minimap` 元素与 CSS、生产 ID、具名静态测试）；
- 新证据目录下的 driver/modules/fixtures/记录/报告/manifest/receipt/budget/command-log。

禁止：改模型、几何/拓扑、拾取求交、相机数学、pointer 生命周期、材质/光照/shader、读图语义、
core/G23/pathA、ST18/默认世界/自然/route/bake/realize；改任何旧包/旧证据/旧 manifest。

## 3. 生产链顺序（固定）

```
合同 + visual intent + 字节备份（已完成）
→ 纯函数与静态闸门（离线，必须先绿）
→ cargo test + build（viewer 经 include_str! 进 exe，必须重新 build）
→ export-context-preview3d → 新包
→ audit-context-preview3d（四段 totalPass）
→ 可选：确定性重复 export
→ 一次完整真实浏览器会话
→ facts/report/budget/command-log → manifest → receipt → 只读闭集核对
```

## 4. 额度（用户已确认）

cargo 2 / CLI 3 / browser 2 / aux 3 / 归档 32 MiB，均从 0 起算；历史超支不追认、不抵消、不重置。
每次真实进程启动前必须写入并打印扁平余额。

## 5. 停止与交付

见 `g33-stop-rules.zh-CN.md`。成功只交 `delivered_for_inspection`，不自宣通过；
失败交准确 partial 并分栏产品/驱动/未观测。
