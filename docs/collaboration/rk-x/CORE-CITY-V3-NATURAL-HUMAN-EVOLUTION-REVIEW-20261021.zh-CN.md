# 核心城区 V3：自然—路径—聚落—新旧城—市场网络设计审核稿

状态：**待项目负责人审核；审核前只做调查与设计，不继续扩大城区实现范围**

日期：2026-10-21

性质：设计审核稿，不是完成声明，也不替代现有 A/B/C 验收合同。

适用范围：GlyphWeave 现代城市核心切面生成。Rust Core 是唯一生成真源；
HTML/Three.js 只读取已烘焙结果；本轮不处理未来需求。

W0 调查进度：已完成全部 48 个代表案例的来源入口审计，其中 12 个完成首轮
OSM 米制观测，机器矩阵见
[`URBAN-CASE-EVIDENCE-W0-20261021.zh-CN.md`](URBAN-CASE-EVIDENCE-W0-20261021.zh-CN.md)
和 `assets/data/urban-case-evidence.json`。48 案全部有覆盖状态，
48 案均为 `source-audited-core-cut`；
12 案达到 `partial-osm-core-window`，尚无案例达到完整
`measured-core-window`。

## 0. 审核结论

当前实现已经具备“自然草图、地形成本道路、需求容量、规划史、市场网络、
工程系数”这些零件，但它们还没有组合成用户要求的空间因果：

```text
无人类痕迹的自然景观
→ 地形、水系、生态和林草灌湿群落成立
→ 人类沿谷地、渡口、泉眼、山坳和海岬走出路径
→ 路径节点和资源条件产生聚落候选
→ 聚落沿既有骨架逐阶段生长、更新和扩张
→ 土地紧张、交通、产业和政治/金融意图形成不同城区
→ 建筑体量、数量和用途由需求与地块容量共同产生
→ 生鲜市场等公共服务按真实网络、容量、竞争和时距布局
→ 宏观工程回写地形，但局部硬合同不放宽
```

本稿确认五个必须改变的点：

1. **普通建筑不再使用固定完成数。** 数量只是需求、地块供给、建筑类型、
   容积率和入口条件的结果；普通建筑不能用“补齐 N 栋”作为成功条件。
2. **自然阶段是最终 bake 中的冻结基线。** 不是只在另一份测试世界里证明一次，
   而是每个城市都要保留可比较的自然快照、人工改造 diff 和零人工痕审计。
3. **道路先服从地形和人类活动节点。** 山谷、渡口、边界、资源和已有路径决定
   选线；曲线、绕坡、盘山、桥涵和分阶段拓宽必须可解释。只有经过规划的平地
   街区才允许形成规则网格。
4. **新旧城区是同一骨架的多次迭代。** 保留、改造、拆合、填充、替换和拓宽
   都必须带父对象、阶段和原因，不能用两套并排模板或换材质伪装。
5. **现代感来自空间制度，不是模型数量。** 旧城小地块、连续街墙和市场；
   新区站城、办公裙房塔楼和大型商业；郊区、旅游、放牧、种植与保护地各有
   自己的道路、地块、建筑和服务逻辑。

## 1. 当前实现审计

### 1.1 已有能力

当前代码中已经存在以下可复用基础：

- `naturalGeography.mode = "derived"`：由 setting、起伏、气候和 seed 派生
  构造、水文、海岸、气候、风蚀、冰川、喀斯特和滑坡等过程；
- `naturalOnly`：纯自然阶段和 `humanTraceCount=0` 控制；
- `routeEvolution`：16m 搜索、坡度/水体/转弯代价、折线与桥/高架记录；
- `settlementEvolution`：住户、就业、商业、服务需求与地块容量；
- `epochEvolution`：保留、修改、拆除、替换和父对象账本；
- `planningHistory`：路网中心、旧城保留、道路拓宽、公园需求和中心角色；
- `marketCatchment`：沿真实折线网络计算需求、容量、竞争、覆盖和缺口；
- `landPressure` / `engineeringScale`：城市规模、地形、保护、特区、旅游
  和样本先验进入宏观工程预算；
- `urbanSlice`：大都市特征切面、必须表达的地理关系、锚点和场外连接。

这些接口应保留，不能推倒重来。

### 1.2 当前核心切面实测暴露的问题

以 `derived-core-city-slice-1km` 和 v19 烘焙结果为例：

| 指标 | 当前实测 | 问题 |
|---|---:|---|
| 最终实体 | 3,538 | 自然细节占绝大多数，城市实体过少 |
| 最终建筑 | 6 | 不足以形成连续现代城区 |
| 住宅/店面 | 1 / 5 | 大量住户被塞进商业临街单元，类型失真 |
| 计划建筑 | 8 | 需求只由少数高容量地块一次满足 |
| 道路段 | 147 | 数量看似不少，但主要是一条 8m 路线走廊的实体切段 |
| 市场 | 1 | 单一类型、单一容量，没有社区菜场—邻里超市—大超市层级 |
| 城市用地代理面积比 | 约 2.2% | 对“核心城区切面”明显偏低 |
| 金融中心 | 入口合同拒绝 | 说明中心设施仍没有配套入口/裙房工程 |

代码层面的直接原因：

