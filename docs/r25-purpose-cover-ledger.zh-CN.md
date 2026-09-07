# r25：用途—物理表面双维度账本与真实覆盖缺口

> 权威工程：G:\difyllmwiki\GlyphWeave。
> 接续：r24-zoned-planning-and-terrain-protection.zh-CN.md。
> 状态：统计与验收依据重构；不是新增建筑/田块/生态生成，**整个路线图仍未完成**。

## 1. 本轮先解决什么

r24 文档要求先查明“农村为 0”的真实原因，再推进公共设施与生态地表生成。现已定位到两套不相容的统计：

- LandUseBudget 的 actual 字段已按真实归属与地表计算。
- quality-report 的 LandUseArea 与比例警告仍读取 analyze_landuse_areas：把 residential_home、road 等类型的矩形面积相加，几乎全部当成 urban；真实田面是 SurfacePatch 而非 farmland 代理实体，因此未被计入。
- 旧警告把 urban/rural/nature 的实体代理面积再次归一化到 100%，隐藏了大部分地图尚未实现的事实。

这不仅是显示错误：后续容量规划如果读取这份比例，就会错误地继续增加农村内容或错误评价城市密度。

本轮没有把旧住宅改名为农村建筑，也没有为通过检查新增虚构田块。我们建立按来源几何统计的账本，保持所有真实生成内容不变。

## 2. 新账本：用途与物理表面是不同维度

新增：bevy/crates/core/src/settlement/landuse_ledger.rs。

用途行：urban / rural / nature / unassigned。

物理表面列：

- buildingsM2；
- roadsM2；
- managedGrassM2；
- accessPavementM2；
- cultivatedSoilM2；
- fieldHeadlandM2；
- farmTracksM2。

例如“农村用途内的道路”同时具有 rural 用途和 roads 物理属性，但在总面积里只算一次。transportSubsetM2 是已实现面积的子集，**不能再加到各用途行之和**。

每行分别提供：targetRatio、targetM2、plannedM2、plannedSceneRatio、realizedM2、realizedSceneRatio、unresolvedM2、targetDeficitM2、targetExcessM2、physical。

守恒关系：

    每行 realizedM2 = 各物理表面列之和
    每行 plannedM2 = realizedM2 + unresolvedM2
    全图 sceneAreaM2 = 各用途行 realizedM2 与 unresolvedM2 之和

realizedSceneRatio 的分母始终是完整场景面积，不是“已经有东西的区域”。目标不足 100% 时保留未指定份额，不把 99% 请求强行放大成 100%。例如平原请求农村目标保持 0.8，旧 mix 警告的约 0.808 不再作为该场景的依据。

## 3. 面积来源与安全边界

### 3.1 建筑和道路

从 development 的半开归属 cells 读取面积，不用可见实体的宽×深代理求和。提取 realized_cells，供既有预算与新账本共享精确 owner 匹配：

- 来源必须是 RoadNetworkPlan 或 DevelopmentPlan 的实体。
- 同 ID 最终实体必须恰好出现一次。
- 位置、尺度、尺寸、朝向、锚点等必须符合原计划的 same_entity 合同；派生 bounds 不参与来源身份。
- 缺失、重复或移动过的 owner 不获得建筑 footprint 面积。
- 交叉路面只按归属分区并集计数一次。

urban/rural 都可开发，一个房屋跨越二者时按实际分区面积切分，而不是把整栋房屋归给中心所在区；跨自然保留用途仍受 r24 约束禁止。

### 3.2 庭院与农业表面

读取已验证 SurfacePatch 的三角网格投影面积，核对 sourceCellId 与真实 landcover cell：

- 重复 patch ID、缺失来源、用途与来源不同、超来源边界、非有限/错误声明面积均拒绝。
- 每个来源单元的累计表面面积不得超过其归属面积。
- 管理草、步道、耕作区、地头、农道分别记账。

**analyze 是账本函数，不是完整几何/安全审计器。** 它不独立证明所有三角形互不重叠、地面坡度或农道可达性。CLI 必须先完成道路、开发归属、landcover 的独立重建与验证，再调用账本；现有调用顺序保持这一前提。bake 同样先生成并验证地表再发布账本。

候选田块、延后开发请求、尚未实现的自然保护管理计划不获得面积。自然高度未被破坏也不会自动得到 ecology realization 面积。

## 4. CLI 与文件接口变化

### 新发布文件

每个 road-first 场景生成 land-use-ledger.json。它是便利诊断报告，CLI 不读取其 status 或面积作为接受证据；即使保存的报告被伪造为 pass，CLI 仍从真实场景来源重新核算。

旧世界缺少这一新增报告时，HTML 不显示账本，但不影响原有地图渲染；CLI 在满足既有 road-first 来源合同的场景上可以重新核算。

