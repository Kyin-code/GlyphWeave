# 城区密度、道路断面与建筑立面复核（2026-10-22）

本轮针对用户复核提出的四类问题：

1. 城心建筑密度不足；
2. 道路没有车道与高速断面，弯道路面破碎；
3. 建筑旋转、碰撞体和墙窗不一致；
4. 需要重新核对自然山体、水体、建筑的公开参考。

## 1. 48 城样本基线

复核沿用仓库已登记的 48 个 `urban-core-measurements` 核心窗口，不重新把
GIS 面数据当作全部建筑真值：

| 指标 | min | p25 | median | p75 | max |
|---|---:|---:|---:|---:|---:|
| 建筑数量 | 48 | 262 | 401 | 937 | 4153 |
| 建筑覆盖 | 2.2% | - | 31.65% | - | 56.9% |
| 可驾驶中心线密度 | 12.10 | - | 20.31 km/km² | - | 36.70 |

这些统计只作核心区机制和量级参照；本轮不声称生成器已经复刻 48 城全部
小体量附属建筑。

## 2. 本轮修正

### 2.1 建筑密度与中心性

- 取消“全城旧城只允许一条街墙”的总量限制，改为每条真实街侧独立形成
  连续街墙；孤立旧城点仍拒绝。
- 城市街区网格新增 13 条局部街段，区分 32m 快速路、24m 主干路、
  16/20m 集散路和 10/12m 本地路。
- 支路临街宽度不再被大都市 24m 统一目标覆盖，而是使用各自真实断面；
  每栋建筑只额外保留 3m 道路净空、2m 退界和自身深度。
- 中心窄面宽地块与外围较宽地块分别取样；建筑数量仍由家庭、岗位、商业、
  服务需求及合法街区容量产生。
- 有界开山授权从“坡度一票否决”改为“坡度可通过 `maxPadCutM` 内台地
  工程解决”；无工程授权的场景保持原坡度拒绝。

### 2.2 道路

- `GroundingSpec.roadDesign` 发布 `laneCount`、`laneWidthM`、
  `medianWidthM`、`sidewalkWidthM`、`speedLimitKmh` 和碰撞高度。
- 8m 街道修正为双向两车道；16m/22m/30m 分别进入集散路、主干路和
  快速路断面。
- 斜向道路、地块、市场和入口统一使用 Three/OBB 的正 Y 旋转约定。
  旧 `rotation=0 + north-south 名称` 兼容规则仅作用于旧 `prop.road`，
  不再误转新建筑和路线实体。
- 后续规划历史拓宽道路前先做建筑 OBB 检查；冲突时保留原断面，并将
  被跳过原因写入 `planning-materialization-report.json`。
- HTML 路面改为“分段矩形 + 圆角接缝”，避免 miter 尖刺和自交孔洞。
  道路碰撞体保持独立，但不再以可见黑色盒体穿过交叉口。
- 路面和车道线按整幅横向边缘采样地形，避免坡面草丛从道路三角面中穿出；
  碰撞体集合在每次近景重建开始时清空，不再覆盖已收集的建筑碰撞体。

### 2.3 建筑朝向与立面

- 建筑保留 Rust 发布的地块 yaw；frontage anchor 只决定本地 `+Z/-Z`
  哪一面是街面，不再将斜街吸附成正南北/正东西。
- 删除住宅原型内部的三棵、四棵和单棵程序化树。植被只能作为独立 Rust
  实体存在，并受最终 OBB 清障和硬审计约束。
- 最终审计把 `grass_clump` 与 tree/bush/rock 一并纳入建筑内部碰撞检查。
- 店铺四面窗带沿用 CGA 立面；塔楼在四个方向都有分段窗带。

### 2.4 生鲜市场

- 市场尺寸从 24×16m 调整为 16×10m 的街坊级生鲜厅，可进入窄街墙。
- 市场入口本地坐标改用与 OBB 相同的逆变换，修复斜街锚点越界。
- 每个市场发布 4m 静态入口步道；高差 0.3–8m 时同时发布楼梯实体。
- 市场覆盖在最终入口验收之后重新计算，只统计实际物化市场。

## 3. r65 实测

输出：

`bevy/work/rk-x/v3-density-road-r65-20261022`

世界修订：

`2ff105fd5f2f02e0b49953b684dd513c46035c89667d0f1d1d1dd2128b8765d3`

