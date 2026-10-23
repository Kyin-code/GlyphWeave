# 核心密度、道路断面与建筑视觉复核（r229）

状态：**硬门通过；中心密度仍低于样本中位，本轮不宣称已达到中环密度。**

## 1. 问题与根因

### 1.1 中心建筑密度

48 个约 1 km² 的核心样本（`assets/data/urban-core-measurements/`）当前给出：

| 指标 | P50 | P75 | Max |
|---|---:|---:|---:|
| 建筑数量 | 410 | 942 | 4,153 |
| footprint 覆盖率 | 32.0% | 37.6% | 56.9% |
| 可驾驶中心线密度 | 20.33 km/km² | 23.43 km/km² | 36.70 km/km² |

这些计数是 OSM building way 的下界，未展开 multipolygon，也不保证包含小棚屋、附属间和后巷小体量。因此它们用于校准形状和覆盖面，不作为必须命中的固定完成数。公开的 OSM 完整度研究也说明城市级 building completeness 普遍偏低：

https://www.tandfonline.com/doi/full/10.1080/17538947.2022.2159550

r201 的中心 150 m 为 42 栋、5,802 m²、8.21%；中心区真正的问题不是单一“户数预算不够”，而是旧城街段被 6 m 宽度下限压成近似固定窄盒，深地块也没有形成连续的内院填充。

### 1.2 道路

Rust 已经发布 `roadClass`、`laneCount`、`laneWidthM`、`medianWidthM`、`sidewalkWidthM`、`shoulderWidthM`、`speedLimitKmh` 与独立碰撞高度。问题在适配器仍保留“宽度猜等级”的 fallback，且虚线、路口和碰撞带各自使用不同几何。

### 1.3 建筑、树木与装饰

- 根 yaw 已统一，但正门仍依赖“第一个 target=road 的 anchor”，应明确使用 `front`。
- 实例化树冠把声明直径误当半径，实际冠幅约为 OBB 的 2.1 倍，会遮挡道路和屋面。
- 店面前花箱虽然已在墙外，但单个高饱和绿色盒仍像道具穿入立面。

## 2. 本轮实现

### 2.1 密度曲线

`settlementEvolution.densityProfile` 新增：

- 180/320/500 m 三段内院采样半径；
- 6/8/10 m 三段填充节奏；
- 旧城 18% 双临街模块、3% 三模块组合；
- 48 城 P50/P75 与未测绘小体量修正系数，仅作校准元数据；
- `settlement-evolution-plan.json` 的 `densityCurve` 记录 100/150/200/300/500 m 建筑数、footprint、覆盖率和单体 footprint 中位数。

旧城住宅最小宽度降至 4 m，商业保持 6 m；当入口或红线限制触发体量回退时，旧城优先收窄沿街面而保留深后翼。所有结果仍须通过入口、水体、坡地、道路 OBB、建筑 OBB 和最终实体审计。

### 2.2 跨分块地形缝

中心区恰好跨 `x=512`。填方坡脚可能只在一侧分块落在建筑平台边缘，入口清理后建筑消失而地形差仍存在，产生 5 m 级断崖。现在内部 512 m 缝在 4 m 带内按两侧同一全局高度函数做确定性渐变：

- 只在内部缝、只对超过 2 m 的阶差生效；
- 道路设计高程、carve 与碰撞真源不变；
- 避免用几十米宽的“禁建带”掏空中心区。

### 2.3 道路

- HTML 删除按道路宽度猜 `roadClass/laneCount/median` 的 fallback；缺设计数据会记录在 `visualChecks.missingRoadDesigns`。
- `?view=street` 默认选择有实际建筑腹地的本地街道，保留旧城近景；HUD 仍显示当前道路等级、车道数、车道宽和限速，快速路/主干路在场景中按同一 Rust 断面渲染。
- 路口从 16 边圆盘改为两条相交道路矩形包围盒的凸包多边形补面。
- 虚线不再经过 miter 多边形生成器，短划线使用独立刚体四边形，避免弯道处折叠为三角碎片。
- 第一人称道路高度从“整段取最大端点”改为沿道路设计纵断面插值。