- `settlementEvolution` 对整张图使用一套 `lotWidthM/lotDepthM/roadStepM/
  setbackM/floorAreaRatio/maxFloors`；
- 每块候选地最多形成一栋建筑，建筑占地为同一地块面积乘固定覆盖率；
- 楼层由到单一路线中心的距离衰减决定；
- 零售需求存在时优先把所有候选建成 `storefront`；
- 需求一旦由前几块高容量建筑满足，循环立即结束；
- 因此“数量是结果”在形式上是真的，但城市仍会被少数同一尺寸模板表示。

这不是继续增加模型即可解决，必须先改街区—地块—建筑容量的结构。

### 1.3 48 城数据现状

`assets/data/urban-case-priors-48.json` 当前为 v2，已包含：

- 中国 12、美国 12、欧洲 12、日本 6、香港 6；
- 自然地理、地貌过程、灾害；
- 至少三段历史；
- 旧核心、更新、新区、边缘；
- 中心体系、社区类型、特殊因子、土地压力先验。

结构校验通过，但 48/48 的证据等级仍全部是 `heuristic-core-cut`，来源主要是
仓库内的三份研究/案例文档，没有逐案 URL、访问日期、切窗面积和官方数据。

因此本稿明确区分：

- **已有：** 48 个核心切面的机制先验；
- **尚无：** 48 城全部达到“可复算真实数值”的史料矩阵；
- **下一步：** 先建立来源矩阵，再允许真实城市名称、切窗和高精度参数进入
  正式 Manifest；合成世界可以继续使用先验，但必须标记为合成。

## 2. 48 城调查方法与本轮发现

### 2.1 调查单位

调查单位是**核心切面**，不是整个行政市。每个案例至少拆成：

| 层 | 需要记录的字段 |
|---|---|
| 自然基座 | 区域地貌、主要过程、洪水/风暴/滑坡/地震等灾害、水系和海岸 |
| 起源 | 渡口、港湾、铁路、水渠、矿点、农业、政治或防御 |
| 旧城 | 路网、地块、街墙、市场、宗教/市政/交通节点 |
| 工业与交通 | 港口、铁路、工厂、仓储、工人住区、高速/轨道 |
| 更新 | 留、改、拆、合、填充、滨水/工业/旧住区再开发 |
| 新区 | 轨道新城、产业园、封闭社区、郊区、填海或山前台地 |
| 功能 | 住宅、办公、零售、商超、学校、医院、宗教、公园、市政 |
| 生鲜网络 | 菜场、街市、商店街、邻里超市、大型超市、周期/批发市场 |
| 工程 | 填海、开山、台地、挡墙、堤坝、排水、桥隧和工程年代 |

切面必须声明 `crop` 和表达对象。1km 图只能证明局部关系，不得声称复刻完整城市。

### 2.2 当前 48 案汇总

从现有 v2 数据可直接复算的结构性事实：

| 维度 | 结果 |
|---|---|
| 城市规模 | megacity 19、metropolis 10、city 18、town 1 |
| 灾害 | flood 36、storm-surge 24、landslide 21、earthquake 15 |
| 常见地貌过程 | fluvial-deposition 20、fluvial-incision 16、mass-wasting 16 |
| 特殊因子 | historic-core 34、financial-center 21、port-industrial 21、tourism 17 |
| 地形/工程因子 | mountainous 15、coastal-reclamation 13、political-capital 4 |
| 社区先验 | europe-perimeter 12、china-gated 10、us-mixed-income 9 |

这组数据说明：

- 水、洪水、海岸、港口和铁路是 48 案中最常见的城市起源与约束；
- 高密度不等于同一种住宅，欧洲周边街区、中国封闭/大院、美国收入混合、
  日本町家、香港屋邨必须产生不同地块与街面；
- 旅游、山地、填海、政治和金融不能被一个统一中心距离公式吸收；
- 48 案没有 village/county-city 样本，县城、乡村和农牧场景不能伪造
  “48 城统计”，必须继续使用区域研究和独立控制样例。

### 2.3 需要补齐的证据

每案升级为 v3 时至少增加：

```json
{
  "evidenceVersion": 3,
  "coreWindow": {
    "areaKm2": 3.2,
    "widthM": 1800,
    "depthM": 1780,
    "anchor": "old market to waterfront",
    "cropGeoJson": "..."
  },
  "sources": [
    {
      "publisher": "municipal planning authority",
      "title": "official plan or heritage inventory",
      "url": "https://...",
      "retrievedAt": "2026-10-21",
      "kind": "official",
      "claims": ["heritage boundary", "block pattern", "station location"]
    }
  ],
  "measured": {
    "roadDensityKmPerKm2": null,
    "medianParcelFrontageM": null,
    "floorAreaRatioRange": null,
    "residentialHeightRangeM": null,
    "marketAccessMinutes": null,
    "reclaimedAreaM2": null
  },
  "inferred": [
    {
      "field": "marketAccessMinutes",
      "value": 12,
      "reason": "facility locations known, household distribution approximated"
    }
  ]
}
```

来源优先级：

1. 政府规划、统计、测绘、遗产与交通机构；
2. 市政历史、大学研究和规划档案；
3. OpenStreetMap 等开放几何只用于位置和路网辅助，不单独证明人口或用途；
4. 百科/旅游网页只作线索，不能作为唯一数值来源。

