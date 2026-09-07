# r19：Landcover 候选计划接入及未实现边界

日期：2026-09-06；权威工程 `G:\difyllmwiki\GlyphWeave`。
接续 `20260906-road-first-development-ownership.zh-CN.md`。核心版本为 `worldgen-2026-09-06-r19-landcover-plan`。

**状态：已将地表候选计划接入真实 bake 和独立 CLI；尚未实现实际 Landcover 表面生成，整体目标未完成。**

> 后续 r21 实际地表执行见 `20260906-yard-surface-realization.zh-CN.md`。庭院已不再全部为零实现；本文件 r19 数据作为历史阶段保留。

## 本轮具体实现

在 DevelopmentPlan 之后生成 `LandcoverPlan`，不从渲染结果或旧农田实体推断土地用途。每个记录保留 development sourceCellId、zoneId、精确矩形、用途候选和未实现原因：

| 来源 | 候选角色 | 当前含义 |
|---|---|---|
| 庭院预留 | managed_yard | 需要庭院/路径/地表实现，不等于已铺装或种植 |
| rural 剩余地 | agriculture_candidate | 农业候选，不证明土壤、坡度、水源及农田入口可用 |
| nature 剩余地 | conservation_candidate | 保护候选，不证明生态、洪水或保护边界已经生成 |
| urban 剩余地 | future_development | 待继续街区/道路/建筑规划 |
| unassigned 剩余地 | unmanaged | 未指定用途，不能自动记为自然完成 |

已规划道路、建筑 footprint 不再进入 Landcover remainder，避免土地重复消费。

- `SceneIndex.landcoverPlan` 与 `landcover-plan.json` 保存同一计划。
- `landcover-audit.json` 为运行结果，不能作为通过依据。CLI 在路网和 DevelopmentPlan 独立重建后重新计算 LandcoverAudit；保存的 audit 缺失、伪造或损坏不决定结果。
- 验证完整 remainder 面积、sourceCellId 唯一性、矩形和角色与来源一致。替换 parent、删除区域、重复来源或把 planned 改成 realized 均不能通过。
- `LandcoverAudit` 报告 plannedAreaM2、realizedAreaM2、unresolvedAreaM2。当前没有真实表面生成合同，所以后两者分别为 0 和全候选面积，而不是按渲染为绿色就给完成量。
- strict gate 现在显式包含 `landcover remains unrealized`，没有绕开原有 development/land-use 失败。

当前实现位置仍在 `bevy/crates/core/src/settlement/development.rs`。真正添加地表算子前应拆成独立 landcover 模块，与几何归属、种植/铺装执行、表面 raster 审计分开；不要继续把所有实现堆入该文件。

## 固定种子实测

输入与 r18 相同；完整输出为 `bevy/work/development-r19-512`、`bevy/work/development-r19-1000`。

| 候选面积 | 512×512m | 1000×1000m |
|---|---:|---:|
| managed_yard | 208 | 1872 |
| agriculture_candidate | 131072 | 530816 |
| conservation_candidate | 65536 | 223796 |
| future_development | 62272 | 233404 |
| 候选总面积 | 259088 | 989888 |
| 候选 cell 数量 | 40 | 167 |
| 有 surface realization 证据的面积 | **0** | **0** |
| LandcoverAudit | incomplete | incomplete |
| 完整质量 gate | fail | fail |

道路/住宅实现和面积与 r18 一致：5/10 条道路、1/9 栋临路建筑；土地所有权分区依旧守恒。与 r16 的自然高度场逐块字节对比完全一致。独立 rules audit 与 bake 报告完全一致，两场景均无 rules rejection。

证据文件：`bevy/work/development-r19-verification.json`。

## 验证

`bevy/work/development-r19-tests.log`：Core 271、App 44、CLI 单元 6、实际 bake 集成 8；合计 **329 项通过**。

补充测试：

- 候选计划保持 remainder 但不声称实现；农业/保护标签只是候选。
- 改角色、伪造 realized、删 cell、重复 sourceCellId 或改 source 拒绝。
- CLI 同时篡改 scene 与 landcover-plan.json，必须独立重建拒绝。
- 将保存的 landcover-audit.json 写成非 JSON，CLI 仍按真实输入重算而非相信历史结果。

前端测试、类型检查和 lint 使用 `bevy/work/development-r19-{web-tests,typecheck,lint}.log`；lint 仍有既有警告，不能描述为零警告。目录检查通过。

预览运行于 `http://127.0.0.1:8832/preview/?view=ownership`；3D 使用 `?view=skyline`。采用独立可执行副本，未锁定 Cargo 输出。截图及浏览器报告在 r19-1000 输出目录的 `ownership.png` / `skyline.png` 及对应 JSON。

ownership 图只显示 Rust 土地归属，不把候选用途渲染成已经建成；深色区域仍表示缺乏具体开发/保护实施方案。3D 材质过亮和植被/道路显示问题沿用 r18，不能以本轮截图当作最终视觉验收。

## 下一步不应继续只加 schema

下面必须实现**实际地表变换**，而不是继续只保存“计划”标签：

1. 从小尺度庭院开始，根据最终地基及实际 access pavement，切分建筑、出入口步道、可种植面和工程影响面；发布 surface/landcover raster 或明确 mesh 所有权。
2. 让旧 farmland/pasture 的生成消费 agriculture candidate，并据全地形坡度、水体和通达性决定真实田块；其来源贯穿 surface 输出。
3. 为自然保留候选提供受约束的生境/地表分类；不因没有人造物就宣布自然质量已通过。
4. 从最终表面反算已实现面积，验证地表 ID、用途、实际范围及与道路/房屋/水体的互斥关系，再逐项减少 unresolvedAreaM2。
5. 如候选土地不适宜，不应硬填平或移除道路；记录真实拒绝及可调整目标，而非降低工程规则。

本轮没有接入外部代码/库、没有新增资产、没有新的许可证兼容性结论。自然地貌重构、真实侵蚀候选及许可证审核、地区人文形态、多片区路网和 6×10km 全 bake 均仍在主目标内。
