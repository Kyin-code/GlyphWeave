# r22：农业田块、地头作业带与实际道路通达

日期：2026-09-06。权威工程：`G:\difyllmwiki\GlyphWeave`。
核心版本：`worldgen-2026-09-06-r22-agriculture-surfaces`。
接续 `20260906-yard-surface-realization.zh-CN.md`。

**状态：农业候选已接入实际地表生成和独立重建；不是完整农业规划、灌排模拟或完整现代地图验收。整体 gate 仍 fail。**

> r23 已接入可选毫米精度，详见 `r23-height-precision-and-terrain-domain.zh-CN.md`。本文原 0.25m 用例与局限作为历史阶段证据保留。

## 1. 这次实际改变了什么

r21 已实现庭院，但农业候选还没有执行，旧 farmland/pasture 盒子仍会在 renderer 里产生无土地归属的田地。

本轮：

1. road-first 的现代模板不再发出普通 farmland/pasture 盒子候选。非 road-first 兼容路径不变；已有 authored 农地不会被删除。
2. Rust 消费 agriculture_candidate 的精确矩形，筛选真实田块和农用通道。
3. 田块、地头作业带、通道发布 terrain-conforming 三角网格；HTML 不再自己划田或虚构作物。
4. CLI 从实际最终地形、水体、保留地和实际实体独立重建，核对 field/source/road/path/mesh。
5. 农用通道面积归 transport，不全算成 rural；相交/共用通道只计一次。

## 2. 实现位置

| 路径（相对工程根） | 职责 |
|---|---|
| `bevy/crates/core/src/settlement/agriculture.rs` | 新模块：候选田块、地形/水体/障碍检查、4m 通道、几何所有权、施工网格及拒绝原因 |
| `bevy/crates/core/src/settlement/landcover.rs` | 接入 agriculture realization；独立重建田块 metadata 与网格；面积审计；区分农用通道计数 |
| `bevy/crates/core/src/settlement.rs` | 导出农业模块 |
| `bevy/crates/core/src/worldgen.rs` | 禁止新管线旧农田 proxy；增加 transport 核算，版本更新 |
| `bevy/crates/cli/tests/baked_audit.rs` | 新增实际平缓场景农业链路、两份产物同步篡改测试；修正独立 access 测试 fixture 的派生地表 |
| `adapters/html/app.js` | 显示 Rust 农田/地头/通道 mesh，抑制对应范围的随机草，增加 farm 近景 |
| `scripts/check-preview-camera.mjs` | 校验实际田块与通道 mesh、没有旧 farm 盒子、顶点与 Rust 一致 |

没有新增外部代码、Cargo/npm 依赖或资产。现有 Three/Playwright 仅用于显示和验证。

## 3. 当前田块模型——必须按实际边界理解

模型名 `rainfed-field-direct-access-v1`；这些常数是**当前生成器保守策略**，不是经过农业工程规范或地区法规验证的科学常数。

### 3.1 田块与地头

- 从 agriculture candidate 内切 24×24m 作业块。
- 中心 16×16m 为准备耕作面，面积 256m²；四周 4m 作业带，共 320m²。
- source 边界至少保留 2m，候选网格间隔 28m。
- 对全部 metre 单元采样，检查双线性梯度上界，田块目前最多 12% 坡度。
- 检查水体、实际树石/建筑/围栏等 solid footprint；已有 canal/river/lake/water 也作为水体 footprint 保留，不能只看 natural WaterGeometry。
- 现有 park/green_space/nature_reserve/mountain_forest/farmland/pasture 属于保留用地。尤其不能把 authored 农地覆盖为另一片新田。
- 已有 verified pedestrian pavement 与田块/农用通道不能重叠。使用三角形/单元相交而非中心采样，窄铺装也不会被漏掉。

输出是 `cultivated_soil` 和 `field_headland` 两种实际地表材质。`preparation` 明确为 `rainfed-prepared-soil-no-irrigation-or-yield-claim`。

**不代表已种植、适合具体作物、具有水源保障或土壤肥力，更不证明侵蚀风险、灌排系统和机械转弯半径已通过。**

### 3.2 通道

