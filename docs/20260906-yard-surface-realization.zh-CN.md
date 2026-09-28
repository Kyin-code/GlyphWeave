# r21：庭院真实地表、门前铺装与渲染合同修复

日期：2026-09-06。权威工程：`G:\difyllmwiki\GlyphWeave`。
核心版本：`worldgen-2026-09-06-r21-facade-yard-surfaces`。
接续 `20260906-landcover-candidate-plan.zh-CN.md`。

**状态：庭院已从候选计划进入实际表面网格生成与独立审计。完整路线图与 strict 地图门禁仍未完成。**

> 后续农业执行与验收见 `20260906-agriculture-surface-realization.zh-CN.md`。本文 r21 数据作为庭院阶段记录保留。

## 1. 本轮不再只增加 schema

r19 的 LandcoverPlan 只有用途候选，全部 realizedAreaM2 为零。本轮让 Rust 直接生成庭院草地的 world-space 三角网格，并与独立生成的入口铺装求交/差：

```text
DevelopmentPlan + LandcoverPlan
  + 最终实体 + 最终高度场 + 结构化水体
      ↓ 独立重建实际可步行入口及 accessSurfaces
      ↓ 校验地块对应建筑及分配道路已实现
      ↓ 每米庭院单元：水体、斜向坡度、已有实物检查
      ↓ 与步道三角形裁剪，区分草地 / 已有铺装
      ↓ terrain-conforming surface mesh + 单元拒绝原因
      ↓ 独立重建、实际三角面积、土地预算、HTML 只读显示
```

未改写自然或工程高度场。没有为了实现庭院而挪走石头、放宽道路限制或重新填平地形。

## 2. 实现与拆分

| 文件（相对权威工程） | 本轮内容 |
|---|---|
| `bevy/crates/core/src/settlement/landcover.rs` | 新的实际地表执行/审计模块；不再把执行算子继续堆入 development.rs |
| `bevy/crates/core/src/settlement.rs` | 导出 landcover 模块 |
| `bevy/crates/core/src/worldgen.rs` | 将 SurfaceRealization 接入最终 bake；发布网格、审计；实际面积回填预算 |
| `bevy/crates/core/src/worldgen/access.rs` | 可步行路径从建筑真实外墙开始，而非止于偏离外墙的入口 marker |
| `bevy/crates/cli/src/main.rs` | 独立重建并校验 scene 和 landcover-surfaces.json 两份实际网格；合并已验证的地表证据 |
| `bevy/crates/cli/tests/baked_audit.rs` | 真实 bake→CLI 网格篡改、面积核算、缺层测试 |
| `adapters/html/app.js` | Rust 草地网格显示、入口铺装去重、抑制庭院区域的旧随机草、ownership 网格叠加、yard 近景 |
| `adapters/html/render/buildings.js` | 新 BuildingPlan 住宅只生成建筑本体，尺寸和门朝向服从 Rust，不再偷偷缩房屋、加院子/树 |
| `scripts/planned-home.test.mjs` | 四个方向的实际 Three 几何 footprint、总高度、门位置与子对象数量测试 |
| `scripts/check-preview-camera.mjs` | 浏览器运行时读取真实 mesh、门朝向、footprint 及 Rust 顶点投影误差 |

r20 是本轮中间实验。最终事实以 r21 产物与本文为准，不把中间全庭院通过结果混入最终验收。

## 3. 实际地表合同

### 3.1 归属与拒绝

只对 managed_yard 候选施工。农业/保护地/未来开发候选仍不计完成。

- 精确匹配该地块的 BuildingPlan 实体，且实体 ID 唯一。
- 独立重建 access graph，必须能走到分配的 road ID。
- 将 sourceCellId、parcelId、zoneKind 写入每块 mesh；拒绝也保留 sourceCellId、坐标、面积和原因。
- 全部最终地形覆盖及 hash 参与合同。丢失地形不能按零高程处理。
- 每米单元检查水体 footprint、已有树石/墙围栏等实物 footprint。实物相交使用 OBB 与 AABB 的分离轴检查，不仅判断中心点。
- 坡度上限为 **25% 的庭院地表坡度**，不是道路上限。双线性单元沿 X/Z 边缘最大梯度合成为斜向上界，不能只看一个方向而漏掉斜坡。
- 超坡、占用、水体交叉或建筑/通路未实现都会留下拒绝原因，不直接抹去候选面积。

