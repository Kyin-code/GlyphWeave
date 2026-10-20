# P5 生鲜市场辐射阶段验收记录

日期：2026-10-21

状态：**网络容量与竞争模型切片通过**

## 1. 数据链

```text
P3 settlement-evolution-plan.json
→ 每个建筑成为需求点
→ P1 route-evolution-plan.json polyline 建图
→ 候选市场放在道路节点
→ Dijkstra 网络时间 + 入户步行时间
→ 容量/质量指数衰减
→ 竞争分摊、市场负载、局部供给密度、缺口
```

不使用几何半径判定覆盖。`candidateSpacingM` 只控制候选频率，实际服务范围来自
道路网络时间。

核心公式：

```text
A_ij = capacity_j * quality_j * exp(-beta * travel_time_ij)
P_ij = A_ij / Σ_k A_ik
load_j = Σ_i demand_i * P_ij
rho_i = Σ_j capacity_j * exp(-beta * travel_time_ij)
deficit_i = max(0, demand_i - rho_i)
```

## 2. 样例

输入：

`examples/geo-bay-route-1km.manifest.json`

输出：

`bevy/work/rk-market-p5-20261021b/`

- 需求点：15 个建筑；
- 总需求：260 单位；
- 市场容量：280；
- beta：0.08 / 分钟；
- 最大网络时间：15 分钟；
- 最终市场：1 个；
- 服务需求：260；
- 覆盖率：1.0；
- 最大/最小局部缺口：0 / 0。

市场位于路线节点 `(448,512)`。每个需求点的最近市场、旅行时间、局部供给
密度和缺口写入 `market-catchment-plan.json`。

## 3. 验收

- `market-catchment-plan.json`：status pass；
- `scale-audit`：pass；
- `quality-report`：pass；
- 同代码复烘焙：110 个可比数据文件 SHA-256 全相同，mismatch 0；
- 战略图显示市场锚点与各需求点覆盖状态；
- 近景显示市场建筑体量；
- 测试覆盖：
  - 市场沿道路网络布置；
  - 所有需求点有最近市场；
  - 覆盖率低于目标时 `incomplete`；
  - 非法 beta/容量 fail closed。

## 4. 边界

- 当前只有单一生鲜市场层级和单一网络步行速度；
- 尚未区分菜场、邻里超市、大超市、周期市场、批发市场；
- 尚未接入公交时刻、停车、游客季节性、冷藏容量和配送物流；
- 当前需求来自 P3 建筑计划，而不是完整人口/就业调查。