严格复刻真实城市时，每个关键空间事实至少两个独立来源；合成切面可以引用
多个案例机制，但结果不得命名成某一真实城市。

### 2.4 代表性案例审计集

全部 48 案先做统一字段和研究分级；下列 12 案优先补到可复算证据，用于后续
核心切面回归：

| 案例 | 主要验证机制 | 后续样例角色 |
|---|---|---|
| 北京 | 政治轴、胡同、环路、CBD、封闭街区 | 政治首都/旧城—新区 |
| 上海 | 里弄、江港、金融中心、填海 | 江海大都市 |
| 重庆 | 两江、山地、台地、陡坡 | 山城工程 |
| 深圳 | 特区、城中村、湾区、填海 | 快速新区/制度因子 |
| 纽约 | 网格、公寓、滨水再开发 | 高密网格/更新 |
| 芝加哥或波特兰 | 平原网格、湖/河、小街区 | 平地网格对照 |
| 巴黎 | 连续街墙、庭院街区、市场街 | 欧洲周边街区 |
| 里斯本或布拉格 | 山城旧城、工业/面板住宅更新 | 历史保护与更新 |
| 东京下町 | 细地块、商店街、轨道、水渠 | 日本混合街区 |
| 神户三宫 | 山—海狭窄走廊、港城 | 山海工程 |
| 香港中环 | 陡坡、金融、填海、天桥/轨道 | 高密金融水岸 |
| 香港沙田 | 新市镇谷地、河道、公共屋邨 | 新城与社区商业 |

补充控制：昆明/丹佛检验高原和平原农业、牧场/铁路边缘；拉斯维加斯和西贡
检验旅游与海岸村落；这些用于区域类型，不与 48 城核心统计混算。

## 3. 总体生成结构

### 3.1 阶段划分

```text
N0 Natural
  地理草图 → 地貌过程 → 高度/水系/土壤/气候 → 生态群落

R0 Route
  自然节点 → 人类欲望线 → 地形成本路径 → 桥涵/渡口/山道

S0 Settlement Seed
  水源、渡口、资源、可建地、灾害和接入 → 聚落接受/拒绝

U0 Urban Skeleton
  路径升级 → 历史中心 → 街区 → 地块 → 公共空间

U1 Epoch Evolution
  founding → old-core → expansion → industrial → modern-new → renewal

F0 Functional Program
  住宅、办公、商业、商超、市场、学校、医院、宗教、市政、公园

M0 Market Network
  生鲜需求 → 多层级市场 → 网络时距 → 竞争/负载/缺口

E0 Engineering
  局部基础、道路、挡墙、排水、桥隧、填海、开山、台地

B0 Bake / Audit
  实体化 → 入口/碰撞 → 完整分块 → HTML 预览 → 视觉反馈
```

每个阶段写入独立机器可读产物和哈希。后续阶段只能读取前一阶段已通过 gate 的
结果，不能在烘焙器内部用临时随机数补一个看起来可用的空间。

### 3.2 最终世界保留两条时间线

1. **生成时间线：** N0→R0→S0→U0→U1→F0→M0→E0。
2. **城市历史时间线：** 聚落起源→旧城→扩张→工业→现代新区→更新。

两者不得混淆。生成器按前者执行，最终对象按后者记录 cohort、父对象和操作。

## 4. N0 纯自然阶段

### 4.1 输入

Manifest 只要求作者给出可理解的地理草图：

```json
{
  "naturalGeography": {
    "version": 2,
    "mode": "derived",
    "setting": "mountain-lake",
    "regionalReliefM": 700,
    "climateBand": "humid-subtropical",
    "hydrologyHint": "lake outlet to river",
    "coastalExposure": "sheltered",
    "hazardHints": ["flood", "landslide"],
    "derivationSeed": 42
  }
}
```

Agent 或作者可给出更详细 `authored` 数据；Rust Core 负责补齐并确定性派生。

### 4.2 必须真实影响世界的自然层

- 构造抬升/沉降、断层、山脊、谷地、盆地、台地、阶地和冲积扇；
- 河流下切、河谷、河漫滩、历史改道、冲积、三角洲、湖泊和湿地；
- 海岸侵蚀、沉积、潮差、波候、海岬、海湾、河口和岛屿；
- 风蚀/风积、冰川、喀斯特、滑坡和火山；
- 坡向、日照、雨影、湿度、土壤、盐碱、排水和洪水频率；
- 林木线、草地、灌丛、湿地、裸岩、季节草场和栖息地接口。

自然世界不能由“低洼处随机染水”“草场随机圆斑”“山体上随机撒树”组成。
植被、水体和地表必须由连续环境场与地貌关系决定。

### 4.3 冻结产物

每个场景输出：

- `natural-stage.json`
- `natural-stage.height.bin`
- `natural-stage.surface.bin`
- `natural-stage.hydrology.json`
- `natural-stage.ecology.json`
- `natural-stage.hash`

最终世界保留 N0 基线和逐格人工改造 diff，使“哪里开山、填海、铺路、削填”
都能回到自然原状。

### 4.4 N0 闸门

