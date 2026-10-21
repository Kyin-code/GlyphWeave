# 生鲜市场最终占地去重叠

日期：2026-10-21

状态：生产路径已接入；1km 控制中 4 个市场保持连通，市场候选不再共享同一占地。

## 1. 问题

市场计划的候选间隔只比较路线节点，没有比较离路 12m 后形成的真实建筑占地。
1km 控制曾出现：

- `market.1` 中心约 `(353,333)`，占地 `36×24m`；
- `market.4` 中心约 `(354,326)`，占地 `48×32m`。

两者节点不同，但最终 footprint 大面积重叠。重叠 pad 的挖填与入口会互相污染，
即使 AccessGraph 各自可达，也不能证明入口和卸货面属于谁。

## 2. 修复

市场选择现在对每个候选计算最终建筑 footprint：

- 使用最近道路站点；
- 应用离路 12m 后的建筑中心；
- 按道路方向旋转；
- 建立世界 AABB；
- 与已选市场保留 2m 净距。

如果候选与已选市场重叠，选择器继续尝试下一个候选节点，而不是先选中再在
渲染时隐藏。

市场尺寸集中到 `market_size(tier)`，计划、占地检查和实体物化使用同一组
宽深高，防止三处漂移。

## 3. 1km 结果

输出：

`bevy/work/rk-x/v3-market-overlap-1km-r12/`

| 项 | 结果 |
|---|---:|
| 市场计划 | 5 |
| 最终市场 | 4 |
| 市场覆盖 | 约 100% |
| AccessGraph 连通实体 | 20 |
| AccessGraph 断点 | 0 |
| scale audit | pass |
| quality report | pass |
| visual feedback | pass |

`market.4` 从重叠位置 `(374,346)` 改选到 `(378,506)`。当前唯一失败市场为
`market.0`，原因是 `anchor_not_on_building_boundary`，已具名写入
`market-materialization-report.json`。

## 4. 边界

- 这是候选 footprint 去重叠，不是市场入口工程完成；
- 2m 是市场之间的最小净距，不替代消防、卸货或街道设计规范；
- 若候选都重叠，应具名失败而不是缩小市场；
- 后续市场仍需要独立 landing、ramp/stairs、卸货面和工程账本。

## 5. 验证

```powershell
$env:CARGO_TARGET_DIR='bevy/target-main'
cargo test --manifest-path bevy/Cargo.toml `
  -p glyphweave-core --lib market_catchment::tests

cargo run --manifest-path bevy/Cargo.toml `
  -p glyphweave-cli -- generate-world `
  examples/natural-evolution-core-city-1km.manifest.json `
  bevy/work/rk-x/v3-market-overlap-1km-r12

bevy/target-main/debug/glyphweave.exe scale-audit `
  bevy/work/rk-x/v3-market-overlap-1km-r12
bevy/target-main/debug/glyphweave.exe quality-report `
  bevy/work/rk-x/v3-market-overlap-1km-r12
```
