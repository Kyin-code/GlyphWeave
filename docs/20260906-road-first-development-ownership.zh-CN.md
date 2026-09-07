# r18：道路先行的地块、建筑计划与互斥土地归属

日期：2026-09-06。权威工程：`G:\difyllmwiki\GlyphWeave`。

**状态：阶段性重构已接入真实 bake/CLI/HTML；整份自然与人文路线图仍未完成。**
本轮不修改 `F:\modenmapbox`，不引入新外部源码、依赖或资产。

> r19 接续见 `20260906-landcover-candidate-plan.zh-CN.md`：增加候选地表计划与独立核验，尚无实际地表实现。以下 r18 验收数据仍保留为本阶段记录。

## 1. 本轮为什么继续重构

r16 已有自然 raster 驱动的连通道路和临路房屋，但同时发布另一套按径向用地区域切出的旧 ParcelPlan。这两套计划互不约束：9 栋房屋有 road ID，却没有真正的 parcel 父关系；旧地块全部未实现。旧面积报表又把道路重叠、绿地/农地实体的投影面积混作土地完成量。

本轮解决的不是让报告更好看，而是建立单一的规划来源、完整土地分区以及建设状态的边界：

```text
Natural raster → RoadNetworkPlan
                       ↓
LandUseZones → DevelopmentPlan（实体生成之前）
                 ├─ FrontageParcel（实际临路地块）
                 ├─ BuildingPlan（建筑 footprint、变换、入口、地块/道路 ID）
                 └─ LandCell（道路 / 建筑 / 预留庭院 / 尚无方案的剩余土地）
                       ↓
              既有规则与工程管线 → 最终实体、地形、可步行通路
                       ↓
              独立 DevelopmentAudit + RoadAudit + AccessAudit
                       ↓
              基于精确归属的土地面积预算 / SettlementGraph / HTML
```

没有删除失败条件，没有把未规划地表重新标成“自然完成”，也没有放宽道路坡度规则。

## 2. 新增与修改的实现

| 文件（相对权威工程） | 变更与边界 |
|---|---|
| `bevy/crates/core/src/settlement/development.rs` | 新模块：矩形差分、互斥分区、跨 Zone 的临路地块、BuildingPlan、独立重建、实体来源、面积及关系图 |
| `bevy/crates/core/src/settlement.rs` | 暴露 development 子模块；保留非 road-first 场景的旧规划合同 |
| `bevy/crates/core/src/worldgen.rs` | 在实体生成前建立 DevelopmentPlan，实体生成消费 BuildingPlan；不再生成旧巨型地块建筑；持久化计划、审计、面积和关系图 |
| `bevy/crates/cli/src/main.rs` | 从自然 raster 验证路网，再重建 development、面积和 graph；strict 完整 gate 保留未完成失败 |
| `bevy/crates/cli/tests/baked_audit.rs` | 新旧两条路径独立重建回归、双副本篡改、1km 跨 Zone/浮点序列化回归 |
| `adapters/html/app.js` | 新增 `?view=ownership` 只读诊断视图，投影 Rust 分区，不在 JS 推断地块 |
| `scripts/check-preview-camera.mjs` | 增加归属视图烟雾测试，保留 3D 相机朝向检查及截图 |

核心版本：`worldgen-2026-09-06-r18-land-accounting`。

### 2.1 迁移边界不是静默双写

- 有自然路网的 road-first 场景使用 `scene.developmentPlan`，`scene.parcelPlan` 必须为 null；不再发布旧轴线 `parcel-plan.json` / `infrastructure-plan.json` 作为另一权威来源。
- road-first 缺 development 或有 legacy parcel 同时存在，CLI 明确拒绝。
- 不启用新路网的场景继续旧 ParcelPlan，保留重建/来源检查。prototype 非 road-first 用例验证该兼容路径，不能借迁移绕过审计。
- 旧 r16 场景没有新字段/文件，应该重新 bake，不能直接把旧数据的版本字段改成新版本。
- `DevelopmentPlan` 中道路归属来自 RoadNetworkPlan 的真实边 ID；现阶段尚没有新的桥涵、排水或分级道路规划实现。

## 3. 土地分区与几何合同

### 3.1 用地分类、建设用途和对象投影分开

LandUseZones 继续保存 coarse urban/rural/nature/unassigned 目标分区。DevelopmentPlan 不擅自改变这些目标。

LandCell 是场景土地的完整、不重叠矩形分区，角色如下：

| role | 含义 | 是否自动计入实际完成面积 |
|---|---|---|
| road | 已规划道路占用，交叉区域按稳定顺序分配唯一归属 | 仅准确匹配实际道路时计入 transport |
| building | BuildingPlan 的 footprint | 仅准确匹配最终实体时计入其 urban/rural 用地 |
| yard | 临路地块减去建筑的剩余区 | **不计入**，目前只有预留，没有 landcover 完成证据 |
| unplanned | 其他剩余区 | **不计入**，目前没有明确开发/保护计划 |

“未规划”不代表没有自然高度场、也不代表没有旧模板农地。它表示尚未进入新的权威土地开发/保护合同。把这些区域自动算作完成会掩盖本项目最重要的缺口。

