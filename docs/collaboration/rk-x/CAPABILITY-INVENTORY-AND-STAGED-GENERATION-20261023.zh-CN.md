# GlyphWeave 现有能力清点与分阶段生成重构文档

状态：**待项目负责人审核**

核验基线：`refactor/authoritative-baked-audit`，提交 `02d4116`；
三城运行态证据为 `bevy/work/rk-x/*-r254-20261023`。

本文回答四件事：

1. 我们现在真正拥有哪些数据、代码、规则、资产和验收能力；
2. 哪些能力是生产路径，哪些只是可选路径、研究资料或未来接口；
3. 为什么继续在当前逻辑上打补丁会失序；
4. 下一步如何把“自然 → 道路 → 聚落 → 街区/地块 → 建筑/设施 → 工程与生态”
   拆成可分别验收的生成阶段。

本文不把历史文档里的目标当成现状，也不把一次通过的特例当成通用能力。

---

## 0. 结论先行

### 0.1 已经有的不是一个“完成的城市生成器”

目前已经有：

- 可烘焙的 Rust 世界数据核心、CLI、HTML 分块预览；
- 一套可解释的 48 城核心区先验和 1 km² 实测数据；
- 自然地理参数、地形高度场、采样排水图、水体和岸线合同；
- 以自然地形为成本面的道路寻路，以及道路等级、车道断面、纵坡和 OBB；
- 由人口/岗位/零售需求驱动、建筑数量不作为完成数的聚落规划；
- 旧城—更新—新区—外围的时代预算、街区/地块/建筑原型；
- 分级生鲜市场、真实路网时间、容量竞争和缺口报告；
- 挡土墙、涵洞、桥/高架、挖填账本、入口坡道和楼梯；
- 规则引擎、跨类型碰撞、最终实体审计、AccessGraph 和市场守恒审计。

但当前仍有**两套并行的生成入口**：

```text
路线/聚落演化路径
routeEvolution -> settlementEvolution -> epoch/planningHistory -> marketCatchment

旧道路/临街模板路径
roadNetwork -> automaticRoadExtensions -> frontageProgram / settlementProgram -> development
```

两条路径都会向同一个 `entities` 集合写入道路、建筑、入口和附属物。某些 Manifest
会同时触发两套规划，再由后置规则和材料化步骤叠加。因而不能把当前结果概括成
“一条已经统一的城市生成逻辑”。

这是当前最优先要解决的问题，优先于继续增加建筑样式、密度或视觉资产。

### 0.2 最应该先重构的是阶段依赖，不是再增加若干阈值

正确的主链应当是：

```text
世界意图与证据
  -> 自然地形/水文/地貌骨架
  -> 人类路径与工程骨架
  -> 聚落种子与网络中心
  -> 时代分区与用地结构
  -> 街区、地块与道路细网
  -> 需求、设施与生鲜市场
  -> 建筑体量与拆改/保留
  -> 入口、楼梯、桥隧和土方闭合
  -> 生态与街景细节
  -> 烘焙、静态审计、运行态验收
```

低阶段决定高阶段的可行域；高阶段不能悄悄回写低阶段的自然事实。所有对自然地形的
修改都必须进入工程账本，并可从自然层与最终层差分重建。

### 0.3 48 城目前是“机制先验 + 核心窗口观测”，不是 48 个完整复刻

- 48/48 均有至少两个独立公开来源入口；
- 48/48 均有 `source-audited-core-cut` 来源结论；
- 48/48 均有约 1 km² 的 OSM 当前快照测量；
- 48/48 的自然地理仍是 `mechanism-prior`，尚未全部升级为逐案官方地理测量；
- 没有 48 个完整城市已经烘焙或经过视觉验收；
- 当前 OSM building way 不展开 multipolygon，建筑数量只能视为下界；
- 城市平均密度不能直接铺进核心切面，核心区也不能直接当作整城。

---

## 1. 状态词与证据口径

本文使用以下状态词，避免把“有字段”误写成“已生成”：

| 状态 | 含义 |
|---|---|
| **生产路径** | 当前真实 Manifest/CLI 路径会消费，并参与最终实体或审计 |
| **可选生产路径** | 需要显式 Manifest 开关，未开启时保持旧语义 |
| **中间产物** | 已有阶段计划和报告，但尚未成为唯一消费链 |
| **接口/钩子** | 有类型或纯函数，可由未来场景接入，但不等于已普遍启用 |
| **研究/参考** | 只在设计文档或外部工程中参考，尚未照搬实现 |
| **未完成** | 当前没有稳定数据或通过验收的实现 |

本文复用项目的证据层级：

```text
static < compile < unit < fixture < production-world < cross-scale
       < consumer < independent
```

“预览里看得到”不能替代静态几何、地形、入口和碰撞审计。

---

## 2. 当前生成拓扑

### 2.1 实际烘焙顺序（按 `worldgen.rs`）

每个场景当前大致执行：

1. 生成/烘焙自然高度场；
2. 对自然高度场做 32m 采样排水图；
3. 冻结自然基线；
4. 运行 `routeEvolution`，产出路径、纵坡、桥梁跨和填方；
5. 运行 `settlementEvolution`，消费路径并生成需求、候选地块、建筑和密度曲线；
6. 运行 `epochEvolution`、`planningHistory` 和 `marketCatchment`；
7. 独立运行 `roadNetwork` 与 `development` 规划；
8. 运行旧模板实体生成器；
9. 追加人口设施、路线道路、聚落建筑、楼梯、入口铺装和市场；
10. 应用规划历史中的道路拓宽、中心和公园；
11. 做入口清理、土方、地形 carve、最终实体审计；
12. 输出 sidecar、gemap、报告和 HTML/Godot 适配器。

### 2.2 现有多真源风险

下列数据都可能影响最终道路或建筑：

- `routeEvolutionPlan`
- `settlementEvolutionPlan`
- `epochEvolutionPlan`
- `planningHistoryPlan`
- `roadNetworkPlan`
- `developmentPlan`
- `frontageProgram`
- `settlementProgram`
- `automaticRoadExtensions`
- 模板生成的 `generated.*` 实体
- HTML 预览中的补充绘制规则

风险不是“这些文件都存在”本身，而是当前没有统一的阶段消费合同明确保证：

- 后一阶段只消费前一阶段已冻结的结果；
- 同一种道路/建筑只能有一个规划真源；
- 模板填充只补充低优先级细节，不能重新定义主结构；
- 预览只读取已烘焙结果，不能形成独立造型逻辑。

因此下一步不应继续在末端叠加更多 retain/sanitize，而要先建立阶段闸门和失效传播。

---

## 3. 我们已经拥有什么

### 3.1 世界底座与存储

| 能力 | 当前实现 |
|---|---|
| 逻辑网格 | 水平 1m × 1m |
| 高度精度 | 0.25m；可发布毫米级实体标高 |
| 坐标 | `world_x` 东西、`world_z` 南北、`world_y` 高度，单位米 |
| 流式分块 | 默认 512m × 512m |
| 场景尺寸 | 最小 512m × 512m；默认 1km × 1km；最大 6km × 10km |
| 存储 | sidecar + `.gemap v3`；16³ Storage Chunk，32×32 Region |
| 主要输出 | `world.json`、Manifest、generation/quality/scale 报告、场景 plan、sidecar、HTML/Godot |
| 确定性 | 固定 seed；历史最大图曾完成逐字节复烘焙一致性 |
| 唯一内核 | Rust Core/CLI 为地图真源；HTML/Godot 为适配器 |

### 3.2 自然地形与地理控制

已有自然参数包括：

- 区域起伏 `regionalReliefM`
- 造山轴与抬升偏置
- 海岸类型、浪蚀、陆架和潮差
- 气候带、主风向、季风、降雨、季节性与雨影
- 河流体系、河道迁移、湿地偏好、河网密度、曲流和洪泛宽度
- 喀斯特、风成、冰川、深切河谷、滑坡
- 自然灾害标签
- 地貌过程与证据字段

当前地形合成：

- 多尺度 `value_noise2d` / `fbm2d`
- 山脊—谷地骨架
- 距海岸控制
- 确定性的近似河谷雕刻
- 湿度、坡度和接近水体影响地表材质
- 1m 高度场完成后再生成 32m 采样排水图

重要限制：

