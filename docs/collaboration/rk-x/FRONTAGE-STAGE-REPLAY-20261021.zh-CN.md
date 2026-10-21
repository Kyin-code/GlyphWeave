# 沿街供给跨阶段重放一致性

日期：2026-10-21

状态：**已修复；Core 定向回归通过**

## 1. 问题

1km 发布世界的 `road-network-plan.json` 已完成道路纵向剖面和
`roadCutMm` 回写后，只读 `frontage_supply::diagnose` 重新搜索沿街候选时，
只有一个商店候选的拒绝原因与初始烘培不一致：

```text
published: existing_support_changed = 13
diagnosis: existing_support_changed = 12, entrance_fill_limit = 1
```

生产世界从最终道路计划重跑 `frontage::append` 也出现同一差异，说明问题不在
“诊断器与生产算法”本身，而在“最终计划被误当成 frontage 输入快照”。

## 2. 根因

正式生成顺序是：

```text
road network / extensions / automatic roads
→ refuse overfilled roads
→ frontage::append
→ publish plan.roadCutMm
→ pad ring / development / bake
```

`roadCutMm` 是 `frontage::append` **之后**才写回的计划字段。离线重放从最终
计划出发时保留了该字段，使道路实体获得了当时尚未发布的削方权限。候选几何
没有变化，但工程组合和拒绝优先级改变，最终把同一个候选从
`existing_support_changed` 改名成 `entrance_fill_limit`。

此外，只读诊断在提交候选时漏掉了生产路径已有的
`sample_natural`，使已接受地块的 `naturalMinM/naturalMaxM` 记录不完整；
既有实体还必须按生产使用的 `profile_roads` 顺序构造，不能直接使用未排序的
最终实体顺序。

## 3. 修复

1. `diagnose` 在克隆最终计划后清除后置字段 `road_cut_mm`，恢复 frontage
   实际看到的前置快照。
2. 诊断提交候选前执行 `sample_natural`，与 `frontage::append` 的提交记录
   一致。
3. 诊断的既有工程集使用 `frontage::profile_roads`，与生产道路遍历顺序一致。
4. 支撑审计和 bounded-cut 离线重放同样清除后置 `roadCutMm`，不允许把最终
   阶段权限倒灌到早期候选搜索。

没有合并拒绝原因，没有修改测试期望，也没有放宽任何工程门限。

## 4. 回归

定向测试：

```text
diagnosis_of_a_published_world_reproduces_its_frontage_report  pass
support_gate_audit_of_a_published_world                         pass
real_stages_never_trade_existing_identities_for_new_ones        pass
nonzero_engineering_control_keeps_natural_and_final_distinct    pass
a_recomputed_graph_reaches_every_assigned_street...             pass
```

完整 Core 实跑：

```text
880 passed; 0 failed; 34 ignored
```

最新 1km 控制世界：

```text
bevy/work/r31/1km-b8-cut-r32o-audit-replay
```

| 指标 | 结果 |
|---|---:|
| 实体 | 89 |
| 房屋 / 商店 | 4 / 16 |
| 路径尝试 | 380 / 422 |
| AccessGraph | 31 连通 / 0 断点 |
| 最大跨 chunk 高度差 | 0.148m |
| scale-audit | pass |
| road-network-audit | pass |
| quality-report | warn（旧探针缺地标与完整土地实现） |

## 5. 边界

本次修复保证“跨阶段重放使用阶段当时的输入”，不表示：

- 固定数量沿街请求已经退出正式城市生产；
- 新旧城连续街墙、拆合父子账本或多层级市场已经完成；
- 该旧探针的 `quality-report=warn` 可以当作最终视觉通过。

下一中型模块继续做同一旧骨架上的新旧城街区表达，重点是连续街墙、
保留/拆合/填充、地块多建筑和街面用途，而不是增加固定建筑数量。