- 每个作业块四个入口候选，从距边缘 2m 的地头作业带中心出发。
- 通道宽 4m，最多 160m，当前只搜索轴向直线或两段直角路线，确定性排序后选可行路线。
- 通道全宽逐 metre 检查，坡度不超过 8%；没有放宽道路规则。
- 通道只能穿 agriculture candidate 或 future_development 候选，不得跨 nature/unassigned/建筑/庭院等保留区域。
- 全路径 footprint 不穿过本田块中心耕作面或其他已认领田块；末端进入实际 road footprint。
- 与已有 farm track 可以共用，按单元唯一 ownership 记面积；道路范围不再作为新通道面积重复生成。
- 这证明的是当前窄模型下的地形/占用通达，**不是车辆运动学、桥涵或公路设计验收**。当前不允许通过修改场景高度来制造可通道。

没有找到直达路线时，记录 `no_verified_direct_access` 及实际搜索遇到的原因；不得写成“整个地图不存在任何可能路线”。

### 3.3 生成次序与确定性

候选按距实际道路的粗略距离、Z/X 坐标排序。每个候选先检查完整田面，再尝试道路/入口组合；成功后锁定田块单元和通道单元，后续候选不能重复消费。

检查缓存按 metre 单元复用；Landcover source 建立逐行区间索引。所有 ID 和 tie-break 稳定。该实现还有分块/稀疏存储的优化空间，不能由这些结构推导“最大地图性能已达标”。

## 4. 面积与独立审计

SurfaceRealization 升级为 version 2，包含 agriculture.fields / rejectedFields 以及实际 surface patches。

- 每个田块记录 sourceCellId、boundary、cultivated、roadId、accessPath、preparation。
- surface patch 保留 source cell、owner（复用 parcelId 字段表示 field owner）、zoneKind、material、实际顶点与索引。
- 独立 landcover verifier 同时重建 agriculture metadata 和所有网格；只伪造路线但保留网格，或者只删除通道 mesh 而保留 metadata，均不能通过。
- 面积来自实际三角形投影，不只看保存的 areaM2。
- 每个田块必须有 256m² 耕作面、320m² 作业带。道路、建筑、庭院、田块和农用通道不能重复归属。
- 田块与庭院共用 Landcover remainder 预算，实际面积不得超过候选面积。
- 庭院仍单独核算 unresolvedYardM2，不因新增农田而抵消庭院缺口。

`LandUseBudget.realizationAccounting` 变为 `disjoint-owned-surfaces-v3`。`farm_track` 无论源 Zone 为 urban 或 rural，都计入 actualTransportM2；不能同时获得农村面积信用。

## 5. 当前环境的限制被实测暴露出来

### 5.1 0.25m 高度量化导致过度保守

最终高度场单位仍为四分之一米。1m 内只要相邻格高度差 0.25m，离散坡度就是 25%，超过田块 12% 和通道 8%。因此当前逐 metre 检查会拒绝不少看起来平缓的缓坡。

本轮没有用平滑后的虚假坡度通过最终地形审计，也没有将上限扩大到 25%。后续必须改进高精度自然/工程工作高度场或实现有预算的整地；这需要单独改动 `.height.bin` / geometry 合同及兼容策略，不能只改一个测试阈值。

### 5.2 主用例零农田仍然是事实

原有 seed=20260906 的两个起伏地形场景，田面/道路条件无法满足，最终仍是 **0 个合格田块**。旧 farmland/pasture 盒子移除后，1km 的总实体数减少，不能把这个减少描述成删除障碍后“质量提升”。真实农业失败保留，目标未完成。

另新增 `terrainProfile=plains`、seed=7、偏农业比例的 512m 用例，证明新执行链路确实可以生成非空农田。该用例不是替换原有失败用例，两者一起保留。

## 6. 最终实测结果

权威产物均为 `bevy/work/farm-r22-verified-*`，不是此前 `farm-r22-*` 或 `farm-r22-final-*` 中间试验。

| 指标 | 原 512m 场景 | 原 1000m 场景 | 新 plains 512m 用例 |
|---|---:|---:|---:|
| 最终实体 | 16 | 35 | 61 |
| 实际道路 | 5 | 10 | 35 |
| 实现田块 | **0** | **0** | **4** |
| 实际耕作面 | 0 | 0 | 1024m² |
| 实际地头作业带 | 0 | 0 | 1280m² |
| 新增实际农用通道 | 0 | 0 | 48m² |
| 包括既有庭院的 surface 实现面积 | 208m² | 1817m² | 7136m² |
| Landcover unresolved | 258880m² | 988071m² | 224592m² |
| 完整 gate | **fail** | **fail** | **fail** |