- 排水图目前是 `sampled-steepest-descent-preserve-sinks-v2`，不是水力侵蚀或洪水模型；
- 侵蚀字段明确标记为 `deterministic-valley-carving-approximation`；
- 排水图主要是**事后审计图**，尚未成为完整地形生成的首要根据；
- 山脊、谷地、冲积扇、阶地、三角洲、岩性等尚未全部形成一等自然对象；
- `HabitatContext` / `HabitatSuitability` 已存在，但主要是未来生态选择的接口。

自然专用模式 `naturalOnly`：

- 有正向 allow-list：树、灌木、岩石、草、芦苇、倒木、水体、岸线、湿地等；
- 出现道路、建筑、桥梁、街具、农业工程或地形工程即失败；
- 要求声明 `naturalGeography`；
- 已有多地理自然切面证据，证明“零人工痕”可以作为独立阶段验收。

### 3.3 水体与岸线

已有合同：

- 水体类型、水平基准/河道坡降、流向和流速
- 岸线过渡宽、浅水带、平均/通航水深
- 波幅、波长、方向、roughness
- 河/湖 footprint 与陆生实体禁区
- 默认 4m 岸线公共净空
- 海岸公共步道、湿边、浅水、桥/堤/码头接口
- 水体内部禁止建筑、树、灌木和岩石；芦苇、桥等有独立例外合同

当前不足：

- 结构化水体仍可能走独立 road-first 路径，未与自然排水骨架完全同源；
- 河床、水面、护岸和最终工程地形还不是所有场景的统一几何；
- 冲积、侵蚀、潮汐、洪泛和岸线变化尚未驱动最终水陆边界。

### 3.4 道路与交通

`routeEvolution` 已有：

- 自然高度场上的路径搜索；
- 坡度、曲率、转向、水体和桥成本；
- `terrain-following`、`straight`、`straight-preferred`；
- 路径 polyline 与可重建纵断面；
- 填方量、桥跨、最大纵坡和最大地形填方；
- 显式 `roadClass` 与道路总路权。

默认局部合同：

| 参数 | 当前默认/限制 |
|---|---:|
| 道路纵坡 | 8% |
| 路线允许地形坡 | 35% |
| 单段地形填方 | 6m 内 |
| 桥跨 | 默认上限 300m，可声明至 2,000m |
| 规划网格步长 | 16m，可设 8–64m |
| 道路宽度 | 4–32m，偶数 |
| 转向代价 | 1.4，可设 0–8 |

道路等级与断面：

| 等级 | 最小路权 | 车道 | 车道宽 | 中央隔离 | 人行道/侧 | 路肩/侧 | 限速 |
|---|---:|---:|---:|---:|---:|---:|---:|
| expressway | 28m | 6 | 3.5m | 3.0m | 2.5m | 1.2m | 100 |
| national | 20m | 4 | 3.5m | 1.6m | 2.5m | 0.8m | 80 |
| county | 14m | 4 | 3.25m | 0 | 1.5m | 0.5m | 60 |
| township | 8m | 2 | 3.2m | 0 | 1.2m | 0.25m | 40 |
| local | 6m | 2 | 3.2m | 0 | 0.8m | 0 | 30 |
| lane | 4m | 1 | 3.2m | 0 | 0.3m | 0 | 20 |

已有但未完全闭合：

- 道路 OBB 与设计标高分离；
- 分段道路碰撞链；
- 高差交叉发布为 `overpass`；
- 路口盖面、去重相交和附属带裁断；
- 直线/地形拟合 A/B 试验。

尚未完成：

- 完整路口并集多边形；
- 转盘、渠化岛和完整互通模板；
- 桥头挡墙、桥面排水、护栏全合同；
- 隧道断面及洞门；
- 按纵向坡度插值的连续第一人称碰撞；
- 高速/主干路在平地“该直就直”的统一决策器。

### 3.5 聚落、需求与建筑

`settlementEvolution` 已明确使用：

```text
人口/岗位/零售面积 -> 需求
需求 + 可建地块 + FAR + 入口条件 -> 建筑数量
```

建筑数量不是作者指定完成数，而是结果。`recommendedCount` 只保留为形态和密度核对参考。

默认聚落参数：

| 参数 | 默认 |
|---|---:|
| 地块宽 | 24m，可设 8–80m |
| 地块深 | 28m，可设 8–80m |
| 内部路网步长 | 36m，可设 8–120m |
| 退界 | 12m，可设 1–32m |
| 地块最大坡 | 25% |
| FAR | 1.6，可设 0.2–8.0 |
| 最大层数 | 18，可设 1–60 |
| 入口高差容差 | 6m |
| 默认局部 pad 挖方 | 2m；当前 Manifest 可设至 30m |

人口程序已支持：

- 居民数、日间岗位数、家庭规模；
- 人均零售面积；
- 学校、诊所、医院、教堂、社区中心、体育中心、体育场等设施；
- 按居民、岗位或日间人口计算设施请求数；
- 设施数量上限与显式宽/深/高。

香港核心控制示例使用：

- 45,212 居民；
- 80,000 日间岗位；
- 家庭规模 2.82575；
- 人均零售 8.8474m²；
- 推导约 16,000 户、80,000 岗位和约 400,009m² 零售面积。

这些数值是输入基线，不是 48 城普遍常数。

### 建筑原型

当前核心原型控制宽、深、层数和覆盖率：

| 原型 | 宽 | 深 | 层数 | 覆盖率 | 推荐数 |
|---|---:|---:|---:|---:|---:|
| 办公塔/裙房 | 20–48m | 24–44m | 10–42 | 0.62 | 1 |
| 混合住宅中层 | 12–30m | 20–34m | 4–10 | 0.76 | 2 |
| 旧城窄排屋 | 5–14m | 24–36m | 2–5 | 0.88 | 3 |
| 欧洲历史周边式 | 18–46m | 24–34m | 3–7 | 0.82 | 2 |
| 新城中层 | 24–48m | 20–34m | 6–18 | 0.62 | 2 |
| 更新街区 | 16–38m | 18–34m | 4–12 | 0.76 | 2 |
| 美国独立住宅 | 8–18m | 10–26m | 1–3 | 0.28 | 1 |
| 香港屋邨平台 | 16–32m | 20–34m | 10–28 | 0.68 | 2 |
| 中国门禁板楼 | 28–64m | 12–26m | 6–20 | 0.28 | 2 |
| 日本町家 | 4–12m | 8–24m | 2–4 | 0.76–0.82 | 3 |
| 欧洲周边街区 | 24–68m | 16–34m | 4–9 | 0.66 | 1 |
| 临街商铺 | 4–70m | 8–32m | 2–8 | 0.68–0.78 | 1–2 |

### 时代与新旧城

`epochEvolution` 默认半径：

- old-core：160m
- expansion/renewal：320m
- modern：560m

允许阶段：

```text
founding
old-core
expansion
industrial
modern-new
renewal
```

操作包括：

- `new`
- `preserve`
- `modify`
- `demolish`
- `replace`

当前已实现时代预算、父对象、建设 cohort 和最终实体继承。

当前不一致：

- epoch 的 `expansion / industrial / modern-new / renewal`；
- district demand 的 `old-core / renewal / new-growth / periphery`；

是两套相邻但不完全相同的分类。重构时应合并为一个受控的分类法和映射层。

### 重规划

大型设施在无空地块时，可以拆除普通建筑后重建。当前替换权重为：

```text
0.35 × age
+ 0.25 × lowDensity
+ 0.18 × poorAccess
+ 0.14 × centrality
+ 0.08 × lowRise
```

历史原型、市场、civic 设施不随普通住宅替换。每次替换必须记录父对象与被拆对象。

### 3.6 生鲜市场与服务网络

市场模型是真实道路图上的时间、容量和竞争模型，不是服务半径圆。

默认：

| 参数 | 默认 |
|---|---:|
| 出行时间上限 | 15 分钟 |
| 步行速度 | 1.3m/s |
| 单市场容量 | 280 |
| 最大市场数 | 4 |
| 候选间距 | 80m |
| 覆盖目标 | 85% |
| 竞争系数 `beta` | 0.08 |

已发布的统计带：

```text
5 / 10 / 15 / 30 分钟
```

三份权威报告：

- `market-catchment-plan.json`
- `market-radiation.json`
- `market-deficit.json`

已实现市场层级：

- 社区菜场/湿货市场
- 邻里超市
- 大超市
- 周期市场
- 批发/集散市场

容量、竞争、负载和缺口均可独立复算。当前市场仍是静态设施规划，不是摊位经营、
营业时间、排队和真实价格模拟。

