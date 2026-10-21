# 新旧城连续街墙、时代继承与供给迭代

日期：2026-10-21

状态：**中型模块完成，Core 与 1km 控制通过**

## 1. 目标

本轮不增加固定建筑数，而是修正三个结构问题：

1. 旧城仍按整个城市共用的粗街区间距取样，窄街宅之间留下不合理的空隙；
2. 同一 epoch 的有限需求被分散到互不相连的道路和街侧，形成孤立建筑；
3. 最终建筑虽有 `districtId`，却没有把 epoch 的 `preserve / modify / replace`
   和父对象写回实体记录。

## 2. 旧城街墙

生成器现在采用窄街取样间距：

```text
frontageStep =
  min(authoringRoadStep, oldCoreLotWidth + 1m)
```

需求仍是唯一建筑数量上限，候选密度只决定街道是否允许连续落位。

时代预算按以下顺序消费：

```text
按 routeId、side、frontagePosition 排序
→ 优先沿同一街侧连续消费本时代预算
→ 旧城只保留至少有两个邻接候选的街段
→ 不允许旧城剩余预算切换到另一条孤立街侧
→ 未使用预算才由 renewal / new-growth 等后续 epoch 承接
```

机器输出新增 `streetFrontage`：

- 每个连续街段的面宽跨度、立面长度、未覆盖长度、最大缺口和覆盖率；
- 旧城连续街段数和最低覆盖率；
- 单栋孤点不能计为连续街墙。

## 3. 时代账本与实体继承

`epoch-evolution-plan.json` 不再用另一套硬半径覆盖
`settlementEvolution.districtStage`。最终建筑阶段直接决定：

| 阶段 | 操作 | 父对象 |
|---|---|---|
| old-core | preserve / modify | historic.<building-id> |
| renewal | replace | historic.<building-id> |
| new-growth | new | 无 |
| periphery | new | 无 |

最终实体新增可审计字段：

```json
{
  "buildingOperation": "replace",
  "buildingParentId": "historic.route-auto-..."
}
```

字段缺省时不序列化，普通道路、自然实体和未启用 epoch 的世界保持旧字节语义。

## 4. 资产合同修正

高层店面原会转换为 `urban_building`，但香港控制 Manifest 没有对应契约，
导致 scale audit 具名失败。补充 `urban_building` 合同：

- 宽/深 6–80m；
- 高 12–120m；
- 仅允许 grounded 在草地、林地、土壤和石材；
- 禁止水体、深水和地下。

## 5. 香港 1km 控制结果

输出：

```text
bevy/work/rk-x/v3-contiguous-epochs-r7-20261021
```

| 指标 | 修正前 | 修正后 |
|---|---:|---:|
| 需求生成的最终建筑 | 35 | 33 |
| 旧城建筑 | 10，分散为 2 侧 | 7，同一条连续街墙 |
| 旧城连续街段 | 0 / 2 | 1 / 1 |
| 旧城覆盖率 | 最低 0.361 | 0.771 |
| 旧城最大立面缺口 | 47.40m | 4.37m |
| renewal 继承 | 账本与最终阶段不一致 | 19 栋全部 replace + parent |
| new-growth | 9 | 7，全部 new |
| AccessGraph | 38 / 0 | 38 / 0 |
| scale-audit | fail | pass |
| quality-report | fail/warn | pass |
| visual-feedback | 未完成 | pass |

需求闭合仍为：

```text
unmetHouseholds = 0
unmetJobs = 0
```

完整 Core 回归：

```text
882 passed; 0 failed; 34 ignored
```

## 6. 视觉检查

运行态检查三个视图：

- `?view=street`：旧城 7 栋沿同一道路连续排列，底层贴路，无空白 chunk；
- `?view=strategic`：完整山坡、道路主脊、树木、水面和城市簇均在画面内；
- `?view=skyline`：无裁切楔形、漂浮体或白色空洞。

## 7. 边界

本轮没有：

- 用固定数量补楼；
- 放宽入口、道路或削填合同；
- 让单栋孤楼伪装成连续街墙；
- 把 33 栋建筑当作完整香港城市统计；
- 处理教堂/学校等全类型功能扩张、TOD 组合或排水/挡墙统一账本。

下一轮可继续做：

- 地块内庭院、裙房塔楼和联排的组合形态；
- 旧城历史建筑的实体化与真实拆合前后并存；
- 站城、公共空间、市场和办公簇的联合布局；
- 48 城代表案例的具体核心窗参数校准。
