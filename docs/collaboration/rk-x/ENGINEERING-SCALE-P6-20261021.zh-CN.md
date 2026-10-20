# P6 土地压力与工程强度验收记录

日期：2026-10-21

状态：**宏观工程预算切片通过**

## 1. 连接关系

P6 使用 `urbanSlice` 输出的城市规模与特殊因子：

```text
settlementScaleFactor
+ specialFactors
→ landPressureIndex
→ engineeringScale
→ bridge / viaduct / embankment budget
```

工程系数只影响宏观工程预算和候选优先级，不放宽局部建筑、道路、入口、岸线
或步行合同。

## 2. 样例

输入：

`examples/geo-bay-route-1km.manifest.json`

输出：

`bevy/work/rk-engineering-p6-20261021a/`

| 项目 | 值 |
|---|---:|
| settlementScale | town |
| landPressureIndex | 0.5900 |
| engineeringScale | 0.9515 |
| embankment operations | 2 |
| viaduct operations | 3 |
| approved operations | 5 |
| rejected operations | 0 |

工程操作示例：

- `lake-valley-link.embankment`：516.17m × 8m，最大填方 5.898m，
  需求体积 9,776.29m³，预算 18,604.28m³；
- `lake-valley-link.viaduct.0`：36m × 8m，最大净空 8.579m，
  需求结构体积 790.67m³，预算 3,288.38m³；
- `basin-to-upstream.viaduct.1`：86.57m × 8m，
  需求结构体积 1,937.50m³，预算 7,907.27m³。

## 3. 固定硬合同

`engineering-scale-plan.json` 显式重复以下合同，任何规模因子都不得修改：

- 最大填方 6m；
- 最大道路纵坡 8%；
- 最大步行坡 25%；
- 最大入口引道 2m；
- 退界 8–32m；
- 岸线净距 4m；
- 局部 pad 削方 2m。

## 4. 验收

- `engineering-scale-plan.json`：status pass；
- `scale-audit`：pass；
- `quality-report`：pass；
- 战略图/近景：pass；
- 同代码复烘焙：113 个可比数据文件 SHA-256 全相同，mismatch 0；
- 测试证明大都市压力预算高于小镇，而两类城市局部合同完全一致。

## 5. 边界

- 当前工程计划连接了路堤和高架预算，尚未把所有建筑基础、排水、挡墙和
  生态影响统一成同一宏观项目账本；
- 现有 C-E1 挡墙/涵洞和 C-E2 工程后水文仍由旧专项合同负责；
- engineeringScale 目前主要作用于预算与批准，不是自动生成任意挖山/填海；
- 宏观工程真实地形执行、灾害与生态审计仍需继续统一。
