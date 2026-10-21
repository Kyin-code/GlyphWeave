# 香港中环机制切面：陡坡旧核、更新台地与填海金融水岸

日期：2026-10-21  
控制 Manifest：`examples/hong-kong-central-natural-evolution-1km.manifest.json`  
最终控制世界：`bevy/work/rk-x/hk-central-natural-evolution-1km-r2a/`

## 1. 定位与边界

这是 **1km × 1km 的合成机制切面**，不是香港全域、不是中环地理配准重建，
也不是 48 城统计样本：

- 表达关系：陡坡山体 → 旧市场核心 → 更新台地 → 新生城区 → 填海金融水岸；
- `style.urbanSlice.mode = feature-cut`；
- `contextRadiusM = 3000`，只声明离图道路、轨道、城区和海岸连续性；
- 来源入口记录在 `style.referenceData`，包括香港规划署、古物古迹办事处、
  房屋委员会和项目内 48 城证据库；
- 不作为“香港完整复刻”或“所有中心均在此图中”的宣传材料。

## 2. 本轮确认口径的落实

| 已确认口径 | 本切面的落实 |
|---|---|
| 自然先行 | 先烘焙 natural heightfield、河海水面与排水完整性审计，再生成道路和聚落 |
| 建筑数量是结果 | `householdsTarget=900`、`jobsTarget=2600` 只表达需求；地块容量、原型和入口决定最终数量 |
| 推荐数仅审计 | 建筑规划保留 `recommendedCount`，但物化数量不追齐推荐数 |
| 建筑尺寸随地形 | 楼体基础取地形支撑与周边已物化道路设计面中的较高者；办公塔、住宅和店面使用各自原型 |
| 新旧城同一骨架 | `districtDemand` 按 old-core / renewal / new-growth / periphery 分配需求，未使用预算才全局回填 |
| 中心来自网络 | 土地使用压力、道路中心性和规划史均由实际 route plan 推导，不使用地图圆心 |
| 分级市场 | 街市、邻里超市和区域市场按容量、品质、步行时距、竞争与网络可覆盖需求 |
| 工程服务规划 | 普通建筑只允许局部有限切填；门前 landing、道路平台与地块平台各有明确来源 |

## 3. 本轮修复

### 3.1 市场不得挤掉同级建筑

市场候选现在计算离路 12m 后的真实旋转 footprint，并同时检查：

- 与其他市场的 2m 净距；
- 与所有已规划住宅/店面的 2m 净距；
- 两侧街面均可选，靠建筑的一侧被占用时改选对侧。

这修复了“市场先落地、随后把建筑门口堵死”的优先级倒置。

### 3.2 办公与住宅必须获得地形支持

`office_tower`、`shopping_mall`、`night_market`、`residential_block`、
`community_plaza` 和 `green_space` 现进入建筑 carve 白名单。此前它们出现在
实体和入口审计中，却没有自己的 support pad，实际站在邻居道路或地形填充上。

### 3.3 入口锚点改为立面中点

入口不再取道路射线与矩形 footprint 的对角交点。对角点可能落入另一条交叉
道路的 owned band，使门口继承另一条道路的高度。现在锚点取朝向分配道路的
立面中点，仍由 AccessGraph 检查 facade → landing → street 的真实 25% 步行坡。

### 3.4 2m 门前 landing

每个建筑门口有独立的 2m × 2m landing carve：

- 上限 `1000mm`，只作用于门前小面；
- 不改变 0.30m 入口台阶合同；
- 不能进入道路 owned band；
- 高于该上限的坡差仍具名失败，后续需要楼梯/挡墙/switchback。

### 3.5 路口更新建筑标高

建筑基础同时参考地形支撑和**已物化**道路实体面，而不是只看路线设计毫米值。
两条道路交叉时，路线设计 32.985m 可能物化为 33.5m 路床；建筑现在随路口更新，
避免“楼在 32.985m，路口在 33.5m”的假台阶。

