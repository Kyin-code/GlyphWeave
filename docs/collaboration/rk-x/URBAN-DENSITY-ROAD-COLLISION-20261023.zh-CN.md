# 核心密度、道路碰撞与建筑朝向复核（2026-10-23）

状态：**模块部分通过，整体未签收**。`scale-audit=pass`、
`final-entity-audit=pass`；`quality-report=warn`，仍未把所有入口与视觉问题
闭合。本文不把“能打开预览”当作完成。

> **2026-10-23 r152 更新**：最新合法候选为
> `bevy/work/rk-x/v3-final-r152-20261023/`。世界修订
> `6f5ec5810138173d6cb9d8d5e37c66d46e05c6f125dfa8bf53b319fd70cf2784`，
> 4,371 实体、335 建筑、309 楼梯、1,475 道路段、393 树、建筑 footprint
> 34,811.2m²；跨 Chunk 最大高差 2.834m。建筑—建筑、建筑—道路、
> 建筑—草木、悬空、穿地、落水均为 0；12 个市场覆盖 96.00%。
> `quality-report=warn` 的原因是 23 栋建筑仍没有一条完整可走入口，
> 以及运行态视觉反馈确认路口边缘和整体 footprint 仍偏弱。
> `interiorInfill` 现在以 16m 采样在道路围合街廓内部生成住宅填充，
> 小体量与约四分之一的中大型组合住宅混合，不参与办公、商业和道路
> 拓宽需求；市场候选避开 Chunk 接缝，
> 避免移动的 approach / pad 制造新的边界高差。
> 每个入口面都可发布独立短梯实体，AccessGraph 会按梯子起点和门面
> 距离选择合法楼梯，不再把非正门楼梯预先判死。
> HTML 端将人行道和隔离带在交叉口 box 内裁断，并用道路 OBB 距离而非
> 长条路段中心距选择街景焦点；主路面与碰撞链仍保持连续。

## 1. 复核口径

基准样本仍是 48 城的一平方公里核心窗口，而不是整城 GIS 汇总：

| 指标 | 48 城 min | median | max | 香港中环 Statue Square |
|---|---:|---:|---:|---:|
| 建筑数量 | 48 | 401 | 4,153 | 410 |
| 建筑覆盖 | 2.2% | 31.65% | 56.9% | 30.2% |
| 可驾驶中心线密度 | 12.10 | 20.31 | 36.70 | 32.719 km/km² |

OSM 只提供该窗口的可观测 building way 和小体量主体；它不是人口普查，
也不包含所有内部隔间、棚屋和未测绘附属物。生成器不把 410 写成固定完成数，
而是用需求、合法街廓、OBB、入口和工程合同求结果。

## 2. 本轮已落地

### 2.1 建筑密度与用途混合

- 同一街侧按连续里程生成地块；地块仍由需求决定是否建设。
- 建筑先使用原型最大体量，再依次尝试 85%/70%/55% 的合法退让体量；
  所有尺寸不得低于 6m，仍须通过道路、建筑、入口、接地与最终碰撞检查。
- 中心、更新带、新区分别使用不同的家庭、岗位、商业和服务容量粒度。
- 增加确定性地块用途抽样：写字楼、沿街商业、混合住宅不再是单一用途
  全覆盖。
- 香港中环 1km 控制输入提高到 16,000 户、80,000 岗位、400,000m²
  商业、240 服务预算；这些是核心切面的情景需求，不是 410 栋目标。

### 2.2 道路断面与碰撞

- 道路设计不再由浏览器现有材质决定，而由道路等级和宽度发布：
  32m 快速路、22m 主干路、16m 集散路、10/8m 本地路及小于 8m 的巷道。
- `RoadDesign` 固化车道数、车道宽、中央隔离带、人行道、限速与碰撞高度。
- 路线车道线在真实交点附近截断；中央双黄线、车道虚线、边缘线和隔离带
  分别发布。
- 路面横断面使用同一个设计标高并在中心加浅拱，不再让左右边各自跟随
  地形形成扭曲三角面。
- 路面 Mesh 与第一人称碰撞链分离；碰撞体按同一路线独立生成，并携带
  道路等级、车道数、宽度和面高。

### 2.3 旋转、墙体与草

- Rust OBB、入口逆变换、HTML footprint 测试统一为 Three 的正 Y 旋转：
  `worldX = localX*cos + localZ*sin`，
  `worldZ = -localX*sin + localZ*cos`。
