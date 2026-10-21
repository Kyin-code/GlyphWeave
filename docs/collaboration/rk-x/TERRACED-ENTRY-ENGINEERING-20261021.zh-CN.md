# 坡地建筑入口台地与半米量化修复

日期：2026-10-21

状态：生产路径已接入并实烘焙。1km 控制从 12 个最终普通建筑提升到 16 个，
市场从 3 个提升到 4 个，最终 AccessGraph 20 个实体连通、断点 0。

## 1. 发现的两个根因

### 1.1 半米量化误差

`EntityInstance.world_y` 是整数米，而 Heightfield 是毫米精度。旧重贴地逻辑：

```text
world_y = ceil(baked_ground)
```

地面为 `12.5m` 时建筑底面会被写成 `13m`，入口合同随即正确报告
`entrance_step_requires_engineering`。这不是真实台阶，而是数据量化错误。

修复：

```text
world_y = floor(ground)
bottomOffsetM = ground - floor(ground)
```

`GroundingPivot::Center` 和 `roadbedOffsetM` 同样从总 ground height 反推，
避免中心 pivot 或路床偏移丢失。

### 1.2 坡地建筑支撑面取整栋最高点

聚落建筑原先以 footprint 内最高点作为整栋底面。坡地建筑因此常出现：

- 后缘在 20m 等高线；
- 临街立面只有 12–14m；
- 建筑整体被抬到后缘高处；
- 入口检查把 6–8m 高差报告为“需要台阶工程”。

本轮改为：

1. 建筑规划保存 `frontageLevelM`，取该建筑对应道路站点的设计标高；
2. 建筑 pad 以道路标高为目标，生成有界挖填平台；
3. 规划的 `maxPadCutM` 是弱门限；控制切面声明 `12m`，超限候选具名拒绝；
4. 建筑入口锚点落在立面上，`frontageRoadPoint` 以明确朝向进入 pad/apron；
5. 短引道与建筑使用同一地面标高，不再额外抬高 0.5m 路床。

控制场景声明：

```json
{
  "settlementEvolution": {
    "maxPadCutM": 12
  }
}
```

普通城市仍默认 `2m`；该值不是全局绝对上限。特征切面、绝对中心城区、
横琴/东沙式工程应按规划授权扩大，并继续写入可追溯工程账本。

## 2. 1km 控制结果

输出：

`bevy/work/rk-x/v3-terraced-entry-1km-r11/`

| 项 | 修复前 | 修复后 |
|---|---:|---:|
| 期望/规划建筑 | 24 | 24 |
| 最终普通建筑 | 12 | **16** |
| 最终市场 | 3 | **4** |
| AccessGraph 连通实体 | 15 | **20** |
| AccessGraph 断点 | 0 | **0** |
| 实体数 | 3581 | 3594 |
| scale audit | pass | **pass** |
| quality report | pass | **pass** |
| visual feedback | 无 | **pass** |

最终普通建筑构成为：

- 3 个 office tower；
- 3 个 residential block；
- 2 个 residential home；
- 8 个 storefront。

阶段分配仍为旧城 2、更新带 21、新区 1 个规划地块；需求仍完整闭合。

## 3. 仍被拒绝的建筑

24 个规划建筑中仍有 8 个未落地：

- 7 个仍具名报告 `entrance_step_requires_engineering`；
- 1 个报告 `entrance_clearance_blocked`；
- 它们已带 `accessDiagnostics`：
  - worldY / bottomOffsetM / groundHeightM
  - anchor 坐标与 anchor 处烘焙地形
  - frontageRoadPoint 与最终状态

诊断可直接回答“差多少、差在哪、是不是边界”。这些剩余高差不是半米量化，
而是真实坡地入口尚未具备 stairs/switchback/landing 工程。

## 4. 设计边界

- 没有放宽 0.3m 入口台阶合同；
- 没有放宽 25% 步行坡；
- 没有把市场或建筑静默删除；
- 没有用固定建筑数补足需求；
- 有界 pad cut 只在声明额度内使用；
- 引道与建筑只要被拒绝，其孤立引道会一并撤销。

## 5. 后续

剩余坡地入口应进入统一 `landing + stairs/switchback + retaining wall`
工程链，而不是继续调高 `maxPadCutM`。同时需要把 pad 挖填、挡墙和台阶汇总到
宏观工程账本，并让 HTML 用真实立面/入口几何消费这些记录。

## 6. 验证命令

```powershell
node --check adapters/html/app.js

$env:CARGO_TARGET_DIR='bevy/target-main'
cargo test --manifest-path bevy/Cargo.toml `
  -p glyphweave-core --lib baked_rebase
cargo test --manifest-path bevy/Cargo.toml `
  -p glyphweave-core --lib settlement_evolution::tests

cargo run --manifest-path bevy/Cargo.toml `
  -p glyphweave-cli -- generate-world `
  examples/natural-evolution-core-city-1km.manifest.json `
  bevy/work/rk-x/v3-terraced-entry-1km-r11

bevy/target-main/debug/glyphweave.exe scale-audit `
  bevy/work/rk-x/v3-terraced-entry-1km-r11
bevy/target-main/debug/glyphweave.exe quality-report `
  bevy/work/rk-x/v3-terraced-entry-1km-r11
```