plains 用例全部 4 个田块有实际 road ID 与实际通道，浏览器显示 4 个 cultivated-soil mesh 和 4 个 farm-track mesh。整个场景没有 farmland/pasture 盒子实体。

原 512m：57 个候选因田面坡度拒绝，1 个与既有用地冲突，70 个超直达道路预算；原 1km：26 个因田面坡度拒绝，624 个超道路预算。plains：79 个因坡度拒绝，3 个因实物占用，3 个超道路预算，另 4 个实现。

道路/住宅专项审计与独立 rules audit 继续有效。三组 CLI rules 与 bake rules 一致。原两场景 natural raster 与 r21 字节相同；plains 用例采用不同自然输入，不能与它们作“不变”比较。

结果汇总：`bevy/work/farm-r22-verified-evidence.json`。
输入：旧两组沿用 `bevy/work/road-network-r15-{512,1000}.manifest.json`，新用例是 `bevy/work/farm-r22-plains.manifest.json`。

## 7. 测试与 HTML 验证

`bevy/work/farm-r22-verified-tests.log`：Core 287、App 44、CLI 单元 6、实际 bake 集成 10，**共 347 项通过**。

重点覆盖：

- 非空田块、4m 通道几何、作业带与耕作面、所有 mesh 单元互斥、道路不重复计面积、稳定输出。
- 缺道路、超坡田面、超坡通道隔离带、水体、旋转实物、保护地不可穿越。
- authored 农地/牧场、公园和运河不得覆盖；双道路 ID/重叠 source/非法边界拒绝。
- 窄人行铺装三角形不依赖中心采样。
- 实际 plains bake 中两份网格同步伪造 field road/source/path、删 field、改顶点高度或删 farm-track mesh，独立重建失败。

前端：170 项 Vitest、4 项住宅 Three 几何测试通过，typecheck/doc-tree 通过；lint 退出 0 但仍有既有警告。

当前预览端口 8836，独立可执行副本，不锁定 Cargo 输出：

- `http://127.0.0.1:8836/preview/?view=farm`
- `http://127.0.0.1:8836/preview/?view=ownership`

截图位于 `bevy/work/farm-r22-verified-plains/farm.png`、`ownership.png`。浏览器验证日志 `bevy/work/farm-r22-verified-preview.log`：4 个已准备田块 mesh、4 个通道 mesh、0 个旧 farmland/pasture 盒子；最大顶点误差约 0.00001425m（Float32 转换），JS 无异常，相机朝向正常。

人工看图可见棕色耕作面、周边作业带及连接道路的通道。**只是准备后的地表，不画假作物来暗示农业已经完整实现。** 整体材质过亮、周围旧草叶密度和渲染道路边缘问题依然存在，不宣称视觉质量最终通过。

## 8. 下一步必做，而非继续只增加地表标签

1. 高精度工作高度场/有界整地，解决低坡度阈值与 0.25m 输出量化之间的冲突；必须保持最终几何审计，不可只平滑验证数据。
2. 自然地形 provider、区域山脊/河谷、水系和自然生态质量改进。当前农业新增没有改进自然地貌算法，更没有接入真实侵蚀。
3. 为农田加入真实地形条件下的排水出口、田埂/缓冲带、土壤适宜性与经营入口；当前 rainfed 标签不是这几项的替代。
4. 多片区路网、真实可调整地块尺度、作业面轮廓及通道转弯/宽度可配置，不能长期固定 24m 方田。
5. 区域人文建筑形态与公共设施规则、自然保护地实现、最大 6×10km 性能、许可证全量审核和最终 HTML 对照仍属于原始目标。

## 9. 开源与许可证

新增算法为 GlyphWeave 自有工程实现，未复制外部源码、增加库或资产。不是 Mapgen4/symbios-ground 的移植，没有新的外部许可证兼容性结论。阈值为当前策略参数而非引用法规。任何后续外部自然地形实现仍必须先固定版本、核对 LICENSE/NOTICE/依赖树，更新 THIRD_PARTY_NOTICES。