- HTML 建筑 root 始终使用 Rust 发布的 `rotationYDeg`；若立面需要转向道路，
  只旋转 root 内的子节点，不再把整个碰撞 OBB 转 180°。
- 删除住宅/混合住宅包络内的绿色草坪和庭院块；草木只能作为独立 Rust
  实体存在于建筑 OBB 外。
- 规划住宅增加贴合最低地面的 footprint 内基础，不再以超出 OBB 的浏览器
  装饰地基掩盖高差。
- 住宅侧立面和背立面补充分层窗带与后门，街角镜头不再只看到整面涂色
  防火墙。

### 2.4 道路和地块的最终复核（r131）

- HTML 道路 ribbon 在 Rust 已发布 `designHeightM` 时只使用该设计高程，
  不再把左右两侧地形最大值混入横断面；截图中的弯道三角形破口已消失。
- 建筑位置在同一合法地块内搜索五个纵深位置；每次移动后重新检查水体
  footprint、坡度、填挖上限、建筑/道路 OBB 和实际门前通路接缝。
- 市场初始部署增加 2% 小型安全余量，并在最终分配时把浮点残余准确记入
  最后可服务的市场，保证计划、辐射和缺口三份报告守恒。

### 2.5 街廓内部住宅填充（r143）

- `settlementEvolution.interiorInfill` 为显式开关；旧世界和既有测试在未声明时
  保持原行为。
- 以 16m 确定性采样在已建道路围合的 300m 核心范围内寻找空隙；约四分之三
  为 6–10m 宽、8–16m 深的窄体量，约四分之一为 10–20m 宽、16–28m 深的
  组合住宅，让同一策略同时表达缝隙填充和完整街廓内院。
- 内部填充只消费住宅需求，不生成办公、商铺或服务容量，也不触发额外道路
  拓宽；其用途是补足街廓内部低层住宅，而不是二次制造中心岗位。
- 每个候选仍检查水体 4m 岸线、坡度、最大削方、道路 OBB、实际门前通路
  接缝、建筑互撞、悬空和穿地。
- 市场候选主动避开 Chunk 接缝 48m；这是 r132/r135 实际边界失败后得到的
  约束，不能为了覆盖率把市场平台放在接缝上。

### 2.6 多门面入口楼梯（r144）

- 同一建筑除已有的正门长梯外，会为侧门重新采样 2m 落地平台；高差在
  0.30–2.40m 时发布独立 `stairs` 实体，避免把侧门直接判成不可达。
- AccessGraph 从实体索引中选择起点与当前门面距离不超过 2m 的合法楼梯；
  楼梯仍须通过障碍、岸线、地形高度和完整到路网路径检查。
- 结果：断开建筑从 48 降到 23，连通建筑从 325 升到 350；没有放宽
  0.30m 台阶门限，也没有把未物化楼梯视为已工程。

### 2.7 交叉口附属带裁断（r152）

- 人行道和中央隔离带在真实路口 box 内分段，不再以一条连续 ribbon
  穿过交叉口；主车行道及独立碰撞链不变。
- 街景焦点用“点到道路 OBB/路段矩形”的距离选择，长路段即使中心很远，
  只要实际穿过中心城区仍能被选中。
- 运行态仍能看到少量交叉口边缘楔形；它们已不属于车道线或人行道连续
  路径，后续需要单独的 junction mesh 模板，而不是继续在 ribbon 层修补。

## 3. r152 实测

输出：`bevy/work/rk-x/v3-final-r152-20261023/`

- 世界修订：`6f5ec5810138173d6cb9d8d5e37c66d46e05c6f125dfa8bf53b319fd70cf2784`
- 实体：4,371
- 常规/市场建筑：335
- 入口楼梯：309
- 建筑 footprint：34,811.2m²（整幅 1km² 的 3.48%）
- 用途：190 residential block、91 storefront、42 office tower、12 market
- 道路段：1,475
- 树 393
- 中心 `(500,450)` 密度：

| 半径 | 建筑 | footprint |
|---:|---:|---:|
| 100m | 25 | 2,302.0m² |
| 150m | 62 | 5,873.5m² |
| 200m | 153 | 14,054.6m² |
| 300m | 301 | 31,244.7m² |
| 500m | 335 | 34,811.2m² |

r140 的 340 栋、3.47% footprint，r131 的 278 栋、3.01% footprint
和 r104 的 195 栋、2.06% footprint 均保留为历史中间结果。

## 4. 硬门结果