### quality-report v2

报告格式 glyphweave.quality-report 的 version 升为 2：

- scenes[].landUseLedger：独立验证输入后重算的账本。
- scenes[].landUseArea.accounting：明确 disjoint-purpose-cover-v1 或 legacy-entity-proxy。
- road-first 的 urbanM2/ruralM2/natureM2 及 Ratio 改为真实用途面积/全图比例。
- 原始类型统计保留在 entityFootprintDiagnostic，明确是 legacy-overlapping-entity-proxy-not-realization。
- 原 byKindM2 不再作为顶层真实实现证据，客户端应从 entityFootprintDiagnostic 读取它。

这是一项有意的报告语义修正，不能在旧 version 1 下悄悄沿用。WorldIndex/SceneIndex 存储版本没有改变。

### 既有 LandUseBudget

为兼容既有来源验证，LandUseBudget 字段不改：其 actualTransportM2 是与 actualUrban/Rural 分列的运输面积；新账本的 transportSubsetM2 则已包含在用途行里。消费者必须读 accounting，不得把两者混加。

平原例：旧 actualRuralM2 24000，加农村道路 10608、农村农道 1568，得到新用途账本 rural 36176；这不是多实现了 12176 m²，而是正交维度的重组。

### 门禁不因统计修正变绿

- 新增 strict gate 对 landUseLedger.status 的检查。
- zoning 的 0.05 全图比例容差不放宽。
- 每个用途的未实现面积仍输出明确警告。
- 原 landcover/development/protection/rules 门禁保留。
- 不再对 road-first 使用旧实体代理归一化比例判定；legacy 分支仍保留旧诊断，未宣称迁移全部旧场景。

allocationStatus pass 只代表规划分配容差通过；realizationStatus 只有剩余面积归零（数值误差范围内）才是 pass。账本本身不证明公共服务、生态或视觉质量，因此即使账本某一天通过也不等于全项目完成。

## 5. 三个实测场景

输入沿用 bevy/work/height-r23-{512,1000,plains}.manifest.json。
输出：bevy/work/ledger-r25-final-{512,1000,plains}。
汇总：bevy/work/ledger-r25-evidence.json。

| 指标（m²） | 512 m 起伏 | 1000 m 起伏 | 512 m plains |
|---|---:|---:|---:|
| 城市用途实际实现（含区内交通） | 3264 | 6568 | 6976 |
| 农村用途实际实现（含区内交通） | 0 | 880 | 36176 |
| 自然用途实际生态实现 | 0 | 0 | 0 |
| 总实际实现 | 3264 | 7448 | 43152 |
| 其中运输面积子集 | 2480 | 4520 | 15792 |
| 全图未实现 | 258880 | 992552 | 218992 |
| 规划分配检查 | pass | pass | pass |
| 实际实现检查 | incomplete | incomplete | incomplete |
| 完整 gate | **fail** | **fail** | **fail** |

平原农村用途 36176 m² 分解：建筑 7872、道路 10608、庭院管理草 2839.5、步道 40.5、耕作土 5888、地头 7360、农道 1568。

平原全图农村实际比例约 13.80%，离目标 80% 仍有巨大差距；“不是零”并不代表“足够”。起伏 512m 场景农村仍然真实为 0，不将错误统计修复泛化成所有场景都有农业。

## 6. 证明没有用重分类制造产出

与 r24 对应目录逐项比较：

- entities、RoadNetworkPlan、DevelopmentPlan、LandcoverPlan、SurfaceRealization 全部一致。
- heightFile、naturalHeightFile、surfaceFile、terrainFile、lod2File 逐字节一致。
- 512/plains 各比较 5 份 payload，1km 比较 20 份。
- 实际道路仍 5/9/29，规划住宅仍 1/3/18，田块仍 0/1/23。

本轮改变的是来源一致的诊断/验收账本和 HTML 读数，不是生成覆盖。这一界限必须保留在后续进度说明中。

## 7. HTML 实际诊断

adapters/html/core/world.js 可选读取 Rust 发布账本，检查版本、模型、场景身份、面积与显示字段数值合法性。不在浏览器推断用途或计算生成配额。

adapters/html/app.js 的 ownership 视图新增表格：目标比例、规划面积、已实现面积、未实现面积。明确显示“Rust bake 诊断，独立验收请运行 CLI”。

浏览器报告不是防篡改证据；显示校验只用于防止错误文件、无效数据和旧世界兼容问题。

独立浏览器检查：scripts/check-landuse-ledger-preview.mjs。

- 正常表格逐字段等于实际发布的 Rust 报告。
- 缺失文件、错误 sceneId、非法数字时不显示伪造数值，原地图继续渲染。
- 保留既有相机/高度/农业网格检查。