### 3.2 几何生成与去重

- 以 1m 单元分割庭院。平面面积单位为 m²，不是斜面展开面积。
- 对独立重建的入口铺装三角形做凸多边形裁剪；扣除前面已经分配的部分，重复或相交的铺装不会重复计面积。
- 草地部分根据最终 metre raster 双线性高度生成顶点，附加 0.025m 薄覆盖偏移；铺装交集的 Y 来自已有铺装三角平面，不重新贴地。
- 网格采用 Y-up 正确绕序。相交面积和拒绝面积之和必须与庭院预算守恒。
- 双线性高度在 1m 网格上三角化是**可视地表近似**，不是真实草皮厚度、土壤或排水模拟。未声称与任意细分 terrain mesh 在所有内部点完全重合。
- landcover surface 与 access surface 都会保存铺装交集证据，但 HTML **只渲染一次原 access mesh**；landcover 的铺装 patch 不再重画，避免 z-fighting。

### 3.3 持久化和独立检查

`scene.landcoverSurfaces` 与 `landcover-surfaces.json` 保存网格。CLI 根据已验证的 DevelopmentPlan、候选 LandcoverPlan、完整最终地形、真实实体重新生成预期网格，分别检查两份实际产物。

检查内容：

- ID、source/parent、material、zoneKind、版本、heightfield hash、拒绝集合。
- 顶点、三角索引、绕序、实际三角面积与保存面积一致。
- 非有限数字、删 patch、改高度/水平坐标、改 owner、改面积、索引越界均拒绝。
- 顶点数值误差限 1e-8m，局部面积误差限 1e-7m²，仅容纳 JSON/浮点残差。小于 1e-7m² 的 unresolved 残差归零，不能抹去整米拒绝单元。
- 不信任保存的 landcover-audit.json。写入假 pass 不能让坏网格通过。

LandUseBudget 新核算标识为 `disjoint-owned-surfaces-v2`。已验证的庭院网格面积按源 urban/rural 计入实际面积；建筑、道路仍用精确所有权面积，未规划地表和旧农地 proxy 仍不给完成量。发布时必须经过独立 surface audit，预算函数自身的几何求和不是安全/实现保证。

## 4. 两个实测中发现并修复的问题

### 4.1 入口铺装没有接到外墙

旧 front anchor 比建筑边界外移 1m。旧 access 算法从这个 marker 开始，导致可步行报告虽通过，实际铺装离门有缺口。

现从建筑真实外墙最近点出发，经外侧 clearance 点连接道路；外墙点和 clearance 点都检查最终地形台阶差与其他障碍。marker 继续作为入口语义，不再被当作实际铺装起点。

本场景通路现在从 24m 房屋边缘到道路边缘，长度 **8m**。早前 7.3m 数值是含 marker 处折返的旧路径，不应继续引用为最终证据。

### 4.2 HTML 独自解释“房屋=院子”

旧 makeResidentialHome 用 entity.width/depth 生成草坪，再把房屋缩到约 60%，自行加树，门还放在 +Z，而 Rust front 是 -Z。这与 BuildingPlan 的 footprint 明显矛盾。

有 BuildingPlan 的住宅改用独立 renderer：

- width/depth 是建筑本体 footprint，不是院子范围；高度含屋顶不超出 heightM。
- 不生成草坪、树、额外 plinth 或围合物；庭院只来自 Rust surface。
- 门与前窗位于局部 -Z，并随 serialized yaw 旋转。
- 门面片向外 0.002m 防止与墙面重叠；几何测试以此明确薄面容差。
- 没有 BuildingPlan 的旧实体仍使用兼容 renderer，不声称已迁移所有房屋类型。

## 5. 最终固定种子结果

seed=20260906；输入仍为 `bevy/work/road-network-r15-{512,1000}.manifest.json`；新输出为 `bevy/work/yard-r21-{512,1000}`。

| 指标 | 512×512m | 1000×1000m |
|---|---:|---:|
| 道路 / 临路房屋 | 5 / 1 | 10 / 9 |
| 庭院预算 | 208m² | 1872m² |
| 实际草地网格面积 | 205m² | 1790m² |
| 庭院内已验证入口铺装面积 | 3m² | 27m² |
| 实际庭院总面积 | 208m² | 1817m² |
| 未实现庭院面积 | 0 | **55m²** |
| 未实现原因 | 无 | yard_grade_exceeds_25_percent |
| surface patches | 5 | 49 |
| 全用地 assignedM2 | 3264m² | 11929m² |
| 仍无实际表面证据的 Landcover 面积 | 258880m² | 988071m² |
| 道路/住宅专项审计 | pass | pass |
| 独立 rules audit | pass | pass |
| 完整 world-quality-gate | **fail** | **fail** |