- `scale-audit=pass`；跨 Chunk 最大高差 2.834m，低于 4m 门限。
- `final-entity-audit=pass`；建筑—建筑 0、建筑—道路 0、
  建筑—草木 0、悬空 0、穿地 0、落水 0、无支撑 0。
- 市场 12 个；10 分钟覆盖率和总覆盖率为 96.00%，计划和辐射报告守恒。
- `quality-report=warn`：350 栋有至少一条入口连通，23 栋所有入口均断开。
- 运行态 `visual-feedback.json` 为 `warn`，已记录道路连续化通过、侧立面
  已补充，同时保留 `core-footprint-too-low`、`internal-block-underused`
  和 `intersection-edge-geometry` 三项问题。

## 5. 未签收项

1. **密度和单体体量仍不足。** 150m 内 62 栋已接近 p75 城市均匀折算值，
   但平均 footprint 约 102m²；街廓内部已开始填充，仍远小于 250–700m²
   的组合建筑。不能把 340 栋解释为“已经达到中环”。
2. **入口仍具名失败。** 23 栋建筑的所有入口均断开，必须逐项生成楼梯、坡道、平台或改选
   地块；不得通过放宽入口距离、坡度或 OBB 合同消除。
3. **交叉口边缘仍复杂。** 路面本身已连续，但路口周边仍有重叠人行道、
   台地和碎片边缘；需要单独的路口模板及统一三角剖分。
4. **立面与街具仍偏简。** 侧立面窗带已补齐，但大面墙、连续店面、雨棚、
   门厅、招牌、路灯和尺度参照仍未达到“现代高密街区”。

## 6. 设计结论

后续密度不能靠缩小需求粒度或复制同一种塔楼。应把“一个合法街廓”继续拆成
多个有独立入口的建筑体，并允许裙房、窄塔、院落和附属小体量共享一个经审计
的台地；道路、入口、OBB 与工程账本仍保持一一对应。

自然、水体与山体继续作为前置约束：道路先沿谷地，聚落再依附道路，台地和
填挖在允许范围内反向修改地形。城市中心可有中央规划与特征工程授权，但每项
开山、填海、填方和挡墙都要进入工程账本。

## 7. 公开参考

- NACTO, *Urban Street Design Guide: Lane Width*：
  https://nacto.org/publication/urban-street-design-guide/street-design-elements/lane-width/
- NACTO, *Neighborhood Street*：
  https://nacto.org/publication/urban-street-design-guide/streets/neighborhood-street/
- Hong Kong Highways Department, *Structures Design Manual*：
  https://www.hyd.gov.hk/en/technical_references/technical_document/structures_design_manual_2013/index.html
- Hong Kong CEDD, *Natural Terrain and Slope Safety*：
  https://hkss.cedd.gov.hk/hkss/eng/natural_terrain.aspx
- HFC, *Urban Design Study for the New Central Harbourfront*：
  https://www.hfc.org.hk/filemanager/files/urban-design-study-for-the-new-central-harbourfront/uds-nch-es-(english-version-only)_r.pdf
- Hong Kong Planning Department, *Planning and Urban Design for a Liveable
  High-Density City*：
  https://www.pland.gov.hk/file/planning_studies/comp_s/hk2030plus/document/Planning%20and%20Urban%20Design%20for%20a%20Liveable%20High-Density%20City_Eng.pdf
- 仓库既有 Quaternius Stylized Nature / Ultimate Buildings：
  `assets/third_party/quaternius/README.md`、
  `assets/third_party/quaternius/buildings/NOTICE.txt`

这些来源用于尺度、断面、坡地安全和城市关系，不复制第三方专有模型。

## 8. 验证命令

```powershell
bevy\target\debug\glyphweave.exe scale-audit bevy\work\rk-x\v3-final-r152-20261023
bevy\target\debug\glyphweave.exe quality-report bevy\work\rk-x\v3-final-r152-20261023
cargo test --manifest-path bevy\Cargo.toml -p glyphweave-core --lib settlement::building_archetypes --no-fail-fast
cargo test --manifest-path bevy\Cargo.toml -p glyphweave-core --lib worldgen::access --no-fail-fast
cargo test --manifest-path bevy\Cargo.toml -p glyphweave-core --lib worldgen::market_catchment --no-fail-fast
cargo test --manifest-path bevy\Cargo.toml -p glyphweave-core --lib worldgen::settlement_evolution --no-fail-fast
node --check adapters\html\app.js
node --check adapters\html\render\terrain.js
node --check adapters\html\render\buildings.js
```