### 3.7 工程改造

默认局部约束：

| 工程能力 | 局部默认 |
|---|---:|
| 最大道路纵坡 | 8% |
| 最大步行坡 | 25% |
| 最大路面填方 | 6m |
| 最大局部 pad 挖方 | 2m |
| 入口前场 | 2m |
| 岸线净空 | 4m |
| 建筑最大坡 | 12%，不同对象可更严 |
| 商铺最大坡 | 8% |

已存在的工程证据：

- 挖填账本；
- 道路设计面与最终 heightfield 差分；
- 挡土墙、涵洞、桥、高架和跨线；
- 路堤预算；
- 地形 carve；
- 地块整平；
- 入口台阶/坡道；
- 工程后排水对比；
- `feature-cut` 宏观工程授权。

工程授权规则：

- 普通地块/道路保留局部合同；
- `routeEvolution.maxFillM > 6m` 需要 `urbanSlice.mode = "feature-cut"`；
- 特征切面可标记 `noAbsoluteEarthworkCap = true`；
- 这并不免除工程体积、灾害、排水、保护区、水域和岸线审计；
- 当前宏观授权范围包括 special-area、reclamation、hillside 和一般 feature-cut。

### 3.8 碰撞、入口与楼梯

### 碰撞

当前最终审计包括：

- 建筑—建筑 OBB 重叠；
- 建筑—主道路 OBB 重叠；
- 建筑—树/灌木/草/岩石/倒木 OBB 重叠；
- 建筑完整 footprint 是否有 baked height 支撑；
- 悬空；
- 地形穿入建筑基座；
- 建筑落水；
- 道路 profile 与实体不一致；
- 跨分块地形连续性；
- 规则审计中的水、坡、地表、biome、hazard、入口净空和通用碰撞。

最终审计数值阈值：

- 建筑—建筑：0.05m 分离；
- 建筑—主道路：0.25m 分离；
- 建筑—自然细节：0.05m 分离；
- 建筑贴地容差：最终审计至少 0.5m；
- 道路中心线/设计面必须能由 profile 重建。

### 入口与楼梯

AccessGraph 合同：

- 步行路线半宽 0.75m；
- 搜索步长 2m；
- 最大路线长度 120m；
- 最大可行坡度 25%；
- 地形每个 0.5m 采样；
- 路径不得穿过建筑、树、岩石、围墙、水体和封闭设施；
- 没有验证入口的生成建筑必须被拒绝，不能留在最终世界。

楼梯合同：

| 参数 | 当前值 |
|---|---:|
| 单级理论踏步高 | 0.18m |
| 短入口高差触发 | 0.30–2.40m |
| 主入口楼梯宽 | 2.4m |
| 侧入口楼梯宽 | 2.0m |
| 单段楼梯行驶长度 | 至多约 22m |
| 整体楼梯 rise 上限 | `8m × ceil(run/22m)` |
| 楼梯实体尺寸 | 宽 1.2–24m、深 1–24m、高 0.2–8.5m |

楼梯必须从建筑门槛出发，终止于道路 OBB 边缘或步行铺装，不能跨车道中心线。

---

## 4. 48 城数据清点

### 4.1 数据文件

| 文件 | 内容 |
|---|---|
| `assets/data/urban-case-priors-48.json` | 48 个核心切面的自然地理、三段历史、旧/更新/新/边缘结构、中心、社区、特殊因子、土地压力先验 |
| `assets/data/urban-case-evidence.json` | 48 案来源覆盖、81 个来源条目、supported mechanisms、测量文件和缺口 |
| `assets/data/urban-core-measurements/*-1km2.json` | 48 个圆形 1km² 核心窗口的当前 OSM 测量 |
| `assets/data/urban-history-patterns.json` | 三段历史关键词与阶段统计 |
| `assets/data/urban-pressure-patterns.json` | 规模、特殊因子和地形组先验统计 |
| `scripts/check-urban-case-evidence.mjs` | 来源和案件覆盖校验 |
| `scripts/check-urban-core-measurements.mjs` | 48 个测量窗口结构校验 |
| `scripts/summarize-urban-history.mjs` | 历史关键词复算 |
| `scripts/summarize-urban-pressure.mjs` | 土地压力分组复算 |

一键检查：

```powershell
pnpm check:urban-cases
```

### 4.2 48 案先验字段

每个案件当前包含：

```text
id
region
coreCut
naturalGeography.setting
naturalGeography.regionalReliefM
naturalGeography.climateBand
naturalGeography.geomorphicProcesses[]
naturalGeography.hydrography
naturalGeography.hazards[]
naturalGeography.evidenceTier
history[3].era / change
urbanStructure.oldCore
urbanStructure.renewal
urbanStructure.newCity
urbanStructure.edge
urbanStructure.centres[]
focusFeature
communityProfile
specialFactors[]
landPressurePrior.settlementScale
landPressurePrior.terrainScarcity
landPressurePrior.protectionResistance
landPressurePrior.specialRegime
evidence.sources[]
evidence.confidence
```

区域分布：

| 区域 | 案件数 |
|---|---:|
| 中国 | 12 |
| 美国 | 12 |
| 欧洲 | 12 |
| 日本 | 6 |
| 香港 | 6 |
| 合计 | 48 |

规模先验：

| settlementScale | 案件数 |
|---|---:|
| megacity | 19 |
| city | 18 |
| metropolis | 10 |
| town | 1 |

社区原型：

| communityProfile | 案件数 |
|---|---:|
| europe-perimeter | 12 |
| china-gated | 10 |
| us-mixed-income | 9 |
| hong-kong-estate | 6 |
| japan-machiya | 6 |
| us-suburban | 3 |
| china-urban-village | 2 |

历史统计：

- 40/48 的首段与水港、河湖或治水相关；
- 26/48 的中段出现铁路或轨道；
- 26/48 的第三段出现更新、保护或重建。

该统计是关键词辅助总结，不替代逐案历史考证。

### 4.3 来源覆盖

当前 `urban-case-evidence.json`：

- 48/48 案件均登记为 `source-audited-core-cut`；
- 81 个来源条目；
- 69 个不同 publisher；
- 67 个 `direct-200`；
- 14 个 `reader-verified`；
- 37 案有 2 个来源，11 案有 3 个来源；
- 每案至少两个不同发布者；
- 48/48 有约 1 km² 的测量文件；
- 48/48 measurementStatus 当前为 `partial-osm`。

主要来源类型：

- 政府总体规划、法定图则、规划标准和开放数据；
- 市政空间数据与交通规划；
- UNESCO 历史/遗产说明；
- 香港 HKPSG、规划署、房屋署、市区更新和郊野公园资料；
- OSM 当前几何测量；
- 形态研究、公开市场数据。

来源限制必须保留：

- 官方规划文件说明机制，不等于当前建筑清点；
- OSM 是当前协作地图快照，不是历史人口普查；
- 名称或地址存在，不证明设施容量或营业状态；
- 来源存在，不等于该城已经复刻。

### 4.4 每个 1 km² 窗口的测量字段

48 个文件均含以下字段：

```text
roadCenterlineLengthM
roadDensityKmPerKm2
drivableCenterlineLengthM
drivableDensityKmPerKm2
roadLengthByClassM
buildingCountInsideCircleCentroid
estimatedBuildingFootprintM2
estimatedBuildingCoverageRatio
buildingLevelsObservedCount
buildingLevelsObservedRatio
estimatedObservedFloorAreaM2
observedFloorAreaRatioProxy
freshFoodFacilityCounts
freshFoodNetworkAccess
```

测量口径：

- 圆形窗口面积 1km²，半径 564.1896m；
- 建筑按 centroid inclusion；
- 建筑数量为 way-based，不展开 multipolygon；
- footprint 与覆盖率是近似投影面积；
- FAR 仅使用有显式层数标签的建筑，是下界代理；
- 道路密度是中心线长度，不含车道容量和路权面积；
- 鲜食覆盖是 1.3m/s 步行网络代理；
- 鲜食设施容量、竞争、营业时间和品质不在 OSM 样本内。

### 4.5 当前 48 窗口汇总（本轮重新读取 JSON）

