# 核心密度、道路碰撞与建筑朝向复核（2026-10-23）

状态：**模块部分通过，整体未签收**。`scale-audit=pass`、
`final-entity-audit=pass`；`quality-report=warn`，仍未把所有入口与视觉问题
闭合。本文不把“能打开预览”当作完成。

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

## 3. r104 实测

输出：`bevy/work/rk-x/v3-final-r104-20261023/`

- 世界修订：`fe0cd0e0e7f310aed340eb825ab1c4ac89b804257c4366607f52fe5d535627e1`
- 实体：3,888
- 普通/市场/中心建筑：195
- 建筑 footprint：20,641.9m²（整幅 1km² 的 2.06%）
- 用途：121 residential block、41 storefront、24 office tower、
  8 market、1 industrial
- 道路段：1,331；快速路 32、主干路 26、集散路 219、本地路 885、巷道 169
- 树 406、灌木 271、草丛 1,220、岩石 367
- 中心 `(500,450)` 密度：

| 半径 | 建筑 | footprint |
|---:|---:|---:|
| 100m | 21 | 2,586.8m² |
| 150m | 51 | 5,484.0m² |
| 200m | 93 | 9,331.2m² |
| 300m | 172 | 18,012.7m² |
| 500m | 194 | 20,641.9m² |

## 4. 硬门结果

- `scale-audit=pass`；跨 Chunk 最大高差 3.916m，低于 4m 门限。
- `final-entity-audit=pass`；建筑—建筑 0、建筑—道路 0、
  建筑—草木 0、悬空 0、穿地 0、落水 0、无支撑 0。
- 市场 8 个；10 分钟覆盖率 92.64%，不是 pass。
- `quality-report=warn`：167 个入口连通、28 个断开。
- 断开原因：12 `entrance_step_requires_engineering`、
  10 `stair_connection_invalid`、3 `entrance_clearance_blocked`、
  3 `no_walkable_route_within_budget`。

## 5. 未签收项

1. **密度仍不足。** 当前中心 150m 为 51 栋，只有香港样本 410 栋的约八分之一；
   2.06% footprint 也远低于样本 30.2%。不能把 195 栋解释为“已经达到中环”。
2. **入口仍具名失败。** 28 个断开入口必须逐项生成楼梯、坡道、平台或改选
   地块；不得通过放宽入口距离、坡度或 OBB 合同消除。
3. **街景仍选中交叉口附近。** 当前默认核心街段可加载，但 4 车道断面在
   路口附近的标线仍呈短碎段；需要把道路标线改成连续路口模板，而不是由
   路口圆形裁剪任意打断。
4. **立面与街具仍偏简。** 大面墙、暗玻璃与稀疏街具尚未达到“现代高密街区”
   所需的连续店面、雨棚、门厅、招牌和尺度参照。

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
bevy\target\debug\glyphweave.exe scale-audit bevy\work\rk-x\v3-final-r104-20261023
bevy\target\debug\glyphweave.exe quality-report bevy\work\rk-x\v3-final-r104-20261023
cargo test --manifest-path bevy\Cargo.toml -p glyphweave-core --lib settlement::building_archetypes --no-fail-fast
cargo test --manifest-path bevy\Cargo.toml -p glyphweave-core --lib worldgen::access --no-fail-fast
cargo test --manifest-path bevy\Cargo.toml -p glyphweave-core --lib worldgen::market_catchment --no-fail-fast
cargo test --manifest-path bevy\Cargo.toml -p glyphweave-core --lib worldgen::settlement_evolution --no-fail-fast
node --check adapters\html\app.js
node --check adapters\html\render\terrain.js
node --check adapters\html\render\buildings.js
```
