# 生鲜市场落地审计与失败具名

日期：2026-10-21

状态：生产路径已接入报告；1km 控制保持 3 个市场实体、AccessGraph 全连通。
市场专用入口工程尚未完成，不能把本记录当作“市场完整入口已完成”的声明。

## 1. 本轮目标

市场计划此前可以显示 5 个候选，但最终场景只剩 3 个，缺失实体没有独立报告。
本轮先把市场实体纳入与普通建筑相同的最终裁决观测，回答：

- 计划了几个市场；
- 最终实际保留几个；
- 每个缺失市场是谁；
- 缺失发生在空间落地还是入口合同；
- 对应的 AccessGraph 状态是什么。

## 2. 新增产物

每个启用 `marketCatchment` 的场景输出：

`scenes/<sceneId>/market-materialization-report.json`

字段：

- `plannedMarkets`
- `materializedMarkets`
- `omittedMarkets[]`
- 每项包含 `entityId / tier / kind / worldX / worldZ / widthM / depthM /
  capacity / load / reason / accessStatuses`

失败市场仍按原严格 AccessGraph 裁决删除，不会留在实体索引中；但失败记录
不会随删除消失。

## 3. 1km 控制结果

输出：

`bevy/work/rk-x/v3-market-audit-1km-r5/`

| 项 | 结果 |
|---|---:|
| 市场计划 | 5 |
| 最终市场 | 3 |
| 最终 AccessGraph 连通实体 | 13 |
| 最终 AccessGraph 断点 | 0 |
| `scale-audit` | pass |
| `quality-report` | pass |

失败市场：

| 市场 | 类型 | 位置 | 原因 |
|---|---|---|---|
| `market.0` | 社区菜场 | `(430,290)` | `anchor_not_on_building_boundary` |
| `market.3` | 邻里超市 | `(472,216)` | `entrance_step_requires_engineering` |

另外两个街道市场和一个邻里超市保留。

## 4. 已尝试但未采用的方案

尝试过把市场直接改成“frontage pad + 正立面 + 短引道”：

- 给市场设置 `frontageRoadPoint`、边界锚点和裙房 cut/fill；
- 生成市场专用 approach；
- 在 1km 控制上重烘。

结果 5 个市场全部被入口合同拒绝。主要原因不是放宽阈值即可解决：

- 市场位于山坡道路侧向 12m 后，入口侧自然地面常高于道路 5–8m；
- 普通 25% 坡道在该距离内不足以消化高差；
- 盲目切出平台会把市场从坡地建筑变成深挖基坑；
- 以固定切深硬塞会失去“先自然、后工程”的因果。

因此该实验已从生产路径撤回，不提交为“已完成入口工程”。正确后续必须生成
分级入口连接：landing + stairs/switchback 或可审计台地，并进入统一工程账本。

## 5. 后续验收条件

市场完整入口至少需要：

1. 占地、裙房、卸货面和入口分离；
2. 入口高差超过 0.3m 时，不允许 render-only 台阶；
3. 优先用可走坡道；超过 25% 时生成真实楼梯/折返楼梯或台地；
4. 工程前后 AccessGraph 都保存；
5. 土方、挡墙、台阶和排水进入统一宏观工程账本；
6. 失败市场在 `market-materialization-report.json` 中具名；
7. 视觉上能从街面识别市场入口和卸货面，而不只是一个普通盒子。

当前只完成第 6 项和普通市场的 AccessGraph 观测；其余仍未完成。

## 6. 验证命令

```powershell
$env:CARGO_TARGET_DIR='bevy/target-main'
cargo test --manifest-path bevy/Cargo.toml `
  -p glyphweave-core --lib market_catchment::tests

cargo run --manifest-path bevy/Cargo.toml `
  -p glyphweave-cli -- generate-world `
  examples/natural-evolution-core-city-1km.manifest.json `
  bevy/work/rk-x/v3-market-audit-1km-r5

bevy/target-main/debug/glyphweave.exe scale-audit `
  bevy/work/rk-x/v3-market-audit-1km-r5

bevy/target-main/debug/glyphweave.exe quality-report `
  bevy/work/rk-x/v3-market-audit-1km-r5
```