| 指标 | N | 最小值 | 中位数 | P75 | 最大值 |
|---|---:|---:|---:|---:|---:|
| 建筑数量 | 48 | 48 | 405.5 | 937 | 4,153 |
| 建筑覆盖 | 48 | 0.02 | 0.32 | 0.37 | 0.57 |
| 可驾驶中心线密度 km/km² | 48 | 12.10 | 20.31 | 23.36 | 36.70 |
| 市场型鲜食 10 分钟覆盖 | 42 | 0.16 | 0.96 | 0.98 | 1.00 |
| 观测 FAR 代理 | 48 | 0 | 1.23 | 2.25 | 4.48 |
| 有显式层数建筑比例 | 48 | 0 | 0.28 | 0.58 | 0.95 |

历史文档曾写建筑中位数为 410。本文按当前 48 个 JSON 重新计算为 405.5；
偶数样本中位数是中间两项的平均值。任何相关数字都必须同时注明数据快照和
分位数口径，不能再把不同轮次的取整值混用。

6 个窗口没有可路由的市场型鲜食点，保留为 null，不记为 0 覆盖：

- 北京王府井
- 上海外滩
- 东京湾台场
- 芝加哥 Loop
- 拉斯维加斯 Strip
- 华盛顿白宫

### 4.6 48 窗口逐案观测

列说明：

- `建筑`：centroid inclusion 的建筑 way 数；
- `覆盖`：building footprint / 1km²；
- `可驾驶密度`：km/km²；
- `10m 鲜食`：市场型设施步行网络 10 分钟覆盖；
- `FAR`：有层数标签建筑的观测代理。

| 测量 caseId | 核心窗口锚点 | 建筑 | 覆盖 | 可驾驶密度 | 10m 鲜食 | FAR |
|---|---|---:|---:|---:|---:|---:|
| cn-beijing-wangfujing | Wangfujing, Beijing | 283 | 29% | 15.4 | — | 1.13 |
| cn-chengdu-tianfu-square | Tianfu Square, Chengdu | 262 | 29% | 20.3 | 81% | 1.76 |
| cn-chongqing-jiefangbei | Jiefangbei, Chongqing | 233 | 31% | 20.3 | 61% | 0.74 |
| cn-guangzhou-beijing-road | Beijing Road Pedestrian Street | 280 | 26% | 16.7 | 41% | 1.17 |
| cn-hangzhou-wulin-square | Wulin Square, Hangzhou | 93 | 21% | 18.8 | 16% | 0.00 |
| cn-kunming-nanping-street | Nanping Street, Kunming | 315 | 24% | 15.8 | 97% | 0.66 |
| cn-qingdao-old-town | St. Michael Cathedral, Qingdao | 752 | 34% | 17.4 | 98% | 0.58 |
| cn-shanghai-bund | The Bund, Shanghai | 301 | 19% | 21.4 | — | 1.25 |
| cn-shenzhen-futian-cbd | Futian CBD, Shenzhen | 177 | 35% | 22.7 | 25% | 2.25 |
| cn-wuhan-jianghan-road | Jianghan Road, Wuhan | 123 | 16% | 17.6 | 98% | 0.00 |
| cn-xiamen-zhongshan-road | Zhongshan Road, Xiamen | 92 | 10% | 19.3 | 88% | 0.05 |
| cn-xian-bell-tower | Bell Tower, Xian | 48 | 2% | 12.3 | 54% | 0.00 |
| eu-amsterdam-dam-square | Dam Square, Amsterdam | 2995 | 46% | 18.9 | 100% | 0.11 |
| eu-barcelona-catalunya | Placa de Catalunya, Barcelona | 882 | 32% | 18.8 | 100% | 1.46 |
| eu-berlin-alexanderplatz | Alexanderplatz, Berlin | 283 | 26% | 22.4 | 98% | 1.38 |
| eu-lisbon-comercio | Praca do Comercio, Lisbon | 1007 | 25% | 14.6 | 96% | 0.73 |
| eu-london-bank | Bank of England, London | 721 | 57% | 24.9 | 99% | 3.07 |
| eu-milan-duomo | Duomo di Milano | 512 | 28% | 19.3 | 94% | 0.58 |
| eu-paris-hotel-de-ville | Hotel de Ville, Paris | 1085 | 25% | 20.7 | 96% | 0.91 |
| eu-prague-old-town | Old Town Square, Prague | 942 | 37% | 18.3 | 97% | 1.21 |
| eu-rome-piazza-venezia | Piazza Venezia, Rome | 1016 | 23% | 20.1 | 44% | 0.12 |
| eu-stockholm-gamla-stan | Gamla Stan, Stockholm | 565 | 18% | 17.4 | 97% | 0.55 |
| eu-vienna-stephansplatz | Stephansplatz, Vienna | 553 | 26% | 18.2 | 99% | 1.02 |
| eu-zurich-hauptbahnhof | Zurich Hauptbahnhof | 825 | 33% | 26.5 | 98% | 0.94 |
| hk-causeway-bay | Causeway Bay, Hong Kong | 477 | 27% | 21.8 | 100% | 2.73 |
| hk-central-statue-square | Statue Square, Hong Kong | 410 | 30% | 32.7 | 98% | 3.19 |
| hk-kwun-tong | Kwun Tong, Hong Kong | 401 | 40% | 23.4 | 96% | 4.48 |
| hk-mong-kok | Mong Kok, Hong Kong | 1370 | 45% | 29.3 | 96% | 3.14 |
| hk-sai-kung | Sai Kung, Hong Kong | 937 | 13% | 12.1 | 97% | 0.05 |
| hk-sha-tin-station | Sha Tin Station, Hong Kong | 379 | 22% | 14.3 | 74% | 0.94 |
| jp-kobe-sannomiya | Sannomiya, Kobe | 2406 | 38% | 33.3 | 100% | 1.46 |
| jp-kyoto-gion | Gion, Kyoto | 4004 | 37% | 22.4 | 94% | 0.08 |
| jp-osaka-namba | Namba Station, Osaka | 2678 | 46% | 28.5 | 99% | 1.57 |
| jp-tokyo-asakusa | Senso-ji, Tokyo | 4153 | 37% | 36.7 | 100% | 0.67 |
| jp-tokyo-bay-odaiba | Odaiba, Tokyo | 86 | 15% | 18.6 | — | 0.32 |
| jp-yokohama-minato-mirai | Minato Mirai, Yokohama | 149 | 32% | 18.9 | 97% | 3.43 |
| us-boston-common | Boston Common | 1154 | 35% | 19.3 | 99% | 2.02 |
| us-chicago-loop | Chicago Loop | 325 | 23% | 20.5 | — | 2.37 |
| us-denver-union-station | Denver Union Station | 386 | 33% | 24.9 | 99% | 2.46 |
| us-las-vegas-strip | Las Vegas Strip | 84 | 53% | 21.8 | — | 2.38 |
| us-los-angeles-downtown | Downtown Los Angeles | 363 | 45% | 24.0 | 54% | 3.49 |
| us-miami-downtown | Downtown Miami | 247 | 37% | 22.8 | 70% | 3.76 |
| us-new-orleans-french-quarter | French Quarter | 1644 | 38% | 22.6 | 91% | 0.12 |
| us-new-york-times-square | Times Square, New York | 784 | 57% | 18.5 | 94% | 2.25 |
| us-portland-pioneer-square | Pioneer Courthouse Square | 395 | 42% | 31.3 | 71% | 3.75 |
| us-san-francisco-union-square | Union Square, San Francisco | 813 | 52% | 23.7 | 80% | 1.86 |
| us-seattle-pike-place | Pike Place Market | 320 | 35% | 23.4 | 96% | 1.73 |
| us-washington-dc-white-house | White House, Washington DC | 240 | 26% | 19.9 | — | 2.09 |

### 4.7 用地压力先验

| 规模 | 案件数 | terrainScarcity 中位 | protectionResistance 中位 | specialRegime 中位 |
|---|---:|---:|---:|---:|
| megacity | 19 | 1.38 | 0.78 | 1.20 |
| city | 18 | 1.42 | 0.91 | 1.18 |
| metropolis | 10 | 1.38 | 0.82 | 1.19 |
| town | 1 | 1.72 | 0.98 | 1.12 |

当前实现的城市规模系数：

| settlementScale | scaleFactor |
|---|---:|
| village | 0.20 |
| town | 0.35 |
| county-city | 0.52 |
| city | 0.70 |
| metropolis | 0.92 |
| megacity | 1.08 |

特殊因子权重：

| 因子 | 权重 |
|---|---:|
| special-economic-zone | 0.22 |
| mountainous | 0.20 |
| coastal-reclamation | 0.18 |
| financial-center | 0.12 |
| port-industrial | 0.12 |
| island | 0.10 |
| political-capital | 0.10 |
| historic-core | 0.08 |
| tourism | 0.06 |