- `humanTraceCount = 0`；
- 不存在 road/building/field/fence/paving 等人工表面和实体；
- 水体、水面、岸线和湿地使用独立连续模型；
- 树/灌木/草/岩石不落水，岸生类型有明确水深和岸距；
- 1m 高度、材质、水文和 chunk 边界连续；
- 同 seed 重烘焙自然基线 hash 一致；
- `?view=natural` 不得出现任何道路、建筑、田块或人工铺装。

## 5. R0/S0 路径与聚落产生

### 5.1 自然节点

从 N0 提取，而不是预先写死：

- 缓坡谷底、支流汇口、天然浅滩、渡口、山坳、垭口、泉眼；
- 湖岸浅湾、海岬背风面、港湾、河口、潮汐通道；
- 冲积扇、可耕土壤、牧草边界、林木线、矿产和特殊资源；
- 洪水线以上的台地、天然防御点和可建地边缘。

每个节点记录来源地形指标、候选用途、接入成本和灾害。

### 5.2 路径生成

路径在三维地形成本场上搜索：

```text
cost =
  slopeWork
  + longitudinalGrade
  + curvature
  + floodRisk
  + wetness
  + forest/soil clearing
  + waterCrossing
  + protectedArea
  + constructionEraCost
  + detourVsDesireLine
```

路径不是“两点直线加噪声”。应允许：

- 沿等高线横切山坡；
- 沿谷底避开陡壁；
- 在天然浅滩形成渡口；
- 通过桥、堤道或涵洞跨越水系；
- 在旧路线基础上局部拓宽和裁弯；
- 在平地规划街区中形成直线段。

### 5.3 路径时代

同一道路可以有多个阶段：

| 阶段 | 形态 |
|---|---|
| trail | 窄、曲折、沿地形与水/牧场节点 |
| rural-road | 连接田块、村口、渡口和集市 |
| industrial-road | 码头、铁路、工厂和仓储间的大半径路线 |
| planned-arterial | 规则几何、明确断面和交通容量 |
| widened-old-road | 保留旧线形，局部拓宽与建筑退让 |

最终道路实体记录 `routeId + stage + parentRouteId + designProfile`，避免把
现代八车道直接覆盖在历史步道上而没有任何更新证据。

### 5.4 聚落接受条件

候选聚落评分：

```text
settlementPotential =
  waterSecurity
  + buildableLand
  + routeAccess
  + resourceAccess
  + floodSafety
  + climateShelter
  + strategicIntent
  + existingSettlementMomentum
  - terrainCost
  - hazard
  - protectionConflict
```

不预设“必须有聚落”。结果为 0 个聚落是合法结果。每个被拒绝节点必须具名写入
`settlement-candidates.json`。

## 6. 城市中心与土地价值

### 6.1 中心不是地图圆心

使用网络场：

```text
centrality =
  historicAnchor
  + roadBetweenness
  + transitAccess
  + jobAccess
  + civicAccess
  + marketAccess
  + tourismAccess
  + agglomeration
  - terrainBarrier
  - protectionBarrier
  - hazardPenalty
```

所有时距优先使用真实步行、道路、公交/轨道和车行网络。直线距离只能作为局部
缺失模式时的显式近似，并标记 `inferred: true`。

不同中心可以并存：

- 历史市场/寺社/教堂；
- 市政/政治中心；
- 车站/码头；
- 金融/办公中心；
- 大学/医院；
- 新区商业中心；
- 旅游入口；
- 工业/物流节点。

### 6.2 用途竞争

每个候选地块对不同用途计算：

```text
siteScore(use) =
  access(use)
  + agglomeration(use)
  + demand(use)
  + suitability(use)
  + planningIntent(use)
  - rentProxy(use)
  - conflict(use)
  - hazard(use)
```

用途不是按同心圆硬分区，而是通过分数、保护规则、地块形态和连续街面形成
混合带、商业街、办公簇、住宅组团和工业边缘。

## 7. 新旧城区与空间类型

### 7.1 阶段不是固定半径

`founding / old-core / expansion / industrial / modern-new / renewal` 的边界
由建设 cohort、历史节点、道路升级和地块关系决定，不由 `distance < R` 决定。

增长触发包括：

- 新渡口、铁路、车站、港口或快速路；
- 人口、就业、旅游和服务需求增加；
- 旧城土地紧张或基础设施超载；
- 新区土地释放、填海、台地或防洪工程；
- 产业迁移、灾害、保护政策和城市更新意图。

### 7.2 地块与街网形态

| 区域 | 街网与地块 | 建筑形态 | 必要证据 |
|---|---|---|---|
| 历史旧城 | 小地块、连续街墙、巷弄、不规则轴线 | 2–5 层混合、底商、院落、市场厅 | 旧路线/地块/历史对象保留 |
| 更新带 | 旧街块与合并地块并存 | 中层公寓、办公、裙房、社区设施 | 父对象、保留/拆合记录、公共空间 |
| 新城区 | 交通先导，街区较大但不应封闭 | 塔楼、裙房、办公、商场、医院、公园 | 站点/干道/公共空间在位 |
| 郊区 | 车行/公交走廊、细分住宅或组团 | 独立住宅、联排、多户、超市 | 学校/停车/服务网络 |
| 旅游区 | 景观入口、步行环、码头 | 旅馆、餐饮、纪念商业、周期市场 | 居民需求与游客需求分离 |
| 放牧区 | 牧道、围栏、饮水点、草场边界 | 棚圈、饲料、兽医、小型集散 | 水源、草场、季节容量 |
| 种植区 | 田块、灌渠、田埂、农路 | 仓房、农资、集散、村庄服务 | 土壤、水权、排水和运输 |
| 保护区 | 连续自然环境、有限步道 | 入口设施、科研/巡护 | 禁止城市化边界 |

