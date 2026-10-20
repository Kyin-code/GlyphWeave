# 核心城区自然—人文生成链 V2 验收记录

日期：2026-10-21

状态：**核心数据链与静态实体接入完成；渲染质量仍不等于最终产品**

## 1. 本轮解决的问题

本轮不是继续调预览颜色，而是修正生成真源：

1. 自然世界从“噪声 + 少量开关”升级为可解释的地貌过程链；
2. Manifest 可以只写大致地理环境，由 Rust Core 确定性派生完整自然控制；
3. 大都市切面能声明自己要表达的地理关系、主体锚点和场外连接；
4. 48 城先验从四条文本升级为完整核心切面、历史、结构和压力数据；
5. 道路与容量建筑从计划 JSON 转成最终 `EntityInstance`；
6. 城市增长加入网络中心、道路拓宽、旧城保留、更新候选和公园需求；
7. 土地压力和工程强度改为可解释系数，同时保持局部硬合同不变。

## 2. 自然地理生成

### 2.1 环境草图派生

Manifest 现在可以只声明：

```json
{
  "naturalGeography": {
    "version": 1,
    "mode": "derived",
    "setting": "coastal-mountain",
    "regionalReliefM": 820,
    "climateBand": "humid-subtropical",
    "derivationSeed": 42,
    "hints": {
      "karst": false,
      "glacial": false
    }
  }
}
```

Rust Core 会确定性派生：

- 构造抬升轴、抬升强度、盆地/三角洲沉降；
- 气候、季节性、主导风和雨影；
- 排水格局、河流密度、弯曲度、河漫滩宽度和历史河道迁移；
- 海岸类型、波候、潮差和海陆侵蚀；
- 喀斯特、风蚀/风积、冰川和重力滑坡；
- 灾害集合和自然阶段证据。

### 2.2 地貌过程

当前过程名包括：

- `tectonic-uplift`
- `tectonic-subsidence`
- `fluvial-incision`
- `fluvial-deposition`
- `river-avulsion`
- `coastal-wave-erosion`
- `coastal-deposition`
- `aeolian-erosion`
- `aeolian-deposition`
- `glacial-scour`
- `karst-dissolution`
- `mass-wasting`

这些过程已经实际影响地形，而不只进入报告：河流密度改变切割间距，弯曲度改变河道场，河漫滩改变古河道宽度，雨影降低背风侧湿度，海岸侵蚀、沉积、冰川槽、喀斯特洼地和滑坡物质改变高度场。

### 2.3 实烘结果

Manifest：

`examples/geo-bay-natural-1km.manifest.json`

输出：

`bevy/work/rk-x/derived-geography-smoke-20261021`

结果：

- 纯自然实体 3,426；
- `humanTraceCount=0`；
- `scale-audit pass`；
- `quality-report pass`；
- 派生过程：
  `tectonic-uplift → fluvial-incision → river-avulsion →
  coastal-wave-erosion → mass-wasting`；
- 排水格局为 `dendritic`；
- 河道密度 0.5902；
- 河道弯曲度 1.3400；
- 河漫滩宽度约 170.6m。

## 3. 48 城核心切面先验

### 3.1 数据升级

`assets/data/urban-case-priors-48.json` 已升级到 v2。每案包含：

- 核心切面；
- 自然地理草图；
- 地貌过程；
- 三段以上历史发展轴；
- 旧核心、更新区、新区、边缘区；
- 中心体系；
- 社区类型；
- 特殊因子；
- 土地压力先验；
- 证据等级。

### 3.2 校验

`node scripts/check-urban-case-priors.mjs`：

```json
{
  "status": "pass",
  "version": 2,
  "cases": 48,
  "prefixCounts": {
    "cn": 12,
    "us": 12,
    "eu": 12,
    "jp": 6,
    "hk": 6
  }
}
```

生成器 `scripts/build-urban-case-priors.mjs` 连续运行两次文件 SHA-256 相同。

Rust Core 通过 `worldgen::urban_case_priors` 加载同一 JSON，并提供：

- `case(id)`：单一案例；
- `natural_geography_sketch(case)`：可直接进入 Manifest 的地理草图；
- `urban_slice(case)`：切面配置；
- `pressure_prior(case)`：压力先验。

## 4. 大都市切面语义

`urbanSlice.semanticsVersion = 2` 新增：

```json
{
  "viewpoint": "old-core-to-financial-waterfront",
  "focusRelations": [
    {
      "subject": "financial-core",
      "relation": "faces",
      "object": "deep-water-bay"
    }
  ],
  "requiredAnchors": [
    {
      "id": "bay-edge",
      "kind": "waterfront",
      "role": "main-public-frontage"
    }
  ],
  "contextAnchors": [],
  "edgeIntents": [
    {
      "edge": "west",
      "kind": "rail",
      "destinationRole": "regional-metropolis"
    }
  ]
}
```

语义 v2 的 feature-cut 必须：