土地压力公式：

```text
centrePressure = 0.88 + networkCentrality * 0.32

terrainScarcity =
  clamp(
    1
    + max(reliefScale - 0.7, 0) * 0.22
    + coastality * 0.18
    + wetlandBias * 0.10
    + average(massWasting, fluvialIncision, coastalErosion) * 0.16,
    0.85,
    1.80
  )

specialRegime = clamp(1 + sum(specialFactorWeights), 1.0, 1.65)

landPressureIndex =
  clamp(
    scaleFactor
    * centrePressure
    * terrainScarcity
    * (1 + protectionResistance * 0.18)
    * specialRegime
    * tourismSeasonality,
    0.15,
    2.40
  )

engineeringScale = clamp(0.45 + landPressureIndex * 0.90, 0.45, 2.40)
```

这些系数是**可解释先验**，不是经因果检验的城市定律。`urban-pressure-patterns.json`
也明确注明这些组间统计重叠严重，不能当作线性回归系数。

---

## 5. 已有建模资源与地图占位尺寸

### 5.1 外部导入资源

注册表：`assets/glyphweave.registry.json`

当前只导入 7 个正式原型：

| assetId | 文件 | 源几何包围盒约值 | 地图使用 |
|---|---|---:|---|
| prop.tree | `CommonTree_1.gltf` | 4.31 × 4.58 × 7.26m（W×D×H） | 运行时按实体树冠宽深 2–5.5m、高 4–9m 归一化；生态树大量实例化 |
| prop.bush | `Bush_Common.gltf` | 1.92 × 1.97 × 1.58m | 按 0.6–2.6m 级别归一化 |
| prop.rock | `Pebble_Round_1.gltf` | 0.50 × 0.37 × 0.095m | 按岩石实体尺寸缩放；规则占位 2×2×1.2m |
| prop.building | `2Story_GableRoof.obj` | 2.16 × 2.38 × 3.13 | 归一化到实体 |
| prop.building.small | `1Story_GableRoof.obj` | 2.10 × 2.38 × 2.02 | 归一化到实体 |
| prop.building.wide | `2Story_Wide.obj` | 3.83 × 2.25 × 2.75 | 归一化到实体 |
| prop.building.round | `2Story_RoundRoof.obj` | 2.11 × 2.13 × 2.90 | 归一化到实体 |

重要技术债：

- 建筑 OBJ 的源包围盒远小于现实建筑，注册表却声明 `unit = meter`；
- 当前渲染实际靠 `fitAssetToEntity` 把 asset 强制塞入实体的
  `widthM/depthM/heightM`，所以视觉正确不代表注册表单位正确；
- 应在后续把注册表改成 `modelSpace` + `sourceBoundsM` + `normalization`，
  禁止消费者自行猜测缩放。

### 5.2 程序化资源

| 资源 | 当前生产方式 | 主要占位/尺寸依据 |
|---|---|---|
| 地形 | 连续 heightfield Mesh | 1m 数据、0.25m 高度精度 |
| 草 | GPU instanced crossed ribbons | 三档高度与 curl；地形/焦点密度调制 |
| 树 | 程序化实例或 Quaternius 原型 | 树冠直径而不是树干直径 |
| 灌木/岩石 | 程序化实例 | 单独地表、碰撞和坡度合同 |
| 道路 | Rust polyline + 横断面 ribbon | 4–32m 总路权；独立 OBB |
| 人行道/路肩/隔离 | 路面横断面子材质 | 属于总路权，不能超出碰撞宽 |
| 建筑 | OBJ/程序化体量 + 立面规则 | 以实体 W/D/H 为唯一尺寸 |
| 店铺门面 | 程序化体块/窗带/招牌 | 典型店面 8–24m，当前合同 6–120m |
| 水体 | 独立水面材质/几何 | 水平基准或明确纵坡 |
| 楼梯 | 程序化矩形楼梯段 | 见 3.8 |
| 桥/高架 | 结构实体 + 桥面 + 碰撞 | span、clearance、支座/挡墙仍需补全 |

当前没有独立成品模型库覆盖：

- 学校；
- 医院；
- 教堂；
- 体育场；
- 火车站；
- 港口吊机；
- 完整立交/转盘；
- 多种建筑年代与地域风格。

这些大多仍由程序化体量或通用建筑原型表达。

### 5.3 规则对象描述符

`assets/objects/*.object.toml` 当前有 22 个描述符，覆盖：

```text
bridge
building
canal
civic-public
civic
coastal-building
coastal-landform
coastal-landscape
coastal-linear
commerce
education
industry
landscape
parking
river
road
rock
storefront
transport
tree
utility
water-edge
```

基准占位：

| 对象 | footprint | 高度 | 备注 |
|---|---:|---:|---|
| 普通建筑 | 12×10m | 7m | 最大坡 12%，入口 0.5m 净空 |
| 商铺 | 8×6m | 4m | 最大坡 8%，距路 footprint 8m 内 |
| 树 | 4×4m | 8m | 树冠直径；最大坡 70% |
| 岩石 | 2×2m | 1.2m | 最大坡 40% |
| 道路 | 8×8m | 0.5m | 最大坡 8%，安全净空 2m |
| 河 | 40×512m | 1m | 名义断面，审计读取实体自身 |
| 桥 | 8×8m | 6m | 可跨水，不避建筑 |
| 路灯/长椅/路牌等 | 1×1m | 3m | 统一 utility 描述符 |
| 芦苇 | 2×2m | 1.5m | 可进入水域 |

描述符尺寸只用于规则引擎和基准核对。世界生成中的最终尺寸必须来自实体自身的米制
`widthM/depthM/heightM`，不能把 TOML 占位尺寸当作所有建筑统一样式。

---

## 6. 已实现的算法

| 算法/模型 | 当前用途 | 状态 |
|---|---|---|
| 多尺度 value noise + fBm | 地形残差、山脊、湿度、材质扰动 | 生产路径 |
| 山脊/谷地近似骨架 | 连续地形轮廓 | 生产路径，但不是完整构造地貌 |
| 确定性河谷雕刻 | 自然沟谷表现 | 生产路径；近似模型 |
| 32m 最陡下降采样 | 排水节点、边、盆地和面积 | 生产路径；事后审计为主 |
| 地形坡/曲率/距离场 | 可建性、道路成本、材质 | 生产路径 |
| 带坡度/转向/水体/桥梁成本的路线搜索 | `routeEvolution` | 生产路径 |
| 规则驱动的候选放置与事务式提交 | 对象放置 | 生产路径 |
| SAT/OBB 相交 | 建筑、道路和细节碰撞 | 生产路径 |
| 约束 A* | 建筑入口到道路的步行检查 | 生产路径 |
| 多源 Dijkstra | 网络中心、市场和设施时距 | 生产路径 |
| 容量竞争分配 | 生鲜市场覆盖和负载 | 生产路径 |
| 街区/地块程序切分 | 建筑前沿和内部地块 | 生产路径 |
| FAR + 人口 + 岗位需求消费 | 建筑数量和用途 | 生产路径 |
| 时代预算与拆改/保留 | 新老城区 | 生产路径 |
| 工程挖填账本 | 道路、地块、入口和局部整平 | 生产路径/可选 |
| 实例化渲染 | 草、树、岩石、水体等 | HTML 适配器 |
| CGA 风格立面组合 | 店铺/建筑窗带和立面 | 渲染层简化实现，不是通用 CGA 解释器 |

---

## 7. 曾经参考过的工程与算法