### 7.3 道路密度与土地利用的初始控制范围

以下使用**可驾驶道路中心线密度**。首轮 12 个核心窗口的实测中位数为
20.59 km/km²，范围为 14.26–36.70，因此中心区旧控制带已按观测修正；
其余类型仍是待校准包络：

| 类型 | 路网密度 km/km² | 主导街区尺度 | 典型 FAR |
|---|---:|---:|---:|
| 历史细街网 | 14–23 | 60–120m | 0.8–2.8 |
| 中心网格/混合区 | 18–34 | 80–180m | 1.2–5.0 |
| 欧洲周边街区 | 14–23 | 90–180m | 1.2–4.0 |
| 新城区/站城 | 14–23 | 120–260m，内部必须可步行穿透 | 1.5–8.0 |
| 产业园/港区 | 4–8 | 200–600m | 0.4–1.5 |
| 美国郊区 | 5–10 | 200–500m | 0.15–0.9 |
| 农牧边缘 | 1–4 | 田块/牧场尺度 | 0.03–0.35 |

每类实际值必须由案例来源或控制场景校准；不能把上表作为全国模板。

建筑覆盖率也不能代替 FAR：首轮窗口中，深圳福田样本只有 177 个建筑
centroid，但覆盖率为 0.351；东京浅草有 4,153 个，覆盖率为 0.368。
因此建筑数量、占地、楼层和 FAR 必须同时表达。

## 8. 建筑数量、尺寸与用途

### 8.1 普通建筑数量是结果

输入需求：

```json
{
  "developmentDemand": {
    "households": 12000,
    "jobs": 9000,
    "retailFloorAreaM2": 85000,
    "freshFoodDemandUnits": 16000,
    "schoolPlaces": 2400,
    "clinicVisitsPerDay": 1800,
    "visitorPeak": 5000
  },
  "countPolicy": "emergent-no-fill",
  "explicitLandmarkCountsOnly": true
}
```

输出链：

```text
人口/就业/商业/服务/游客需求
→ 按区域和网络可达性分配
→ 街区切分
→ 地块形态与法定上限
→ 建筑类型和体量
→ 实际交付容量
→ unmetDemand / vacantCapacity
```

建筑数量不是目标值。需求未满足时必须解释“缺需求、缺地块、缺接入、缺工程、
保护冲突”中的哪一个，不能复制建筑凑数。

### 8.2 先选建筑原型，再决定尺寸

原型至少包括：

- 窄面宽街屋、町家、胡同/里弄住宅；
- 欧洲庭院/周边街区翼楼；
- 中层公寓板楼；
- 中国封闭小区板楼与裙房；
- 塔楼+裙房；
- 办公楼/金融塔；
- 香港屋邨/垂直混合；
- 独立住宅、联排和多户住宅；
- 市场厅、邻里超市、大型超市、仓储；
- 学校、医院、市政、寺社/教堂；
- 工业厂房、仓库、码头设施。

尺寸由以下约束共同决定：

- 地块面宽/进深和允许覆盖；
- FAR、楼层、退界、日照、防火、疏散；
- 结构跨度和典型楼板深度；
- 入口、卸货、停车和消防车接入；
- 坡度、地基、洪水线和挡墙；
- 街墙连续性和历史保护；
- 同一街区内的体量协调。

同一个地块允许：

- 单栋；
- 裙房+塔楼；
- 多栋联排；
- 庭院围合；
- 住宅+底层商业；
- 保留旧体量并在后部或上部填充。

### 8.3 地块目录示例

以下为类型范围，不是固定尺寸：

| 原型 | 面宽 | 进深 | 层数 | 说明 |
|---|---:|---:|---:|---|
| 历史街屋/町家 | 4–12m | 8–24m | 2–4 | 连续街墙，混合底层 |
| 庭院/周边街区 | 30–100m 总面宽 | 18–45m 翼楼 | 4–8 | 内部庭院，街区边界连续 |
| 中层公寓 | 18–55m | 14–32m | 5–12 | 入口与绿地/停车关系 |
| 办公塔+裙房 | 40–120m 裙房 | 20–55m 塔楼平面 | 12–50+ | 中心性和轨道优先 |
| 封闭小区板楼 | 40–90m | 12–22m | 6–18 | 围墙、门禁、内部公共空间 |
| 独立住宅 | 8–18m | 10–24m | 1–3 | 前院/侧院/车库 |
| 市场厅 | 30–90m | 20–50m | 1–3 | 摊位、冷链、卸货和多个入口 |
| 邻里超市 | 35–80m | 35–80m | 1–3 | 停车/公交/卸货 |
| 大型超市 | 70–160m | 60–120m | 1–2 | 车行/物流边缘 |

最终尺寸必须有米制字段、资产契约和 footprint 审计。

### 8.4 中心专属功能

**政治中心：**

