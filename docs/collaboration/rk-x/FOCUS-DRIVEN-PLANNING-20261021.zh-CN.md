# 切面焦点驱动规划与工程授权

日期：2026-10-21

状态：**已接入 Core 生成链**

## 1. 解决的问题

此前 `urbanSlice.focusFeature` 主要被写入报告，土地压力和工程预算只读取
城市规模与特殊因子。结果是一个“金融水岸切面”和一个“历史山城切面”可以
拥有不同说明，却得到几乎相同的中心角色、时代预算和道路拓宽目标。

## 2. 推导模型

`urban_slice::compile` 现在生成 `focusBias`，模型名为：

```text
urban-slice-anchor-factor-keyword-v1
```

输入按以下顺序合成：

1. `specialFactors`：金融、政治、港口工业、旅游、历史核心、山地、填海、
   岛屿和特区；
2. `requiredAnchors` / `contextAnchors`：旧城、市场、宗教、政务、就业、
   交通、水岸、港口、公园、山体和海岸；
3. `focusFeature` 关键词：历史、更新、水岸、新区、山地工程和保护性外围。

输出为：

- `centreRoles`：中心角色优先级；
- `districtMultipliers`：旧城、更新带、新区和外围四类时代需求倍率；
- `parkBias`：公共空间需求倍率；
- `roadWideningBias`：道路拓宽目标倍率；
- `engineeringBias`：宏观工程预算倍率；
- `signals`：所有实际命中的因子、锚点和关键词，供审计。

所有倍率都被限制在有限区间内，且模型是确定性的。

## 3. 消费路径

### 时代预算

`settlementEvolution.districtDemand.inferred=true` 时，四阶段份额会乘以
`districtMultipliers` 并重新归一化。手写 `districtDemand` 不受倍率影响，
继续作为作者合同。

### 规划史

`planningHistory` 现在使用 `centreRoles` 决定中心优先级；道路拓宽目标使用
`roadWideningBias`；公共空间需求使用 `parkBias`，水岸/旅游偏向时会优先在
港口或旅游中心附近寻找公园落点。

### 工程授权

`engineeringScale` 使用 `engineeringBias` 调整宏观工程预算。特征切面的
`MacroAuthority` 新增：

```json
{
  "authorityScope": "reclamation-planning-led",
  "noAbsoluteEarthworkCap": true,
  "engineeringAuditRequired": true,
  "signals": ["special-factor:coastal-reclamation"]
}
```

这表示特区、填海、造岛、山地和中央规划切面可以为了规划合理性突破局部
弱门限，但仍受工程预算、地形、水岸、保护地、入口和施工审计约束；不存在
绕过账本的“无限施工”。

## 4. 保持不变的合同

- 普通地块仍使用 2m 局部削方弱门限；
- 普通道路仍使用 6m 填方、8% 纵坡和 25% 步行坡；
- 水岸净空、水体禁入、保护地和入口碰撞不因切面焦点放宽；
- 作者手写时代预算、锚点和关系不被自动覆盖；
- 生成器仍只输出静态实体。

## 5. 验证

```text
feature_focus_drives_distinct_planning_bias                    pass
feature_focus_reweights_only_inferred_district_budgets         pass
focus_feature_changes_centre_priority_and_road_target          pass
planning_led_reclamation_authority_is_explicit_and_audited     pass
```

断言覆盖：

- 水岸金融切面的新区倍率和道路拓宽倍率高于历史山城；
- 历史山城的旧城倍率和山地工程倍率高于水岸金融；
- 推断预算重新归一化后总和仍为 1，手写预算保持原值；
- 中心首角色和道路目标宽度随焦点改变；
- 填海授权明确标记无绝对土方上限、必须工程审计，同时局部合同仍为
  6m / 8%。

完整 Core 回归：

```text
889 passed; 0 failed; 34 ignored
```

## 6. 香港中环实烘焙

输出：

```text
bevy/work/rk-x/v3-focus-planning-hk-r2-20261021
```

结果：

- `quality-report status=pass`、`scale-audit status=pass`；
- 2,998 实体、206 道路、39 个入口连通、0 断点；
- 5 个分级市场、497 棵树、1 个港口物流中心；
- `focusBias` 中心角色为 `financial, historic, port`；
- `oldCore=1.5104`、`renewal=1.4016`、`newGrowth=1.7000`、
  `periphery=1.2320`；
- 道路拓宽倍率 1.45、公园倍率 1.4071、工程倍率 1.60；
- 工程授权范围 `reclamation-planning-led`，无绝对土方上限且必须工程审计；
- 该控制手写的 7 / 19 / 7 / 0 时代预算保持不变，证明焦点倍率只作用于
  推断预算；
- 与上一版连续街墙控制相比，实体数由 3,011 变为 2,998，焦点已实际改变
  中心与物化结果。

第一次重烘因港口物流中心缺少 `industrial` 资产契约而被 `scale-audit`
失败关闭。控制 Manifest 随后补齐 40–260m 宽、40–200m 深、6–40m 高的
工业资产语义契约，并重新烘焙通过。

## 7. 已知边界

关键词只负责在结构化因子和锚点不足时补足意图，不替代逐案城市调查。
48 城仍只作为核心切面机制先验，不能宣称为完整城市统计或真实地块复刻。