| 参考 | 借鉴内容 | 当前项目状态 | 不能直接照搬 |
|---|---|---|---|
| Red Blob Games Mapgen4 | 山峰距离场、海陆约束、下坡流、汇水、湿度 | 已借鉴“自然先行/图结构”；当前排水是 32m 采样，不是 dual mesh | Mapgen4 是“好看优先”的宏观地图，不是工程地形真值 |
| Azgaar Fantasy Map Generator | 高度图改变后重建所有派生数据 | 设计原则采用；尚未有完整依赖失效图 | 宏观地图，不是建筑级生成器 |
| FastNoiseLite / OpenSimplex2 / Perlin / Cellular | 连续噪声、频率和 seed | 当前使用自研 value noise/fBm | 噪声不能定义山脊、汇水和城市结构 |
| 水滴侵蚀开源实现 | 质量守恒、侵蚀、沉积 | 仅参考，当前为确定性近似河谷 | 大地图成本高、边界敏感 |
| FullSWOF/地表流模型 | 坡面流和汇水 | 仅参考，不是当前模型 | 不能把审计图冒充水动力模型 |
| symbios-ground-lab `carve_roads` | 道路走廊地形雕刻 | 已实现简化版 bounded carve 和工程账本 | 不是当前可直接复用的代码依赖 |
| symbios-ground-lab CGA | Split / Extrude / Comp / Repeat | HTML 有简化 CGA 风格立面；Core 无通用 CGA | 不能把 UI 材质规则当建筑生成语法 |
| Poisson disk / 蓝噪声 | 自然物体分布 | 设计参考；现有生态仍有密度预算和候选搜索 | 仅靠均匀点分布会形成圆斑，必须由生境斑块切割 |
| 张量场 / L-system | 未来道路和街区曲率 | 仅设计参考，尚未实现 | 不适合在统一主链前直接引入 |
| OpenStreetMap | 核心窗口道路、建筑、设施和步行网络 | 48/48 已测量 | 不是历史、产权或容量数据 |
| HKPSG / HK 规划署 / CEDD / Highways | 高密城市、山地边坡、道路结构 | 设计/尺度参考 | 不能直接把行政标准复制为游戏阈值 |
| Quaternius | 树、灌木、岩石、建筑原型 | 7 个 MVP 资产已登记 | 展示图透视不能反推真实尺寸 |
| Three.js terrain/ocean examples | 连续地形与水面表现 | HTML 渲染参考 | 不是 Rust 地形/水位真源 |

---

## 8. 当前已确认的核心规则矩阵

| 领域 | 规则 | 级别 |
|---|---|---|
| 自然 | `naturalOnly` 中存在任何人工对象即 fail | 硬约束 |
| 自然 | 场景必须声明宏观自然地理 | 硬约束 |
| 自然 | 1m 高度和 0.25m 高度精度 | 数据合同 |
| 自然 | 自然 heightfield 与最终 heightfield 必须可差分 | 工程合同 |
| 排水 | 保留封闭洼地和平坦终端，不伪造海洋出口 | 审计合同 |
| 水 | 陆生实体不得落入水/深水 | 硬约束 |
| 水 | 河湖宽度必须与渲染使用同一 source | 硬约束 |
| 水 | 水面使用水平基准或明确河道纵坡 | 硬约束 |
| 道路 | 普通道路纵坡默认不超过 8% | 硬约束/可声明的局部合同 |
| 道路 | 普通地形填方默认不超过 6m | 硬约束 |
| 道路 | 超过 6m 填方需要 feature-cut 规划授权 | 硬约束 |
| 道路 | 道路等级决定车道、隔离、路肩和人行道 | 生产规则 |
| 道路 | 平地主干路优先直线，地形处允许受控拟合 | 当前规则/仍需统一决策器 |
| 道路 | 路面与道路 OBB 不得离地、悬空或错宽 | 最终审计 |
| 建筑 | 建筑数量由需求、地块和入口产生，不是固定 target | 生产规则 |
| 建筑 | 建筑 footprint 必须有完整地形支撑 | 硬约束 |
| 建筑 | 建筑禁止压主道路、压其他建筑、进入水体 | 硬约束 |
| 建筑 | 树冠、岩石和细节不得进入建筑 OBB | 硬约束 |
| 建筑 | 生成建筑必须至少有一个经过验证的步行入口 | 硬约束 |
| 入口 | 步行坡不超过 25%，路线不超过 120m | 硬约束 |
| 楼梯 | 高差用显式楼梯/台地连接，不允许静默放宽坡度 | 硬约束 |
| 入口 | 门前净空、路缘落点和侧/后入口需独立记录 | 合同 |
| 时代 | 新旧城使用保留/修改/拆除/替换的父对象链 | 生产规则 |
| 设施 | 学校、医院、教堂等按人口/岗位阈值申请 | 生产规则 |
| 设施 | 不允许通过缩小设施尺寸静默满足请求 | 硬约束 |
| 市场 | 服务使用步行网络时距，不使用直线半径 | 硬约束 |
| 市场 | 市场有容量、竞争、负载和缺口 | 审计合同 |
| 用地 | 城市/郊区/绿地/农田/林地/牧场/保护地分别预算 | 合同 |
| 工程 | 挖填、桥、挡墙、涵洞和入口坡道均需工程账本 | 合同 |
| 保护 | 保护区、水面和岸线带禁止普通削方 | 硬约束 |
| 确定性 | 同 seed 应可重建相同世界 | 审计合同 |
| 分块 | 不可在 chunk 边界出现地形、水岸、道路或材质断崖 | 硬约束 |

---

## 9. 当前三城切面说明

三城只用于验证“同一世界模型能否表达不同地貌与城市关系”，不是三城完整复刻。

### 9.1 r254 汇总

| 切面 | 实体 | 最终建筑 | 主道路检查数 | 树 | 市场 | 质量 |
|---|---:|---:|---:|---:|---:|---|
| 旧金山金门南岸 | 232 | 9 | 7 | 51 | 无市场计划 | warn |
| 上海外滩 | 455 | 18 | 15 | 63 | 1，覆盖 100% | pass |
| 贵阳阿哈湖 | 906 | 27 | 123 | 141 | 3，覆盖 100% | pass |

三城共同结果：

- `scale-audit=pass`；
- 建筑—建筑、建筑—道路、建筑—细节碰撞为 0；
- 悬空、穿地、落水为 0；
- 实体级入口断连为 0；
- 当存在市场计划时，市场辐射守恒并通过。

但必须区分：

- 实体级 `disconnectedEntities=0` 不等于每个可选 front/side/rear anchor 都连通；
- 上海 18 栋、贵阳 27 栋说明它们仍是低建筑数量切口，不是高密核心验收；
- 旧金山仍有 structured-water land-use、服务、生态和岸线带未完成；
- 三个 Manifest 仍以 `generationProfile=legacy` 输出，虽然实际使用了
  route/settlement evolution。

### 9.2 旧金山警告的根因

r254 旧金山 warnings 包括：

- structured-water 场景未执行 profile-driven land-use；
- 自然河道有 21,381/122,368 个水单元不低于 0mm datum；
- 12 户最低住宅未满足；
- 4 个服务请求中仅 2 个物化；
- 1,139/1,852 株生态植物没有合法落点；
- 183,120m² 用地没有 surface realization；
- 城市/乡村面积实现远低于预算；
- 岸线公共通道带未通过独立面积归因。

这些不是“模型不够精致”，而是自然、水体、道路、用地、生态阶段尚未完全同链。

### 9.3 数据输入风险

当前 `guiyang-ahha-wetland-512.manifest.json`：

- `referenceData.caseIds` 指向 `cn-kunming`；
- `viewpoint` 使用旧金山/外滩式的 old-core-to-financial-waterfront；
- `specialFactors` 包含 financial-center、historic-core、coastal-reclamation；

它与“贵阳阿哈湖湿地公园广场走廊”的真实意图不一致，说明三城输入仍有模板复制残留。
在三城成为正式对标样例前，必须先做逐城 Manifest 语义审计。

### 9.4 版本标识风险

`WORLD_GENERATOR_REVISION` 仍为：

```text
worldgen-2026-09-08-r51-stable-v1-transport-identity
```

这与当前 `r254` 代码严重脱节。即使 worldRevision 文件哈希能证明重建一致，也无法仅凭
该字符串判断生成器版本。进入阶段化重构前必须把生成器 revision、程序集内容和 Manifest
程序哈希纳入同一构建标识。

---

## 10. 当前生成逻辑的问题诊断

### P0-1：两套道路/聚落系统并存

`routeEvolution/settlementEvolution` 与 `roadNetwork/frontageProgram/settlementProgram`
都可能写最终实体。这不是简单的“新旧兼容”，因为二者在道路骨架、地块前沿、建筑数量
和工程合同上使用不同的父关系。

需要：

- 一个 Manifest 场景只能有一个主生成器；
- `routeEvolution` 或 `roadNetwork` 必须在 schema 层互斥，或明确一个为另一个的上游；
- 旧模板路径降级为只能补非结构性细节；
- 最终实体必须能追溯到唯一规划父对象。

### P0-2：自然阶段不是所有城市的冻结输入

自然 heightfield 已经先烘焙，但结构化水体、road-first land-use、生态和最终工程仍可能
各自构造与自然层不同的水和地表。应把水陆边界、岸线、汇水和可建性统一成自然阶段输出。