- 市政/议会/法院/纪念广场形成公共轴线；
- 安全退界、礼仪空间、公共交通和大片开放地是合理用途；
- 不强行追求最高 FAR，也不把整个中心做成封闭办公园区；
- 政治功能可由案例因子出现，不是每城必有。

**金融中心：**

- 办公塔、裙房商业、酒店、交易所/金融机构、轨道和公共广场成组出现；
- 高中心性、高就业密度和高地价共同驱动；
- 必须生成入口、消防、卸货、地铁/地面接驳和公共空间；
- 塔楼不是孤立盒子，缺少裙房/广场/接驳时不得宣称中心完成。

**宗教设施：**

- 教堂、寺社、清真寺等由文化背景、历史 cohort、社区和公共节点决定；
- 不要求每个街区都出现；
- 历史场所是保留对象，不能随机替换为普通商业。

## 9. 生鲜市场与辐射密度

### 9.1 设施层级

| 类型 | 服务半径/时距 | 容量表达 | 典型位置 |
|---|---|---|---|
| 便利店生鲜柜 | 步行 5 分钟 | 冷藏面积、日销量 | 街角、站点、社区入口 |
| 社区菜场/街市 | 步行 8–12 分钟 | 摊位、冷链、日客流 | 旧城、密集住宅 |
| 邻里超市 | 步行/公交 10–15 分钟 | 营业面积和日需求单位 | 混合区、新区组团 |
| 大型超市 | 车行/公交 15–25 分钟 | 建筑面积、停车、卸货 | 新区、郊区走廊 |
| 周期市场 | 每周/季节 | 摊位容量、开放日 | 广场、旅游入口 |
| 批发/集散市场 | 30–60 分钟 | 日吞吐、冷链、装卸 | 铁路、港口、城市边缘 |

具体容量必须由营业面积、摊位、冷藏和卸货能力推导；不再只给一个全局
`marketCapacity`。

### 9.2 需求

需求点来自最终建筑，而不是先验均匀网格：

```text
freshFoodDemand_i =
  households_i * householdWeight
  + jobs_i * workerWeight
  + retail/service demand
  + visitors_i * seasonalWeight
  + farm/pasture household demand
```

住宅、就业、游客、季节人口和农牧人口使用不同时间带和权重。

### 9.3 吸引力、分摊和辐射

```text
A_ij = capacity_j * quality_j * typePreference_ij
       * exp(-beta_mode * travelTime_ij)

P_ij = A_ij / sum_k(A_ik)

load_j = sum_i(demand_i * P_ij)

supplyDensity_i = sum_j(capacity_j * exp(-beta_mode * travelTime_ij))

deficit_i = max(0, demand_i - supplyDensity_i)
```

- `travelTime_ij` 来自步行、道路、公交/轨道和车行网络；
- 不同模式使用不同速度和衰减；
- 市场容量有上限，负载超过阈值时触发扩容或新增候选；
- 竞争由所有市场共同分摊，不使用统一圆形覆盖；
- 输出 5/10/15/30/60 分钟供应密度、最近市场、负载、缺口和未覆盖原因。

首轮 12 个 1km² OSM 核心窗口已完成步行网络校准。9 个窗口存在可路由的
市场型设施，市场型中位步行时间的中位数为 3.87 分钟，10 分钟覆盖比例的
中位数为 96.2%；深圳福田 CBD 样本为 19.12 分钟/24.9%，沙田站为
7.64 分钟/73.6%。北京、上海、芝加哥样本未映射到市场型设施，记为
“数据不足”，不得解释为“零供给”。

因此正式市场模型必须先过数据完整性 gate：

- OSM/官方设施数据缺失时，不能输出市场缺口结论；
- 必须区分市场厅、街市、邻里超市、大超市、周期和批发市场；
- 等权建筑只是测量代理，正式生成使用住户、就业和游客需求权重；
- 竞争、容量、营业时段、冷链和卸货条件仍由 Rust 市场模型计算。

### 9.4 市场实体化

市场计划必须产生：

- 市场厅/店面/超市的最终建筑实体；
- 入口和卸货面；
- 摊位/营业面积与容量；
- 服务分区和辐射字段；
- 入口或工程失败时的具名报告。

`market-radiation.json` 和审计图只用于验证；不是最终产品 UI 的替代物。

## 10. 土地压力与宏观工程

### 10.1 两层合同

**局部硬合同**继续固定：

- 道路最大填方 6m；
- 最大纵坡 8%；
- 最大步行坡 25%；
- 建筑 pad 最大局部削方 2m；
- 水体、保护区、岸线、入口和 footprint 规则。

**宏观工程**单独进入账本：

- 开山、台地、削坡、挡墙、填海、海堤、堤道；
- 桥梁、高架、隧道、涵洞和排水系统；
- 河道治理、湿地补偿和生态修复；
- 每项记录面积、体积、来源、目标标高、灾害、生态影响、预算和最终 diff。

宏观工程可以改变 N0，但不能静默放宽局部合同。

### 10.2 城市与时代系数

```text
landPressure(x,z) =
  settlementScale
  * networkCentrality
  * terrainScarcity
  * developmentDemand
  * protectionResistance
  * specialRegime
  * tourismSeasonality
```

建议的初始先验：