- 实体 3,293；
- 最终建筑 140（普通规划建筑 133、市场 6、规划中心 1）；
- 建筑 footprint 17,767m²；
- 道路实体 686；
- 树 434；
- 最终建筑—建筑、建筑—道路、建筑—植被碰撞均为 0；
- 悬空、穿地、落水、无支撑 footprint 均为 0；
- AccessGraph：134 栋计划普通建筑中 133 栋有验证入口，0 断点；
- 6/6 市场物化，6/6 入口连通；
- 市场 10 分钟覆盖 93.46%，服务需求 16,800 / 17,976；
- `scale-audit: pass`；
- `quality-report: pass`；
- 跨 Chunk 最大高差 3.974m，低于 4m 门限。

以中心 `(500,450)` 为原点：

| 半径 | 建筑数 | footprint |
|---:|---:|---:|
| 100m | 20 | 2,488m² |
| 150m | 46 | 5,568m² |
| 200m | 75 | 9,371m² |
| 300m | 127 | 15,753m² |
| 500m | 140 | 17,767m² |

中心 150m 半径内建筑数为 46。与 r28 的 53 栋全场景建筑相比，本轮不是
简单放大单栋体量，而是把普通建筑增加到 2.6 倍，并形成明显向中心聚集的
密度曲线。

## 4. 公开参考

以下资料只用于尺度、断面与形态机制，不复制第三方专有资产：

- NACTO, *Urban Street Design Guide: Lane Width*  
  https://nacto.org/publication/urban-street-design-guide/street-design-elements/lane-width/
- NACTO, *Neighborhood Street*  
  https://nacto.org/publication/urban-street-design-guide/streets/neighborhood-street/
- Hong Kong Highways Department, *Structures Design Manual*  
  https://www.hyd.gov.hk/en/technical_references/technical_document/structures_design_manual_2013/index.html
- Hong Kong CEDD, *Natural Terrain and Slope Safety*  
  https://hkss.cedd.gov.hk/hkss/eng/natural_terrain.aspx
- HFC, *Urban Design Study for the New Central Harbourfront*  
  https://www.hfc.org.hk/filemanager/files/urban-design-study-for-the-new-central-harbourfront/uds-nch-es-(english-version-only)_r.pdf
- Hong Kong Planning Department, *Planning and Urban Design for a Liveable
  High-Density City*  
  https://www.pland.gov.hk/file/planning_studies/comp_s/hk2030plus/document/Planning%20and%20Urban%20Design%20for%20a%20Liveable%20High-Density%20City_Eng.pdf
- 仓库既有 Quaternius Stylized Nature / Ultimate Buildings 参考及许可记录：  
  `assets/third_party/quaternius/README.md`、`assets/third_party/quaternius/buildings/NOTICE.txt`

## 5. 仍保留的边界

- 本轮 140 个最终建筑仍低于 48 城“全部可观测建筑”中位 401；当前实体
  更接近主要建筑体量与公共设施，不把每间小屋、车库和棚屋拆成地图实体。
- 路口仍是通用铺装加车道线，尚未生成真实导流岛、转向车道和立交桥型。
- 山体和水体的细部仍以地形材质与预留接口为主，专项山水生成不属于本轮
  A/B/C 里程碑收口。

> **2026-10-23 后续复核**：r65 的 `quality-report=pass` 不能延续解释为当前
> 代码质量。统一旋转约定后，r104 暴露了 28 个严格入口失败；最新事实、密度
> 曲线、道路断面计数和视觉结论以
> [`URBAN-DENSITY-ROAD-COLLISION-20261023.zh-CN.md`](URBAN-DENSITY-ROAD-COLLISION-20261023.zh-CN.md)
> 为准。

## 6. 失败样本与修正轨迹

- r49 在加密街区后出现 15 处台地穿入。原因是相邻道路肩带把终态地形抬高，
  而建筑仍使用仅覆盖自身 footprint 的标高；修正为同时继承 6m 道路施工肩带
  的完成面标高。
- r52 的复制检查曾出现 64 条建筑压路。根因不是地块放置，而是后续
  `planning-history` 无条件拓宽道路；现在拓宽本身执行建筑 OBB 阻挡，并将
  被跳过项写入 `skippedRoadWidening`。
- r56 的市场预检显示 93.46%，但入口验收后只有 4/6 个市场可用；原因是
  后处理仍按计划市场而非最终市场计算。现在市场入口先经过真实 AccessGraph，
  再按最终集合重算覆盖，并生成 4m 入口步道及必要楼梯。
- 曾在 r53 尝试直接删除零产出支路，跨 Chunk 高差随即达到 6.702m；该输入
  被 `scale-audit` 拒绝并撤回，保留道路拓扑与 4m 边界门限。