### 2.4 建筑和植被

- 建筑朝向明确读取 `front` anchor，不能被侧门或后门顺序覆盖。
- 实例化树冠按声明冠幅缩放；树冠不再越出树木 OBB 两倍。
- 店面花箱改为花盆 + 低矮灌木两件式，仍在立面外。
- 店面加入檐口、侧壁柱、宽立面中柱，玻璃与墙体分开表现。

## 3. r229 实测

输出：

```text
bevy/work/rk-x/v3-density-road-final-r229-20261023
```

| 项目 | 结果 |
|---|---:|
| 实体 | 4,109 |
| 最终建筑（含市场） | 325 |
| 市场 | 11 |
| 10/15 分钟鲜食覆盖 | 100% |
| 步行入口断连 | 0 |
| 建筑-建筑/道路/细节碰撞 | 0 / 0 / 0 |
| 最终实体审计 | pass |
| scale-audit | pass |
| 最大跨块高差 | 0.988 m |
| quality-report | pass |

密度曲线：

| 半径 | r201 建筑 | r229 建筑 | r201 覆盖 | r229 覆盖 |
|---:|---:|---:|---:|---:|
| 100 m | 14 | 7 | 6.50% | 5.0% |
| 150 m | 42 | 43 | 8.21% | 10.0% |
| 200 m | 114 | 136 | 12.90% | 16.0% |
| 300 m | 261 | 328 | 12.80% | 15.0% |
| 500 m | 311 | 379 | 5.50% | 6.0% |

150–300 m 的建筑数量、footprint 和覆盖率均有提升，但完全固定圆心附近有中央道路汇流和水面，100 m 半径不能单独代表城区中心。下一轮仍需把“街廓组合体/内院第二排”做成独立可建供给，而不是继续压缩 6 m 盒体。

回归：

- `glyphweave-core --lib`：908 passed / 0 failed / 34 ignored；
- `pnpm test`：22 files / 170 tests passed；
- 原本两条道路测试已按“单一路线宽度真源”和“多跨线合并后 kind 为 overpass”修正。

## 4. 公开参考

- Three.js Terrain: https://threejs.org/examples/webgl_geometry_terrain.html
- Three.js Ocean: https://threejs.org/examples/webgl_shaders_ocean.html
- Parish & Müller, *Procedural Modeling of Cities*: https://cgl.ethz.ch/Downloads/Publications/Papers/2001/p_Par01.pdf
- ArcGIS CityEngine CGA tutorial: https://doc.arcgis.com/en/cityengine/2025.1/tutorials/tutorial-6-basic-shape-grammar.htm
- Hong Kong Planning Department, *Planning and Urban Design for a Liveable High-Density City*: https://www.pland.gov.hk/file/planning_studies/comp_s/hk2030plus/document/Planning%20and%20Urban%20Design%20for%20a%20Liveable%20High-Density%20City_Eng.pdf
- Hong Kong CEDD, *Natural Terrain and Slope Safety*: https://hkss.cedd.gov.hk/hkss/eng/natural_terrain.aspx
- Hong Kong Harbourfront Commission, *Urban Design Study for the New Central Harbourfront*: https://www.hfc.org.hk/filemanager/files/urban-design-study-for-the-new-central-harbourfront/uds-nch-es-(english-version-only)_r.pdf
- Kenney City Kit Roads: https://kenney.nl/assets/city-kit-roads
- Quaternius Ultimate Buildings: https://quaternius.com/packs/ultimatetexturedbuildings.html

## 5. 未完成

1. 固定圆心 100 m 的建筑数量仍低于样本，需要街廓级组合体和内院第二排。
2. 快速路仍缺少匝道、互通、护栏与排水；当前已有 4/6 车道断面、隔离带、车道线和独立碰撞。
3. 路口已改为凸包补面，但尚未由 Rust 发布道路并集多边形。
4. 建筑立面仍以程序化窗带为主，教堂、写字楼、商业裙房和住宅的细部差异需继续扩展。
5. 自然山水继续由高度场、水面和独立生态契约控制；地貌类接口已预留，本里程碑不实现完整演化。