| 因子 | 范围 | 含义 |
|---|---:|---|
| village/town | 0.20–0.55 | 人力/小型机械，工程能力有限 |
| county/city | 0.55–1.10 | 市政工程与外溢 |
| metropolis | 1.10–1.60 | 轨道、桥隧、填海和高密更新 |
| megacity | 1.35–2.40 | 强工程能力，但保护/灾害约束也更高 |
| terrain scarcity | 0.88–1.82 | 来自 48 案压力先验，越缺地越受压 |
| protection resistance | 0.44–1.00 | 历史/生态保护越高，可改造度越低 |
| special regime | 1.02–1.40 | 特区、首都、金融/港口等制度因子 |

这些是范围先验，不是无限授权。工程规模最终由候选收益、预算、灾害、生态和
政治/保护约束共同决定。

## 11. 数据合同

### 11.1 新主入口

建议新增 `urbanEvolution`，负责阶段、需求、区域和建筑原型；旧的
`frontageProgram.requests.count` 只能用于显式测试夹具或声明式地标。

```json
{
  "urbanEvolution": {
    "version": 1,
    "stagePolicy": "natural-first",
    "naturalBaseline": "freeze-and-diff",
    "settlementScale": "metropolis",
    "slice": {
      "mode": "feature-cut",
      "focusFeature": "old-market-to-financial-waterfront",
      "cropReason": "express the old-core/waterfront/new-finance relationship"
    },
    "developmentDemand": {
      "households": 12000,
      "jobs": 9000,
      "retailFloorAreaM2": 85000,
      "freshFoodDemandUnits": 16000,
      "visitorPeak": 5000
    },
    "districtPolicy": "derive-from-natural-route-centred-demand",
    "countPolicy": "emergent-no-fill",
    "marketPolicy": "multi-tier-network",
    "engineeringPolicy": "explicit-macro-ledger"
  }
}
```

### 11.2 权威产物

- `natural-stage.json`
- `human-trace-audit.json`
- `route-evolution-plan.json`
- `settlement-candidates.json`
- `urban-evolution-plan.json`
- `parcel-plan.json`
- `building-program.json`
- `market-program.json`
- `market-catchment-field.json`
- `engineering-operation-ledger.json`
- `generation-trace.json`

所有拒绝、未满足需求、工程冲突和降级必须具名持久化。

## 12. 后续修改顺序

### W0 调查与证据矩阵

交付：

- 48 案统一字段和证据等级；
- 12 个代表性核心切面补充公开来源、URL、访问日期、单位和 crop；
- 先验数值与实测数值分离；
- `scripts/check-urban-case-priors.mjs` 升级为来源矩阵校验。

验收：48/48 结构通过；12 个代表案例每个至少两个独立来源；
不得把启发式数字写成官方统计。

### W1 N0 自然基线并入正式链

主要文件：

- `worldgen/natural_geography.rs`
- `worldgen/natural_only.rs`
- `worldgen.rs`

交付：每个场景冻结自然快照、hash、人工改造 diff 和
`humanTraceCount=0` gate。

提交：Conventional Commit `feat(worldgen): freeze natural baseline`。

### W2 地形路径与聚落候选

主要文件：

- `worldgen/route_evolution.rs`
- `worldgen/road_network.rs`
- `worldgen/urban_growth.rs`

交付：自然节点提取、多阶段路径、桥涵/盘山/平地网格判定、聚落候选接受/
拒绝报告。

验收：曲线绕行可解释；道路不穿水；桥/堤道/涵洞显式；0 聚落合法。

提交：`feat(worldgen): derive routes from natural nodes`。

### W3 城市网络中心与街区骨架

主要文件：

- `worldgen/planning_history.rs`
- 新 `worldgen/urban_evolution.rs`（或等价模块）

交付：网络中心场、道路拓宽、旧城保留、街区骨架、公共空间和新区候选。

验收：中心不是圆；旧路和父地块可追踪；大都市只输出声明切面。

提交：`feat(worldgen): evolve urban districts from network centres`。

### W4 地块与建筑原型

主要文件：

- `worldgen/settlement_evolution.rs`
- `settlement/development.rs`
- `settlement/morphology_compile.rs`
- `worldgen/epoch_evolution.rs`

交付：街区切地块、多建筑地块、原型选择、体量推导、需求交付和未满足账本。

验收：

- 同一需求在旧城/新区/郊区产生不同数量与体量；
- 需求增加时建筑数量/容量合理变化；
- 不允许按固定 count 补楼；
- 商业、住宅、办公、市场、学校、医院和宗教设施按各自规则出现。

提交：`feat(settlement): synthesize buildings from parcel capacity`。

### W5 多层级生鲜市场

主要文件：

- `worldgen/market_catchment.rs`
- 建筑实体化相关模块

交付：多类型市场、需求点、不同交通模式、竞争、负载、缺口、市场实体。

验收：5/10/15/30 分钟场可复核；容量与负载一致；缺口具名；不出现全图单店。

提交：`feat(worldgen): model tiered fresh-food markets`。

### W6 工程回写与联合审计

主要文件：

- `worldgen/land_pressure.rs`
- `worldgen/engineering_scale.rs`
- `worldgen/engineering*.rs`
- `worldgen/access.rs`

交付：局部硬合同 + 宏观工程账本 + 排水/挡墙/生态/基础 diff + 入口重检。

验收：金融塔、市场、公共建筑都有真实入口/裙房/广场；失败具名。