### 3.2 当前临路地块

- 24×24m 建筑 footprint；28×28m 地块；建筑 576m²、庭院预留 208m²。
- 保留临路选址已经确定的中心、角度、支撑标高、门口和 road ID。
- 建筑生成与最终基础变换恢复消费 BuildingPlan，不再从旧 Zone 块尺寸重新生成大房屋。
- 地块可能跨越 coarse Zone 边界。保存每个 sourceZone 的准确相交面积，而不是认领“最近的一个 Zone”。1km 实测有 3 个跨 Zone 地块。
- nature/unassigned 与住宅地块冲突不会被静默改标；专项审计报告 conflict 面积并失败。
- 28m 地块仅是土地归属范围，**不是完整工程填方过渡范围**。后者仍由工程管线与自然/最终 raster 审计负责；不能把地块外工程影响当作已解决。

### 3.3 分区算法

采用整数米、半开矩形，坐标运算为 i64。矩形与另一个矩形求交后，把剩余分成最多四个互斥矩形；道路按稳定边顺序依次从 Zone 剩余区切出，再切出建筑和庭院，最后完整保留剩余地表。

- 相交道路不重复记面积，交叉区域具有唯一 owner ID。
- 检查完整 Zone 覆盖、唯一 ID、合法坐标/尺寸、互斥地块、父对象、sourceZone 面积、footprint/yard 面积及每个 Zone 的面积守恒。
- 检查非法大坐标/尺寸及加法溢出边界；i64 字段不代表任意 i64 都可作为合法场景坐标。
- 独立重建验证比只验证面积总和更强：同时篡改 scene 与独立计划文件，仍须与输入重建结果一致。

该算法尚未完成密集 6×10km 城市的性能基准。测试中的最大场景只验证规划几何与面积，不能当作全生成性能结论。

## 4. BuildingPlan、SettlementGraph 与验收

BuildingPlan 有稳定 plan ID、parcelId、roadId、footprint 及完整预期 EntityInstance（含入口）。保留旧实体 ID 是为了明确来源连续性，不是重命名为另一实体后认领。

独立审计核对：

1. BuildingPlan 源实体唯一存在，变换、尺寸、kind/asset/scale/grounding 与入口一致。
2. audit 不会自动创建缺失入口，不会自动重新贴地；缺失或改变均失败。
3. 可步行 access 必须连接到**分配的道路**，仅连到任意其他道路不算履行当前 frontage 合同。
4. 最终出现没有 BuildingPlan 的建筑会列入 `unplannedBuildingIds`，不能因其外观合理就算通过。
5. 路面和地基的全 raster 验证沿用 road-network audit，development 实体匹配不是它们的替代。

SettlementGraph 的 Parcel ID 和 BuildingRelation 使用规划父关系，地块尺寸是 28m，不再等同于建筑宽深。当前只有一个 road-network settlement / frontage district，这是现有单连通低地子网的关系投影，**不是已经完成多中心城市生成**。

兼容字段 `nearestRoadId` 在该 v2 graph 路径记录的是分配 road ID，`relationReason` 明确标记 `planned_frontage_assigned_road_not_nearest_search`，距离字段为 null；未来 schema 应拆成明确 assignedRoadId 与独立 nearest-road 观测，不应混淆。旧 v1 路径仍是最近道路派生。

## 5. 土地利用比例控制的当前进展与缺口

`LandUseBudget.realizationAccounting` 标明核算来源：

- 新管线：`disjoint-owned-footprints-v1`；
- 兼容管线：`legacy-entity-proxy`。

新管线：

```text
assignedM2 = actualUrbanM2 + actualRuralM2 + actualNatureM2 + actualTransportM2
unassignedM2 = sceneAreaM2 - assignedM2
```

道路是独立 transport 面积，不全塞进 urban；农村住宅 footprint 归 rural；保护地内违规住宅不给比例完成量。旧 entityUrban/Rural/Nature 字段继续可用于诊断，但不再决定新管线的 assignedM2。

**尚未完成**：农田、牧场、保护地、庭院/步道、公共空间等需要真实地表范围、用途来源和完成合同。目前 existing 模板农田和绿地仍会生成，但因没有新 land ownership 来源而不计入真实用地完成面积。下一步应把它们迁移到 LandcoverPlan，不是回退到累加实体面积。

当前 urban/rural/nature 的目标比例算法仍是 coarse radial 分区；尚未增加显式 transport 目标预算与用户比例取舍策略，不能称为比例控制已经全部完成。

## 6. 实测（seed=20260906，temperate-plain，strict/rules）

输入沿用 `bevy/work/road-network-r15-{512,1000}.manifest.json` 的完整 profile。新输出目录为 `bevy/work/development-r18-{512,1000}`。