最早 r20 的单轴坡度实现误认为 1km 的 1872m² 全部可实现。补充斜向梯度检查后，55 个单元明确拒绝。未降低坡度来保留原来的漂亮数据。

自然高度场与最终工程高度场，相对 r19 均逐 chunk **字节完全一致**。本轮实际新增的是可审计表面层以及延伸到外墙的铺装，不是地形再造。

证据汇总：`bevy/work/yard-r21-verification.json`。CLI rules 与 bake rules 内容相同，最终产物在最新代码添加实物障碍检查后再次独立验证通过；完整 gate 仍按真实未完成项失败。

## 6. 测试与 HTML

`bevy/work/yard-r21-verified-tests.log`：Core **279**、App 44、CLI 单元 6、真实 bake 集成 9，共 **338 项通过**。

覆盖：

- 平地庭院全面积守恒、与入口铺装不重复、顶点高度与绕序。
- 斜向/包含/相离三角裁剪，大正负 world origin 的数值稳定性；重复铺装只计一次。
- 缺房屋/缺道路不产生施工面积；超坡庭院拒绝；已有旋转石头不删除也不覆盖。
- 双文件同步伪造坐标、Y、面积、owner、索引、缺 mesh 都不能通过。测试同步与攻击无关的预算，保证真正检查到地表独立重建，而非提前因陈旧面积退出。
- 独立确认 access pavement 起点在实际建筑外墙。

前端 `pnpm test`：170 项；`node --test scripts/planned-home.test.mjs`：4 项实际 Three 几何测试；typecheck/doc-tree 通过；lint 退出 0 但仍有既有警告。

HTML 当前端口 8834，使用 work 中可执行副本：

- `http://127.0.0.1:8834/preview/?view=yard`：首个临路房屋近景。
- `http://127.0.0.1:8834/preview/?view=ownership`：实际 surface 三角形叠加在土地归属图上。

实际截图：`bevy/work/yard-r21-1000/yard.png`、`ownership.png`。运行时报告及日志：`yard.png.json`、`bevy/work/yard-r21-preview.log`。

近景运行时验证了 3 个 BuildingPlan 房屋、22 个草地 patch：

- 门位置误差 0；无 renderer 私自添加的院落子物件。
- 建筑超过 footprint 的最大值 0.002m，来自明确门面片偏移。
- Rust 顶点进入 Float32 后最大差约 0.00001425m；没有四倍高度错误或再次贴地。

已人工查看近景，建筑本体、门前步道和庭院网格能够显示。但场景整体仍过亮，旧草叶密度/覆盖和单 chunk 显示边缘仍不好；此处只验证新地表/建筑合同，**不宣称最终现代地图视觉质量已通过**。

## 7. 下一步必须继续

1. 将 agriculture candidate 变成实际田块：全地形坡度/水体/通达性、灌排/田埂与土地经营面，而不是重新累计旧 farmland 盒子面积。
2. 处理 55m² 不适宜庭院：选择保留裸露地、明确挡土/台阶或修改合法建筑地块方案；只能有工程依据，不能自动判完成。
3. 将 real surface 存储逐步改为 chunk/空间索引，避免高密度大图 JSON mesh 和 repeated pavement reconstruction 的成本；当前尚无 6×10km 密集地表性能验收。
4. 补充 road frontage、庭院外入口 access、农业运输和公共设施的土地/工程归属；当前仅计庭院内铺装部分，不把通路外侧面积随意算进庭院。
5. 自然地貌 provider/侵蚀候选与许可证核验、地区人文布局规则、多片区道路、最大地图性能都仍属于完整目标，不能被本轮庭院完成替代。

## 8. 来源与许可证

本轮凸多边形裁剪、地形采样、表面执行、审核及渲染修复为项目自有实现，没有复制外部项目源码、增加依赖或资产。测试使用仓库既有 Three 和 Playwright。没有对候选开源地形库作新的许可证结论，也没有把研究参考当作已集成。仍需在具体引入版本前核对 LICENSE/NOTICE/依赖树并更新 THIRD_PARTY_NOTICES。