- 有 `focusFeature`；
- 至少一个 `focusRelations`；
- 至少一个 `requiredAnchors`；
- 至少一个 `edgeIntents`；
- `metropolis/megacity` 不得声明 `whole-settlement`。

旧 Manifest 保持兼容；缺少这些字段时仍按 v1 解释。

## 5. 城市规划史

`planningHistory` 已进入生成链：

```json
{
  "version": 1,
  "enabled": true,
  "oldCoreRadiusM": 220,
  "parkAreaM2PerPerson": 8,
  "renewalShare": 0.35
}
```

输出 `planning-history-plan.json`，包含：

- 网络中心及其角色；
- 建筑到中心的网络距离和中心性；
- 旧核心/更新扩张/新区边缘归类；
- 街面保留、再开发评估、填充连接或新增长建议；
- 道路拓宽动作；
- 公园面积需求；
- 金融、政治、港口、旅游、历史或混合中心动作。

核心切面实测：

- 3 个中心：`financial / mixed / historic`；
- 7 类规划动作；
- 6 个有严格可达入口的最终建筑；
- 2 个计划建筑因生成正面 120m 内没有满足合同的可验证路线，被具名记录为未实现，不伪装完成。

## 6. 土地压力与工程强度

土地压力输出：

```text
landPressureIndex =
  urbanScale
  * centrePressure
  * terrainScarcity
  * (1 + protectionResistance * 0.18)
  * specialRegime
  * tourismSeasonality
```

如有 48 城样本先验，再与模型结果按样本权重融合。

特殊因子独立计权：

- 特区 0.22；
- 金融中心 0.12；
- 政治中心 0.10；
- 港口工业 0.12；
- 填海 0.18；
- 岛屿 0.10；
- 山地 0.20；
- 历史核心 0.08；
- 旅游 0.06，并叠加季节因子。

工程系数只影响宏观项目预算和候选优先级：

```text
engineeringScale = clamp(0.45 + landPressureIndex * 0.90, 0.45, 2.40)
```

局部合同始终不变：

- 道路最大填方 6m；
- 最大纵坡 8%；
- 最大步行坡 25%；
- 入口 pad 削方 2m；
- 岸线净距和保护区合同。

核心切面实测：

- 基础土地压力 2.1129；
- 路线网络中心性 0.5271；
- 路线修正土地压力 2.1257；
- 工程系数 2.3631；
- 6 项宏观工程全部在预算计划内；
- 局部合同未改变。

## 7. 道路和建筑实体化

### 7.1 道路

道路计划不再只存在 `route-evolution-plan.json`。`materialize` 将折线转成：

- 有 `rotationYDeg` 的静态道路段；
- 每段保留设计高程；
- 适度抽稀后仍能跟随弯道；
- 不与 6m/8% 工程合同冲突。

### 7.2 建筑

容量建筑不再只存在 `settlement-evolution-plan.json`。每栋最终建筑：

- 有稳定实体 ID；
- 有真实宽度、进深、高度；
- 有地面高度；
- 进入入口和 access graph；
- 被严格入口合同拒绝时必须进入 `settlement-materialization-report.json`，不能静默消失。

## 8. 核心切面实烘

Manifest：

`examples/derived-core-city-slice-1km.manifest.json`

输出：

`bevy/work/rk-x/derived-core-slice-materialized-v6-20261021`

结果：

| 项 | 结果 |
|---|---:|
| 最终实体 | 3,537 |
| 道路段 | 147 |
| 内部拓宽道路段 | 86 |
| 最终建筑 | 6 |
| 住宅 | 1 |
| 店面 | 5 |
| 最终公园 | 1 |
| 已验证入口 | 6 |
| 断开的入口 | 0 |
| 计划路线 | 2 |
| 总路线长度 | 977.71m |
| 最大曲线偏移 | 89.76m |
| scale audit | pass |
| quality report | pass |

计划建筑 8 栋，最终 6 栋；2 栋在 `settlement-materialization-report.json` 中具名记录为：

`strict generated-facade access contract produced no verified route`

这体现“建筑数量是结果”，而不是强行复制两栋来凑计划数。

预览：

`http://127.0.0.1:8095/preview/index.html?view=strategic`

## 9. 当前边界

已完成：

- 自然地理草图派生；
- 地貌过程进入地形和生态；
- 48 城核心数据与 Rust 读取；
- 大都市特征切面语义；
- 网络中心规划史；
- 土地压力和工程系数；
- 道路和建筑静态实体化；
- 入口可达和具名拒绝。

仍未完成：

- 48 城全量代表性世界烘焙；
- 金融/政治中心从角色计划落实为专属公共建筑与广场组合；
- 多层级市场设施实体；
- 排水、挡墙、生态和建筑基础统一工程账本；
- 真正接近最终产品的大量高密度城区视觉。

因此，本轮证明的是生成链和空间因果关系，不宣称现代城市视觉已经最终完成。