提交：`feat(worldgen): reconcile urban works with terrain`。

### W7 预览与视觉回归

主要文件：

- `adapters/html/`
- `server/`
- 验收截图与 `visual-feedback.json`

交付：自然基线、战略图、2.5D 近景、旧城街景、新区/滨水、市场服务图。

验收：不得出现大片空置、纯色白块、错接道路、悬空建筑、落水实体、chunk
棋盘格或“统计有建筑但画面没有”的情况。

提交：`feat(preview): expose natural-to-urban evolution evidence`。

每个 W 完成后更新本文件状态、`requirements-and-progress-master`、
`HANDOVER` 和相应验收记录，再推送远端。

## 13. 验收场景

| 场景 | 必须证明 |
|---|---|
| 山湖旧城—新区 1km | 自然基线、沿谷道路、旧城小地块、金融水岸、市场网络 |
| 江海老城—港区 1km | 河道/潮汐、港铁、旧城保留、工业更新、多级商业 |
| 山城两江 1km | 盘山路径、台地、挡墙、桥隧、步行垂直联系 |
| 平原县城 1km | 完整聚落、外围种植、田渠/农路、中心与社区市场 |
| 海岸旅游 512m | 居民/游客分离、季节需求、岸线保护、周期市场 |
| 牧场边缘 512m | 牧道、围栏、饮水、集散和低密服务 |
| 政治首都轴 1km | 公共轴线、安全退界、市民空间、混合城区 |
| 同 seed 制度对照 | 封闭小区/欧洲周边街区/美国郊区/日本町家/香港屋邨 |

每个场景统一检查：

- N0 无人工痕；
- 道路曲折来自地形，不是随机噪声；
- 建筑数量随需求/地块/原型变化；
- 旧城/更新/新区/郊区/旅游/农牧/保护在空间与功能上可辨认；
- 市场辐射由网络时距和容量竞争产生；
- 全部工程可回溯到自然基线；
- 预览无空置和错乱。

## 14. 迁移与兼容

保留：

- 1m Heightfield、毫米高度、chunk、sidecar、render stream；
- 现有 B/C 工程、入口、access graph、rules、审计和测试夹具；
- `naturalOnly`、`routeEvolution`、`settlementEvolution`、
  `planningHistory`、`marketCatchment` 的底层能力；
- 现有验收场景作为回归控制。

降级：

- `frontageProgram.requests.count` 不再作为普通城市生产入口；
- 单一 24m 临街单元不再作为所有街区的默认地块；
- 单一路线中心距离不再作为楼层和用途唯一依据；
- 单一全局市场容量不再作为全部市场类型。

兼容：

- 旧 Manifest 继续能烘焙，但报告标记 `legacy-fixed-program`；
- 新 Manifest 必须使用 `urbanEvolution`；
- 任何“具名真实城市重建”必须提供来源矩阵和 crop，否则拒绝宣称真实复刻。

## 15. 需要审核拍板的设计选择

请确认以下 8 项：

1. **自然先行：** 是否要求每个正式城市输出冻结 N0 基线、零人工痕审计和
   最终改造 diff。建议：是。
2. **建筑数量：** 是否彻底取消普通建筑固定完成数，只保留需求、地块容量、
   原型与交付结果。建议：是。
3. **建筑尺寸：** 是否接受同一地块的多栋/裙房塔楼/庭院/联排等形态，并把
   尺寸从地块和合同推导，而不是固定 24m×20m。建议：是。
4. **新旧城：** 是否要求旧城保留、更新、拆合、填充和道路拓宽都带父对象与
   cohort。建议：是。
5. **中心性：** 是否用真实路网/交通/历史/就业/公共设施中心场替代单一距离
   圆。建议：是。
6. **市场：** 是否接受菜场、便利店、邻里超市、大超市、周期和批发市场按
   容量/时距/竞争/季节需求分别建模。建议：是。
7. **48 城使用：** 是否按“全部机制先验 + 12 个代表案例达标来源 + 少量完整
   烘焙对照”推进，而不是 48 城都做完整世界。建议：是。
8. **工程权限：** 是否坚持局部 6m 填方、8% 纵坡、25% 步行坡、2m pad
   削方等合同不变，开山/填海/台地只作为宏观工程账本。建议：是。

## 16. 审核通过前的边界

审核通过前不做：

- 继续调预览颜色或把占位模型堆满；
- 扩大固定 frontage 数量；
- 为金融塔、市场或教堂放宽入口合同；
- 把 48 城先验写成已完成官方统计；
- 把大都市 1km 切面宣称为完整城市；
- 把未实现的设计写成完成状态。

审核通过后的第一步是 W0 证据矩阵和 W1 自然基线，不是先补建筑模型。

## 17. 结论

V3 的核心不是“多放一些房子”，而是让每个最终空间对象都能回答：

```text
自然为什么允许它在这里？
人类为什么从这里走？
聚落为什么在这里形成？
这个街区属于哪个时代？
为什么是这个用途、体量、数量和入口？
市场和公共服务为什么覆盖这里？
为它做了哪些工程，改写了多少自然基线？
```

只有这些答案同时存在，画面才会从“逻辑好像对、但不像现代城市”变成可解释的
自然—聚落—新城—旧城演进结果。