### P0-3：排水是事后图，不是地形发生机制

当前高度场先由噪声、山脊和近似河谷形成，再以 32m 采样生成排水图。真正的地貌链应是
构造/山系 → 汇水 → 河流 → 侵蚀沉积 → 谷地与冲积面，再由 1m 残差补细节。

### P0-4：板块/地块/建筑仍可由多个阶段修改

路线阶段、聚落阶段、规划历史、设施、市场和模板都可能影响建筑。需要冻结规则：

1. 街区边界只能由道路和地形阶段决定；
2. 地块只能切街区；
3. 建筑只能消费地块；
4. 设施可申请重规划，但必须走显式 replace 操作；
5. 生态和街具只能避让已有实体，不能改变结构。

### P0-5：数量/尺寸/推荐值口径混杂

当前有：

- 作者显式 `count`；
- `householdsTarget/jobsTarget/retailFloorArea`；
- `recommendedCount`；
- 实际 delivered count；
- 48 城统计中位数。

正确的用途是：

- `count` 只用于显式 patch 或测试；
- 城市生成优先使用人口/岗位/商业需求；
- `recommendedCount` 只做密度曲线对照；
- 48 城只做验收分布和参数标定，不成为硬配额。

### P0-6：时代分类法不统一

`epochEvolution`、`planningHistory`、`districtDemand` 使用不同的阶段名称。应合并成一个
有 `founding / old-core / expansion / renewal / new-growth / industrial / periphery`
映射的规范枚举，并保留显示名。

### P0-7：设施、市场与建筑先后关系仍需固化

已有规则是“市场先占临街，再放大型设施；空地块不足时重规划”，但必须把依赖写入
统一编排器，而不是依赖当前函数调用顺序。

### P0-8：三城输入语义与生成版本不可完全信任

输入中仍存在跨城市模板复制；报告 revision 也未更新。应先修输入与 provenance，
再讨论视觉和建筑密度。

---

## 11. 推荐的分阶段生成架构

以下阶段应成为唯一主链。未声明的新 Manifest 不允许跳过阶段，也不允许后阶段静默
重新生成前阶段。

### Stage 0：意图、证据与裁剪

输入：

- 世界/场景尺寸、seed；
- 真实地点或虚构场景；
- `urbanSlice`、crop、viewpoint、off-map continuity；
- 48 城 `caseIds`；
- 人口、岗位、时代速度、工程政治约束。

输出：

- 规范 Manifest；
- Source Gate 报告；
- `world-brief.json`；
- 场景裁剪与外部连续性说明。

硬闸门：

- 来源、访问日期、crop 和比例依据齐全；
- 真实地点名与实际窗口一致；
- 城市规模/社区原型/特殊因子可从来源或显式 inference 解释。

### Stage 1：自然骨架

输入：

- 自然地理、气候、海岸、水文、岩性和灾害；
- world seed。

输出：

- 宏观地貌和 1m natural heightfield；
- 分水岭/河道/汇水/湖泊/湿地；
- 山脊、谷地、冲积扇、阶地、三角洲、岸线、坡度和可建性；
- 自然生境场；
- natural baseline hash。

硬闸门：

- 零人工对象；
- 排水图无非法边或伪造出口；
- 高度、岸线、水体跨 Chunk 连续；
- 自然对象尺寸、地表和禁区通过合同。

### Stage 2：人类路径与工程骨架

输入：

- 自然骨架；
- 出入口、港口、山口、渡口、市场/政治中心目标。

输出：

- 多条候选路线与保留方案；
- 道路等级、宽度、纵坡、桥/隧道/高架；
- 初步工程账本；
- 路径中心和边缘外部连接。

硬闸门：

- 主要路线可通行；
- 路线纵坡、填挖和桥跨通过合同；
- 平地主干路优先直线；
- 高差冲突必须显式桥/隧道/高架，不允许无声扭曲。

### Stage 3：聚落种子与网络中心

输入：

- 路线图；
- 可建性、水岸、坡度、灾害和生态保护。

输出：

- 渡口、路口、站场、港口、桥头、市场、宗教/政治历史锚点；
- 主中心、次中心、边缘中心和聚落边界；
- 初始空间关系图。

硬闸门：

- 中心必须有网络证据或叙事证据；
- 不允许仅按整个场景几何中心固定造城；
- 聚落内部不得落水、落陡坡或侵入绝对保护区。

### Stage 4：时代分区

输入：

- 聚落中心和路线网络；
- 48 城机制先验；
- 历史/保护区/再开发约束。

输出：

- 规范时代分区；
- 旧城、更新、新城、工业、外围的预算份额；
- preserve/modify/demolish/replace 策略；
- 每区道路形态和地块粒度。

硬闸门：

- 时代份额总和与允许 spill 合同一致；
- 历史锚点和保护建筑具有不可拆级别；
- 新区和旧区不能只靠颜色或建筑高度区分，必须有街网/地块/功能差异。

### Stage 5：街区与本地路网

输入：

- 主道路/轨道/港口；
- 各时代街区形态；
- 地形坡度和水体。

输出：

- 主路—集散路—本地路—巷道；
- 街区多边形、临街边界、公共空间；
- 路口、环线、转盘和过街连接。

硬闸门：

- 地块不能先于道路完成；
- 主干路交叉数量和间距受控；
- 平地街区优先正交/规则，只有边缘、地标、地形或历史原因才特殊；
- 可见道路与道路碰撞几何来自同一设计面。

### Stage 6：地块与用地分配

输入：

- 街区；
- 时代、保护、功能需求和坡度。

输出：

- 每块地的用途、可建面积、临街面；
- 公共设施、学校、医院、教堂、市场、公园预留；
- 内院、街墙和退界。

硬闸门：

- 地块不跨水、不跨陡坡、不跨分块缝；
- 学校/医院/市场等大型设施先取得合法地块；
- 保护区与生态核心不可被普通住宅回填。

### Stage 7：人口、设施与生鲜市场

输入：

- 人口/岗位基线；
- 地块和道路网络；
- 设施标准、市场层级和容量。

输出：

- 住房、岗位、零售、公共设施需求；
- 市场候选、竞争分配、覆盖和缺口；
- 设施请求与实际交付。

硬闸门：

- 覆盖率、容量、负载和缺口守恒；
- 不靠直线半径满足服务；
- 请求失败必须具名，不能缩小设施尺寸静默补齐。

### Stage 8：建筑体量与拆改

输入：

- 地块、需求、时代、社区原型和中心压力。

输出：

- 建筑 W/D/H、层数、FAR、覆盖率、用途；
- 原建筑与新建建筑的父对象链；
- 建筑推荐数与最终交付数分离。

硬闸门：

- 建筑数量不是固定 target；
- 同一地块不得被多套建筑覆盖；
- 建筑尺寸、旋转和碰撞来自同一实体；
- 历史/保护对象不得被普通重规划误删。

### Stage 9：入口、楼梯与工程闭合

输入：

- 建筑和道路；
- 最终地形；
- 土方预算和桥隧方案。

输出：

- 门前铺装、台阶、坡道、侧/后入口；
- 台地、挡墙、涵洞、桥、高架；
- 自然与最终地形差分账本。

硬闸门：

- 每个最终建筑有验证入口；
- 楼梯从门槛落到道路边缘；
- 悬空、穿地、落水、压路和碰撞全为 0；
- 所有地形修改可归因到工程操作。

### Stage 10：生态与街景细节

输入：

- 自然生境场；
- 最终工程地形；
- 道路、建筑、入口和用地。

输出：

- 森林、灌木、草甸、湿地、岩生的斑块；
- 街道树、树池、路灯、长椅、标识；
- 水岸芦苇、湿边和岸线生态。

硬闸门：

- 植被分布由坡度、湿度、高程、林缘、水体、道路和建设压力决定；
- 不以独立随机圆斑作为主要形态；
- 树冠、灌木和草不进入建筑/道路/入口；
- 不使用与真实资产尺度和地表无关的纯噪声。

### Stage 11：烘焙与运行态验收

输出：

- sidecar、gemap、LOD、hash、质量报告；
- HTML 战略图、街景、探索和生态视图；
- `visual-feedback.json`。

闸门：

- Scale Gate；
- Continuity Gate；
- Bake Gate；
- Adapter Gate；
- Visual Gate；
- Feedback Gate。

---

## 12. 建议的实施顺序

### P0-A：统一入口与 provenance

