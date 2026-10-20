# P1 地形道路阶段验收记录

日期：2026-10-21

状态：**P1 核心切片通过；视觉保留 warn**

## 1. 生成流程

1. 读取 P0 自然高度场；
2. 在山谷、河岸或山口锚点之间建立 16m 搜索图；
3. 每步代价包含距离、地形坡度、转弯角和过水成本；
4. 对路径做两次 Chaikin 平滑，保留起终点和显式 via 点；
5. 用前后向最小 Lipschitz 包络生成不超过 8% 的纵向剖面；
6. 地形支撑段填方不得超过 6m；
7. 超过 6m 的挖谷/架空段必须登记为 `viaduct`，跨水段登记为
   `bridge`，不允许静默填平；
8. 清除道路 footprint 内的树、灌木、草簇、岩石和倒木；
9. 输出 `route-evolution-plan.json`，HTML 读取同一 polyline、道路宽度、
   桥/高架跨段和设计标高。

## 2. 样例

输入：

`examples/geo-bay-route-1km.manifest.json`

输出：

`bevy/work/rk-route-p1-20261021h/`

| route | 直线距离 | 实际长度 | 最大偏离 | 最大纵坡 | 地形填方 | 结构 |
|---|---:|---:|---:|---:|---:|---|
| lake-valley-link | 389.30m | 516.17m | 97.65m | 8.00% | 5.90m | 36m viaduct |
| basin-to-upstream | 409.80m | 487.90m | 80.27m | 8.00% | 5.83m | 16m + 86.57m viaduct |

两段路线都不是直线，实际长度分别比直线多 32.6% 和 19.1%。
总填方体积约 17,232m³；高架段不作为地形填方计入 6m 局部合同。

## 3. 机器与视觉验收

- `scale-audit`：pass，3,386 entities；
- `quality-report`：`warn`，原因是
  `route-ribbon-still-rural-placeholder` 与 `vegetation-placeholder`；
- 同代码复烘焙：108 个可比数据文件 SHA-256 全相同，mismatch 0；
- 战略图可以看到弯曲路线与地形关系；
- 2.5D 探索视图可以看到道路带、中心线和高架支撑；
- 修复了无水体 chunk 被自定义草材质渲染成灰白的问题；
- 没有把视觉 `warn` 写成 `pass`。

## 4. 测试

- 桥梁过水路径必须输出 bridge span；
- 禁用桥梁时必须绕水成功；
- 最大纵坡不超过 8%；
- 地形支撑填方不超过 6m；
- 高架跨度超过上限时具名失败；
- `cargo check -p glyphweave-core --tests` 通过。

## 5. 边界

- 当前 route 阶段仍独立于旧 `road_network`/frontage 生产链；
- viaduct 已有数据和支撑表达，但尚未转换为 P6 的完整
  `EngineeringOperation` 预算/排水/灾害审计；
- 村庄和城市的接入、路口渠化、公交/铁路仍属于后续阶段；
- 当前道路视觉为 8m 乡村道路占位，不代表最终城市道路质量。
