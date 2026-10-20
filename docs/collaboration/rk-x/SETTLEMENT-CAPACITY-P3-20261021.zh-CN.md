# P3 容量聚落阶段验收记录

日期：2026-10-21

状态：**容量与需求规划切片通过；实体 AccessGraph 接入仍待后续**

## 1. 解决的问题

旧的 `frontageProgram.requests.count` 把“建 N 栋”当作完成条件。P3 新增
`settlementEvolution`：

```text
住户 / 岗位 / 商业面积 / 服务预算
→ 沿真实路线候选地块
→ 坡度、水体和重叠检查
→ 地块面积 × 楼层 × FAR
→ 可承载住户/岗位/商业/服务
→ 建筑数量作为结果
```

如果没有足够合法容量，生成结果保持 `incomplete`，并持久化
`unmetDemand`，不会复制建筑或缩小合同凑数。

`settlementEvolution` 与旧 `frontageProgram.requests` 同时出现时会直接
失败，避免两套完成逻辑并存。

## 2. 样例结果

输入 Manifest：

`examples/geo-bay-route-1km.manifest.json`

输出：

`bevy/work/rk-settlement-p3-20261021b/`

需求与交付：

| 项目 | 目标 | 交付 |
|---|---:|---:|
| 住户 | 180 | 180 |
| 岗位 | 60 | 60 |
| 商业楼面 | 1,200m² | 1,200m² |
| 服务预算 | 2 | 2 |

地块与建筑：

- 候选地块 93；
- 接受 15；
- 拒绝 78，均有具名原因；
- 建筑类型：10 个商业界面 + 5 个住宅；
- 楼层：3 层 10 栋、4 层 5 栋；
- 总楼面约 16,704m²。

15 不是 Manifest 中的常量，而是需求被满足时的自然停止结果。

## 3. 验收

- `settlement-evolution-plan.json`：status pass；
- `scale-audit`：pass；
- `quality-report`：pass；
- 战略图：建筑沿弯曲路线分布；
- 近景：建筑尺寸和高度由地块与楼层结果决定；
- 同代码复烘焙：109 个可比数据文件 SHA-256 全相同，mismatch 0；
- 测试覆盖：
  - 更高需求生成更多建筑；
  - 岗位需求会生成办公类建筑；
  - 容量不足返回 `incomplete + unmetDemand`；
  - 固定 frontage count 与需求模式冲突时失败。

## 4. 明确边界

- 当前建筑由独立 `settlement-evolution-plan.json` 表达，并直接渲染；
- 尚未把这些建筑转回 `EntityInstance` / AccessGraph 的入口硬审计；
- 当前类型规则是容量驱动的第一版，尚未接入 epoch、中国小区/美国社区、
  教堂/学校/市场等完整功能设施规则；
- 因此不能宣称 P3、P4、P5 已全部完成。