首轮浏览器回归实际失败：新增读取逻辑最初接在 core/world.js 的旧 loadSceneData，而当前 app.js 直接加载 scene.json，未经过这一入口。现已提取共享 loadLandUseLedger，接入实际 loadScene；场景切换用 token 防止较慢旧请求覆盖新场景。未改测试去接受“表格不存在”。首轮日志 ledger-r25-preview-ledger.log 保留。

当前阶段预览端口 8839，ownership 路径显示账本，farm 路径显示原实际几何。HTML 两份改动已从 adapters 源文件同步到三个 r25 输出目录；这一同步没有修改世界数据。

- 账本截图：bevy/work/ledger-r25-final-plains/ledger-ownership.png。
- 农业截图：bevy/work/ledger-r25-final-plains/ledger-farm.png。
- 表格及缺失/非法文件回归：bevy/work/ledger-r25-preview-ledger-verified.log。
- 相机、毫米高度与农业 mesh 回归：bevy/work/ledger-r25-preview-farm-verified.log。

已实际查看账本截图，表格与 Rust 发布结果一致且明确显示缺口；这不是材质或现代城市视觉终验。

## 8. 测试与验证记录

Rust workspace：Core 303、App 44、CLI 单元 6、真实 bake 集成 13，合计 **366 项通过**。最终结果见 bevy/work/ledger-r25-final-tests.log。

新增 4 个账本单元回归：

1. 非零 origin、跨 urban/rural 道路、真实田面/住宅守恒；实体类型不变也能正确记农村用途面积。
2. 缺失、重复、移动建筑不获得 footprint credit。
3. 声明面积、来源、用途、重复 patch 伪造拒绝；缺少 realization 合同拒绝。
4. 空输出/候选自然地不可宣称完成，不把已用区归一化掩盖全图缺口。

扩展现有真实 bake 集成测试：检查 v2 报告、新旧总量守恒、农村实际非零与旧代理零的区别；伪造保存账本 pass 不影响独立输出；严格门禁明确因用途实现不完整失败。原 r24 像素/计划篡改回归仍在。

前端 Vitest 170 项、Node 高度/住宅 mesh 合同 7 项通过；新增实际浏览器检查通过正常报告和 3 种缺失/非法报告情形。lint/typecheck/doc-tree 退出 0，lint 仍有既有警告。最终 v2 quality/gate 重算日志为 ledger-r25-final-{512,1000,plains}-gate-v2.log；三个完整 gate 仍 fail。

## 9. 对下一步开发的约束

### 必须继续，不能把账本当完工

1. **公共设施实际规划**：DevelopmentProposal 仍是延后请求。下一步应把有容量和入口/道路要求的市政、学校、商店、市场等需求转为真实合法地块与建筑，不再回到模板直接 add。
2. **农村生成能力**：平原农村实际仍不足规划面积的五分之一。田块/道路可达性与工程可行性仍需改进；不能仅增大统计 credit 或放宽坡度。
3. **精细用途配额**：当前只按城市/农村/自然/未指定闭环；农田与牧场、商业与住宅、市政/绿化等子类尚没有各自的完整配额实现合同。
4. **生态 realization**：森林、湿地、自然公园等必须有真实生态/地表实现与边界合同；高度保全不等于生态完成。
5. **自然核心与最大图性能**：provider 拆分、自然侵蚀候选与许可证、大地图内存/耗时、工程排水/桥隧等 r24/主路线图缺口未被本阶段替代。

### 下一步不能直接复活旧模板

已检查平原输出的 16 条延后请求：包含 160×110m 商业中心、130×100m 娱乐中心、80×60m 学校、88×64m 市政厅、两种大型工业建筑等。这些是旧生成器请求历史，**不是用户明确要求的设施清单**，不能因为它们存在就全部塞回一个只有 18 栋规划住宅的场景。

下一阶段必须先区分显式用户需求、地区/聚落规模推导的需求、旧模板诊断；然后支持不同尺寸合法地块和入口/服务容量。现有 RoadsideSite/住宅固定 24×24m，无法证明能安置 80×60m 学校，更不能把大设施强制缩小成住宅尺寸冒充实现。

### 已发现的性能边界

账本复用 exact owner 匹配、遍历归属 cells 与表面三角形；还调用现有分区/source 校验。它不会生成额外全图 raster，但也没有最大 6×10km 实测。不能因小图测试快速就宣称最大图性能完成。

## 10. 来源与许可证

本轮为项目自己的 Rust 记账与来源验证重构、CLI 集成、HTML 只读展示和测试。没有复制外部地形算法、引入第三方依赖或下载资产；不新增任何 Mapgen4/symbios-ground 兼容性结论。

全项目开源来源审计仍按主路线图和 THIRD_PARTY_NOTICES.md 推进。统计实现通过不等于自然算法或资产许可证审计通过。