### 3.6 门区自然细节清理与预览

- 最终计划实体全部出现后，再清理门口 3m 内的树、灌木、石块等障碍；
- 增加 `?view=waterfront`，从内陆看向海港；
- `?view=skyline` 按视口宽高比扩大取景，保证整个 1km 世界和东侧水面都在画面内；
- 战略图、街景和全景不再出现裁掉的三角 chunk 边界。

## 4. r2a 最终控制数据

| 项 | 结果 |
|---|---:|
| 场景 / 分块 | 1 / 4 |
| 实体 | 2,973 |
| 需求住宅 / 岗位 / 商业 / 服务 | 900 / 2,600 / 20,000m² / 36 |
| 需求闭合 | 全部闭合 |
| 规划建筑 | 35 |
| 最终普通建筑 | 28 |
| 最终市场 | 6 |
| 最终办公塔 / 店面 / 住宅街区 | 6 / 22 / 1 |
| 城镇关系 | 2 districts / 35 buildings / 35 parcels |
| AccessGraph | 35 connected / 0 disconnected |
| 市场计划覆盖（最终 28 栋建筑口径） | ≈100% |
| `scale-audit` | pass |
| `quality-report` | pass |
| `visual-feedback` | pass |
| 最大跨 chunk 高度差 | 0.847m |
| 最大边界 quarter 摘要差 | 3.388（仅粗摘要，不是米制验收口径） |

最终连通数 35 = 28 栋普通建筑 + 6 个市场 + 1 个规划中心。

## 5. 被严格拒绝的规划

35 个计划建筑中 28 个物化，7 个被入口合同拒绝：

| 原因 | 数量 | 处理 |
|---|---:|---|
| `entrance_step_requires_engineering` | 4 | 保留为门槛/楼梯/挡墙待办，未放宽 0.3m |
| `entrance_clearance_blocked` | 3 | 保留具名拒绝，未用隐藏实体或删入口制造连通 |

被拒建筑没有进入最终 AccessGraph。它们的 ID、位置、预期需求、入口锚点和
拒绝原因保存在 `settlement-materialization-report.json`。

## 6. 视觉证据

运行态检查使用 r2a：

- **战略图**：可辨认西侧山地绿区、旧核与更新带的多组街区、连续弯曲道路、
  东侧长条海港；
- **`?view=skyline`**：完整 1km 地形面在画面中，东侧水体为可见青蓝色，
  不再有被相机裁切的三角空场；
- **`?view=street`**：能看到塔楼、店面和住宅街区组成的连续城市面，不再只有
  三栋孤楼；
- **`?view=waterfront`**：从内陆侧观察城市与海港关系，水体不再完全落在镜头背后。

视觉反馈已写回 r1w 的 `visual-feedback.json`，并用 `quality-report` 复核为 pass。

## 7. 仍开放的问题

1. **斜向道路 carve**：45°/135° 普通道路仍按 X 向长条近似，见
   `KNOWN-ISSUE-ROTATED-ROAD-CARVE-20261021.zh-CN.md`。
2. **真实楼梯**：7 个严格入口拒绝中，坡差大于 landing cap 的情况应生成
   landing + straight/switchback stairs + retaining/drainage，而不是继续加深切方。
3. **市场最终需求复核**：已在本轮关闭。入口拒绝后以最终 28 栋建筑重算市场
   需求与负载，原计划保留为 `market-catchment-plan-pre-access.json`；见
   `POST-ACCESS-MARKET-RECONCILIATION-20261021.zh-CN.md`。
4. **街区拆合**：旧城连续街墙、更新带父/子拆合、新区站城/TOD 仍是下一阶段。
5. **统一宏观工程账本**：开山、挡墙、台地、填海、造岛、堤坝、涵洞和排水仍
   需要统一入账，不能只依赖局部 carve。