1. Manifest schema 明确 `cityGenerator = evolution | legacy-road-network`；
2. 同一场景只允许一套主道路/聚落生成器；
3. 每个最终实体必须带 `sourceStage`、`parentPlanId`；
4. 旧模板只能用于显式 legacy 兼容测试；
5. 修复 `WORLD_GENERATOR_REVISION` 与程序哈希；
6. 修正三城模板复制和 caseId 错误。

验收：

- 对同一 Manifest 只出现一套道路和一套聚落主计划；
- 任一最终建筑可追溯到地块、街区和规划阶段；
- 三城 Manifest 来源、crop、caseId、viewpoint 完全一致。

### P0-B：自然先行

1. 将排水图提升到 8m 或更细，并作为地形骨架消费；
2. 山脊、谷地、冲积面、岸线、湿地成为显式自然对象；
3. 水陆边界、河床、水位和岸线使用同一源；
4. 自然阶段输出 habitat field；
5. 生态斑块消费 habitat field，而不是独立圆形采样。

验收：

- 自然图无人工对象；
- 河流全部向下游或合法封闭盆地；
- 树/草/湿地形态由地貌和湿度产生可解释斑块；
- 同一自然基线可复烘。

### P0-C：道路先行

1. 只从自然骨架和聚落目标生成道路；
2. 平地主干路默认直线，复杂地形选择拟合/桥隧；
3. 道路断面先定等级，再定材质；
4. 路口、转盘、互通、高架和隧道作为显式结构；
5. 道路路基完成后再生成临街地块。

验收：

- 没有建筑先于道路；
- 无道路穿地形、固定标高或弯道碎片；
- 主路网断点、碰撞和坡度通过；
- 道路宽度等于实体 OBB 总路权。

### P0-D：聚落与时代

1. 聚落种子只从渡口、路口、港口、桥头、资源和历史锚点产生；
2. 旧城/更新/新区/外围按网络距离和功能角色划分；
3. 每区使用各自街网、地块和建筑原型；
4. 所有拆改/保留/替换显式记录。

验收：

- 新旧城差异可从街网、地块、用途、建筑高度和道路宽度读出；
- 中心强度由网络中心而不是虚构圆形决定；
- 历史核心不是被随机高楼覆盖。

### P0-E：街区、地块、建筑

1. 先切街区；
2. 再切地块；
3. 预留公共设施；
4. 建筑由需求消费地块容量；
5. 建筑宽深、入口、碰撞和土方同时求解。

验收：

- 建筑数量与推荐数分离；
- 中心密度、边缘密度和覆盖率符合分解后的目标；
- 单体 footprint 不再固定；
- 相邻建筑碰撞为 0。

### P0-F：设施与市场

1. 市场先占合法临街地块；
2. 大型设施在市场之后按人口需求申请；
3. 无空地块时执行具名重规划；
4. 报告计划、交付、缺口和父对象。

验收：

- 市场覆盖和负载守恒；
- 学校/医院/教堂等设施失败具名；
- 建筑交付不靠缩小设施尺寸。

### P0-G：生态、街景与视觉

1. 使用 Stage 1 生境场；
2. 按自然斑块/林缘/水岸/城市约束生成植被；
3. 生成街道树、树池、路灯、店面和公共空间；
4. 预览仅消费 Rust 实体和已烘焙高程。

验收：

- 草场不再是独立圆斑；
- 树不是竹竿或统一冠幅；
- 建筑、道路、水体与细节没有明显空置/错位；
- 近景至少能复核尺度、街墙、树冠、水面和入口。

---

## 13. 建议的阶段产物与闸门

| 阶段 | 必备产物 | 失败条件 |
|---|---|---|
| Source | sources、crop、caseId、比例、置信度 | 缺来源或窗口名不实 |
| Natural | natural heightfield、hydrology、landform、habitat、hash | 人工对象、非法排水、跨块断崖 |
| Routes | route plan、road design、bridge/tunnel/earthwork plan | 超坡、非法填方、道路断开 |
| Seeding | anchors、centres、settlement graph | 无证据中心、落水/落保护区 |
| Epoch | epoch plan、district budget、operations | 时代无法追溯、保护对象被误拆 |
| Blocks | road network、blocks、public space | 地块先于道路、街网无连接 |
| Parcels | parcel plan、facility reservations | 地块跨界、设施未预留 |
| Demand | population、jobs、retail、facility needs | 需求不守恒、静默缩小设施 |
| Markets | catchment、radiation、deficit | 覆盖/容量/负载/缺口不守恒 |
| Buildings | building plan、archetypes、parent chain | 固定 count、碰撞、无入口 |
| Engineering | carve ledger、walls、culverts、bridges | 未记账挖填 |
| Ecology | habitat patches、plant placements | 水/建筑/道路禁区、圆斑噪声 |
| Bake | sidecar、gemap、LOD、hash、audits | 任一硬审计失败 |
| Preview | strategic/near/street/explore | 空屏、缺失、明显错位 |

---

## 14. 当前合适的下一步

建议不要立即继续增加“更多建筑外观”或“更多随机规则”。下一步按以下顺序推进：

1. **先冻结统一主生成器和阶段 schema**；
2. **把自然阶段从“先生成高度、再做审计”改为真正的地貌/水系/生境骨架**；
3. **只允许路线阶段从自然骨架选路**；
4. **只允许聚落阶段消费已冻结路线**；
5. **把街区、地块、建筑和价值需求按依赖顺序串起来**；
6. **最后再补立面、街具、树种、水面表现和城市风格资产**。

对于三城：

- 贵阳优先验证“自然湿地与山体 → 路径 → 低密聚落/广场”；
- 上海优先验证“历史街墙 → 水岸 → 更新区 → 高密核心”；
- 旧金山优先验证“海湾/断崖 → 桥头与坡地路网 → Presidio 保护边界与社区”。

只有先修好 `Guiyang manifest` 的 caseId/viewpoint 和三城主生成器选择，三城才适合作为
阶段化重构的回归基线。

---

## 15. 审核时需要确认的事项

1. 是否接受“每个场景只能有一个主城市生成器”作为新硬约束；
2. 是否把 `frontageProgram.count` 限定为 legacy/测试输入，不再用于普通城市生成；
3. 是否把 `recommendedCount` 只作为验收参考，不参与硬配额；
4. 是否以自然排水骨架作为所有道路、湿地、水岸和生态的唯一上游；
5. 是否把街区→地块→建筑作为不可跳过的固定顺序；
6. 是否先以三城作为阶段化重构的回归样例，而不是继续扩大城市数量；
7. 是否允许为特征城区启用独立宏观工程预算，但始终保留工程账本、水体和保护地硬边界。

---

## 16. 证据索引

| 内容 | 文件 |
|---|---|
| 48 城机器先验 | `assets/data/urban-case-priors-48.json` |
| 48 城来源覆盖 | `assets/data/urban-case-evidence.json` |
| 48 个 1km² OSM 窗口 | `assets/data/urban-core-measurements/` |
| 历史统计 | `assets/data/urban-history-patterns.json` |
| 压力统计 | `assets/data/urban-pressure-patterns.json` |
| 道路/水体/建筑参考审计 | `docs/collaboration/rk-x/REFERENCE-AUDIT-20261023.zh-CN.md` |
| 最新三城 | `docs/collaboration/rk-x/THREE-CITY-SLICES-R249-R252-20261023.zh-CN.md` |
| 人口设施 | `docs/collaboration/rk-x/POPULATION-FACILITIES-R246-20261023.zh-CN.md` |
| 重规划 | `docs/collaboration/rk-x/REDEVELOPMENT-R248-20261023.zh-CN.md` |
| 道路等级 | `docs/collaboration/rk-x/ROAD-HIERARCHY-R247-20261023.zh-CN.md` |
| 道路 OBB/密度 | `docs/collaboration/rk-x/ROAD-SECTION-OBB-DENSITY-20261023.md` |
| 市场辐射 | `docs/collaboration/rk-x/MARKET-RADIATION-20261022.zh-CN.md` |
| 自然演化 | `docs/collaboration/rk-x/NATURAL-EVOLUTION-W1-W5-20261021.zh-CN.md` |
| 自然到城市总设计 | `docs/collaboration/rk-x/NATURAL-TO-URBAN-REDESIGN-REVIEW-20261021.zh-CN.md` |
| 需求/进度总表 | `docs/collaboration/requirements-and-progress-master.zh-CN.md` |