| 指标 | 512×512m | 1000×1000m |
|---|---:|---:|
| 最终实体 | 16 | 50 |
| 道路计划/实现 | 5/5 | 10/10 |
| 临路地块 / BuildingPlan | 1 / 1 | 9 / 9 |
| 精确实现且接入对应道路的建筑 | 1 | 9 |
| 跨 Zone 地块 | 0 | 3 |
| 土地互斥 cell 数量 | 51 | 198 |
| 全分区面积 | 262144m² | 1000000m² |
| 道路并集面积 | 2480m² | 4928m² |
| 建筑 footprint | 576m² | 5184m² |
| 仅预留的庭院 | 208m² | 1872m² |
| 无新开发/保护方案的剩余面积 | 258880m² | 988016m² |
| 实际归属 assignedM2 | 3056m² | 10112m² |
| development 几何/建筑硬错误 | 0 | 0 |
| development 状态 | incomplete | incomplete |
| 完整 world-quality-gate | **fail** | **fail** |

1km 相较 r16 多出两个非建筑 landcover 实体：移除旧巨型地块候选后，旧模板候选的占用/顺序条件发生变化。**未把这些新增实体算成新土地规划完成。**

道路专项审计、直接 CLI rules audit 均通过；直接 rules 与 bake 报告一致。自然高度场与 r16 两个同输入场景逐 chunk **字节完全一致**，证明本轮归属重构没有反向改变自然地形。

整体验收仍有两个真实失败：land-use realization underfilled；road-first development incomplete。不是旧地块无法关联造成的假缺失，而是剩余土地与庭院确实尚无实现合同。

聚合证据：`bevy/work/development-r18-verification.json`。最终 CLI 独立报告在各输出目录的 `cli-rules-audit.json`、`quality-report.json`、`world-quality-gate.json`。

## 7. 验证与修复记录

### Rust 测试

`bevy/work/development-r18-final-tests.log`：

- Core：269；App：44；CLI 单元：6；实际 bake 集成：8；共 **327 项通过**。
- 矩形差分覆盖 3136 组相交/包含/相离情况。
- 相交道路只计并集面积；跨 Zone 地块面积分配；土地完整守恒。
- 非法 source zone、碰路地块、错误 road、重复来源、缺入口、变换篡改、缺/重复实体、错 access road、违规保护地、无计划建筑。
- 6×10km **规划层**面积守恒，未做完整 bake。
- 实际 512m 场景篡改两份 plan，删除 remainder、改 owner/parent/yard，独立重建拒绝。
- 实际 1km 场景验证 3 个跨 Zone 地块、9 栋对应道路可达房屋，并验证 graph 序列化和父关系篡改。
- legacy 非 road-first 场景仍检测旧 ParcelPlan 伪造。

1km 实测发现 graph 文件中的 f32 均值被按任意 JSON f64 解析后精确比较会误报；现按声明的 SettlementGraph 类型反序列化再比较，不放宽 ID/结构检查。该场景增加了回归测试。

前端：22 文件、170 测试通过；typecheck 与 doc-tree 检查通过；lint 退出 0，仍有既有警告。日志为 `bevy/work/development-r18-{web-tests,typecheck,lint}.log`。

## 8. HTML 与视觉边界

新增只读 `?view=ownership`：蓝色道路、橙色建筑、浅色预留庭院、深色未规划区。图例明确“规划不等于完成”。JS 只投影 Rust 的矩形，未用浏览器计算地块、补路或核算建设成功。

本次启动端口 8831，使用 `bevy/work/preview-runtime-r18/` 内可执行副本；不会锁定 Cargo 的 glyphweave.exe。

- 归属诊断：`http://127.0.0.1:8831/preview/?view=ownership`
- 三维总览：`http://127.0.0.1:8831/preview/?view=skyline`
- 实际截图：`bevy/work/development-r18-1000/ownership.png`、`skyline.png`，旁边有运行时 JSON。

已人工看图：归属图确实显示 9 个临路地块与大面积未规划区，3D 加载了实际地形、房屋与农地。材质过亮和草覆盖路面等 renderer 问题仍在，**未声明三维视觉验收通过**。

## 9. 下一步必须做什么

1. **LandcoverPlan**：庭院、入口步道、农田、牧场、保护地、公共绿地从所有权几何生成，必须有真实表面层与可复算 realization；先移植旧模板 landcover，而不是新增装饰掩盖未规划区。
2. **道路/片区规划升级**：当前仅共同标高的最大低地分量。需要多片区、受限纵坡、地块需求反馈与明确终止条件；同时把前期中美欧日港形态研究落实到 district/building 规则，而不止固定 24m 房屋。
3. **土地与工程分开**：28m lot 不是工程影响边界；实现地块外填方、排水、挡土与跨地块冲突的统一工程合同。
4. **公共服务与自然保护**：公共设施服务半径、乡村道路/农田入口、水体与保护地硬边界，不能用剩余区统一刷绿。
5. **自然地形核心**：provider 拆分、原始地貌质量、侵蚀/沉积候选与固定版本许可证审核仍按主路线图继续。本轮没有改进侵蚀算法，没有集成 Mapgen4/symbios-ground。
6. **发布级核验**：多 seed、含水/山区、最大 6×10km 全 bake 性能与内存、真实地表渲染对照及全部许可证审计。

本轮无新增第三方许可证事项；现有 `THIRD_PARTY_NOTICES.md` 和主文档的候选研究结论不能替代未来引入具体版本的许可证确认。
