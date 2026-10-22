# 交接文档（GlyphWeave · B/C 里程碑推进）

> **2026-10-23 道路纵断面/跨线/碰撞**：最新可验收输出为
> `bevy/work/rk-x/v3-final-r169-20261023/`，`scale-audit=pass`。
> r155 的 2,451/2,588 个道路中心线采样低于地形，最大埋深 14.716m；根因是
> 路线设计面未进入最终地形 carve，并叠加规划历史 8m→32m 的实体单侧拓宽。
> 现已为任意朝向路线分段发布 `routeGrade`，道路 corridor 同步挖填；
> 高差冲突的交叉、共线重叠和宽 footprint 重叠发布为 41 个合并跨线 span；
> 低位道路保留独立地形与 OBB 碰撞。r169 地面中心线偏差中位数
> -0.0004m，最大埋深 2.817m，超过 0.75m 的剩余采样 16 个。
> 逐段实体宽度隐式拓宽已停用；未来必须升级为 segment-local width 并同步
> carve、HTML、碰撞后再开启。当前仍未完成 Rust 路口并集多边形、桥头挡墙与
> 剩余局部高差。详见
> [`ROAD-GRADE-COLLISION-20261023.zh-CN.md`](ROAD-GRADE-COLLISION-20261023.zh-CN.md)。

> **2026-10-23 核心密度/道路碰撞复核**：当前工作分支
> `refactor/authoritative-baked-audit`。已统一 Rust OBB、入口局部坐标与
> THREE 正 Y 旋转；建筑可见 root 保留权威 yaw，立面翻转仅在子节点完成。
> 道路改为同一横断面设计面、分级车道断面与独立碰撞链，建筑用途恢复
> 办公/店面/混合住宅/市场的确定性混合。r155 输出
> `bevy/work/rk-x/v3-final-r155-20261023/`：4,371 实体、335 最终建筑、
> 309 入口楼梯、34,811.2m² footprint、1,475 道路段，中心 150m 为 62 栋；
> `scale-audit=pass`、`final-entity-audit=pass`、市场覆盖 96.00%，
> 碰撞/悬空/穿地/落水均为 0，跨 Chunk 最大高差 2.834m。
> `quality-report=warn`，仍有 23 栋建筑的所有入口失败；运行态战略图和
> 街景已检查，设计高程驱动的路面连续、侧立面窗带、街廓内部住宅填充、
> 侧门楼梯、交叉口附属带裁断、封闭盖面和 approach 步行化已补齐，
> 但单体体量、路口边缘模板、入口工程和街具仍未验收。
> 不得把 r144 当作完整 B/C 签收。
> 参考审计见
> [`REFERENCE-AUDIT-20261023.zh-CN.md`](REFERENCE-AUDIT-20261023.zh-CN.md)，
> 实现记录见
> [`URBAN-DENSITY-ROAD-COLLISION-20261023.zh-CN.md`](URBAN-DENSITY-ROAD-COLLISION-20261023.zh-CN.md)。

> **2026-10-22 城区密度、道路与立面复核**：本轮修复了道路宽度被全城统一
> 目标吞掉、旧城只允许单侧街墙、斜向道路/地块 yaw 镜像、规划拓宽二次
> 穿楼、市场斜向入口锚点与入口高差等根因。香港 1km r65 为 3,293 实体、
> 140 最终建筑、686 道路段、434 树；中心 150m 半径 46 栋，最终
> 建筑—建筑/道路/草木碰撞、悬空、穿地、落水全部为 0；6/6 市场物化，
> 10 分钟覆盖 93.46%，AccessGraph 0 断点。scale 与 quality 均 pass。
> 浏览器地址为 `http://127.0.0.1:8165/preview/index.html?view=street`，
> 输出目录 `bevy/work/rk-x/v3-density-road-r65-20261022/`。详见
> [`DENSITY-ROADS-FACADES-20261022.zh-CN.md`](DENSITY-ROADS-FACADES-20261022.zh-CN.md)。

> **2026-10-22 最终实体占用与接地回归闭合**：提交 `c965dc6` 已推送。
> 任意 yaw 只在 0/90/180/270° 使用精确轴对齐，其他整数度不再误退化为
> AABB；道路先占据规则放置索引，建筑不得压路；多阶段 placement 审计选择
> 真正含 `inputItems` 的 reconciliation。正式烘焙执行
> `final-entity-audit.json` 失败关闭。香港 r27 为 2,901 实体、53/53 入口
> 连通、5 市场/100% 覆盖，建筑碰撞/悬空/穿地/落水均为 0；街景加载邻接
> Chunk，规划道路按统一 polyline 绘制。scale/quality 和 Computer Use 三视图
> 均 pass；最终 workspace 为 1,140 passed / 0 failed / 37 ignored。见
> [`HARD-OCCUPANCY-GROUNDING-REGRESSION-20261022.zh-CN.md`](HARD-OCCUPANCY-GROUNDING-REGRESSION-20261022.zh-CN.md)。

> **2026-10-22 生鲜市场辐射证据闭合**：提交 `b3b5502` 已推送。市场计划
> 现在同时发布 `market-radiation.json` 与 `market-deficit.json`，按真实
> 路网输出 5/10/15/30 分钟覆盖、逐需求市场分配、竞争压力、市场负载、
> 利用率和具名缺口；quality-report 独立校验分配守恒。香港 r28 为
> 5/10/15/30 分钟覆盖 `0.582/0.821/0.998/0.998`，总覆盖 `0.997971`，
> 唯一 8.624 需求缺口因容量耗尽被保留。见
> [`MARKET-RADIATION-20261022.zh-CN.md`](MARKET-RADIATION-20261022.zh-CN.md)。
> 最终 workspace 为 `1,141 passed / 0 failed / 37 ignored`。

> **2026-10-21 切面焦点驱动布局**：`urbanSlice` 现已产出可审计的
> `focusBias`，并实际改变推断时代预算、中心角色、道路拓宽、公园和宏观工程
> 预算。填海/造岛/特区/山地/中央规划可声明无绝对土方上限的规划授权，但仍须
> 工程预算和施工审计；局部 6m 填方、8% 纵坡、25% 步行坡、2m 削方与水岸/
> 保护合同不变。见
> [`FOCUS-DRIVEN-PLANNING-20261021.zh-CN.md`](FOCUS-DRIVEN-PLANNING-20261021.zh-CN.md)。
> 香港 r2 实烘焙 2,998 实体、39 入口连通/0 断点，scale/quality 均 pass。

> **2026-10-21 48 城先验正式消费**：`referenceData.caseIds[0]` 现在会在
> morphology 编译前自动补齐 `naturalGeography`、`urbanSlice`、
> `communityProfile`、案例历史元数据和启发式时代预算；作者手写字段优先，
> 未知案例失败关闭。推导预算的来源字段可通过下游严格 schema，不会被误判为
> 未知字段。
> 详见
> [`URBAN-CASE-PRIOR-COMPOSITION-20261021.zh-CN.md`](URBAN-CASE-PRIOR-COMPOSITION-20261021.zh-CN.md)。

> **2026-10-21 新旧城连续街墙与时代继承**：旧城改用窄街取样间距，时代预算
> 按同一街侧连续消费，单栋孤点不冒充街墙；最终实体继承 epoch 的
> `preserve/modify/replace` 与父对象。香港 1km r7 为 33 栋最终建筑，
> 其中 old-core 7 栋形成 114m 连续街墙（覆盖率 0.771、最大缺口 4.37m），
> renewal 19 栋 replace、new-growth 7 栋 new；需求闭合，AccessGraph 38/0，
> scale/quality/visual 均 pass。详见
> [`EPOCH-STREETWALL-20261021.zh-CN.md`](EPOCH-STREETWALL-20261021.zh-CN.md)。

> **2026-10-21 沿街跨阶段重放修复**：`frontage::append` 在正式链中先于
> `plan.roadCutMm` 发布。诊断、支撑审计和 bounded-cut 重放现在先恢复该前置
> 快照，并补齐已接受地块的自然范围采样与 `profile_roads` 顺序。唯一 Core
> 回归已关闭；定向 1km 为 89 实体、4 户/16 店、AccessGraph 31/0，
> scale-audit 与 road-network-audit 通过。详见
> [`FRONTAGE-STAGE-REPLAY-20261021.zh-CN.md`](FRONTAGE-STAGE-REPLAY-20261021.zh-CN.md)。

> **2026-10-21 HTML 街景修复**：街景相机修正
> `streetHotspot.road` 包装对象读错导致的 NaN、`routeFocus` 覆盖街景点、
> approach 被误选为主街以及 OrbitControls 拉回旧状态。店铺改为真实
> frontage 朝向并补齐四面上层窗带；办公塔进入分段塔楼渲染。r6 控制
> 3,011 实体、41 连通/0 断点，scale pass；运行截图非空，但视觉反馈仍为
> `warn`（前景留白、材质偏简、战略图偏 3D 沙盘）。见
> [`HTML-STREET-CAMERA-FACADE-20261021.zh-CN.md`](HTML-STREET-CAMERA-FACADE-20261021.zh-CN.md)。

> **2026-10-21 48 城完整性审计**：新增 `pnpm check:urban-cases`。48/48
> 均具备自然地理字段、三阶段历史、城市四层结构、土地压力、来源入口和
> OSM 测量窗口。审计同时明确 `heuristicGeographyCases=48`：自然地理仍是
> mechanism-prior，不能宣称已逐案升级为 source-verified。见
> [`URBAN-CASE-COMPLETENESS-AUDIT-20261021.zh-CN.md`](URBAN-CASE-COMPLETENESS-AUDIT-20261021.zh-CN.md)。

> **2026-10-21 香港中环机制切面完成**：新增合成 1km
> `feature-cut`，表达陡坡山体、旧市场核心、更新台地、新城区和东侧填海水岸。
> 修复市场挤占规划建筑、办公/住宅缺少 support carve、对角入口锚点、
> 门前 landing、路口建筑标高、门区植被阻挡和全景裁切。r2h 最终
> 35 个规划建筑中 34 个物化，生成 23 个短梯 + 3 个长直梯，6 个市场，
> AccessGraph 41 连通/0 断点；
> scale、quality 与 visual feedback 均 pass。7 个严格入口拒绝保留具名；
> 直梯落地后拒绝从 7 降至 1；入口后市场需求以最终 34 栋重算并保持
> 约 100% 覆盖。证据见
> [`HONG-KONG-CENTRAL-MECHANISM-CUT-20261021.zh-CN.md`](HONG-KONG-CENTRAL-MECHANISM-CUT-20261021.zh-CN.md)。

> **2026-10-21 已知道路缺陷**：自然道路中 45°/135° 等斜向段占多数之一，
> Core 现已统一到 oriented footprint：carve 保存真实 yaw 与局部半长，
> 土方、索引、bounded-cut、transport 与桥面审计使用同一格点判定。
> 轴对齐 0°/90° 的旧边界保持不变。r3c 重烘为 3,010 实体、34/35 建筑、
> 6 市场、25 楼梯、AccessGraph 41/0，scale/quality pass；街景相机已改为
> 沿真实道路轴线，密度与材质仍 visual `warn`。见
> [`ORIENTED-ROAD-CARVE-20261021.zh-CN.md`](ORIENTED-ROAD-CARVE-20261021.zh-CN.md)。

> **2026-10-21 新旧城实体继承**：最终建筑现携带
> `districtId/constructionCohort`。香港 r3d 为 old-core 10、renewal 15、
> new-growth 9、periphery 0；不再只能在计划侧识别时代。见
> [`DISTRICT-ENTITY-PROVENANCE-20261021.zh-CN.md`](DISTRICT-ENTITY-PROVENANCE-20261021.zh-CN.md)。

> **2026-10-21 工程账本统一**：bounded-cut 已纳入门前 landing、非 frontage
> 弱削方建筑和最终道路实体；旧 6 个失败关闭 5 个。R31 1km r32l 重烘成功。
> 剩余 frontage diagnosis 仅一个原因分类不同。见
> [`ENGINEERING-LEDGER-UNIFICATION-20261021.zh-CN.md`](ENGINEERING-LEDGER-UNIFICATION-20261021.zh-CN.md)。

> **2026-10-21 市场占地去重叠**：市场候选现在按离路 12m 后的真实 footprint
> 检查 2m 净距，不再只按路线节点去重。`market.4` 从与 `market.1` 重叠的
> 节点改选到 `(378,506)`；1km 控制最终 4 个市场，AccessGraph 20 连通/0
> 断点，scale/quality/visual 均 pass。见
> [`MARKET-FOOTPRINT-OCCUPANCY-20261021.zh-CN.md`](MARKET-FOOTPRINT-OCCUPANCY-20261021.zh-CN.md)。

> **2026-10-21 坡地入口台地**：修复 `world_y` 整数米与毫米 Heightfield
> 之间的半米量化误差；聚落建筑 pad 改用道路设计标高，控制切面声明
> `maxPadCutM=12`，短引道不再额外抬高 0.5m。1km 最终普通建筑 12→16，
> 市场 3→4，AccessGraph 20 连通/断点 0，scale/quality/visual 均 pass。
> 仍有 8 个规划建筑因真实坡地高差待 stairs/switchback 工程，均带
> `accessDiagnostics` 具名记录。证据见
> [`TERRACED-ENTRY-ENGINEERING-20261021.zh-CN.md`](TERRACED-ENTRY-ENGINEERING-20261021.zh-CN.md)。

> **2026-10-21 街景空屏修复**：`?view=street` 不再聚焦单个边缘 storefront，
> 改用城市实体质心和稳定斜俯视，运行态不再出现大面积黑屏；窗口/全屏 resize
> 会同步 WebGLRenderer、透视相机 aspect 和 postFX。r6 控制
> scale/quality/visual feedback 均 pass。证据见
> [`PREVIEW-STREET-CAMERA-FIX-20261021.zh-CN.md`](PREVIEW-STREET-CAMERA-FIX-20261021.zh-CN.md)。

> **2026-10-21 市场失败具名**：启用市场的场景现在输出
> `market-materialization-report.json`。首版控制保留 3 个市场；后续占地
> 去重叠和毫米标高修复使控制提升到 4 个市场，失败市场仍具名，不再静默
> 消失。市场专用台阶/台地入口仍未完成，详见
> [`MARKET-MATERIALIZATION-AUDIT-20261021.zh-CN.md`](MARKET-MATERIALIZATION-AUDIT-20261021.zh-CN.md)。

> **2026-10-21 分时代需求预留**：`settlementEvolution.districtDemand`
> 已接入生产路径。旧城、更新带、新区和外围先按需求份额消费，首轮未被
> 本阶段地块使用的预算才进入全局回填；建筑数量仍是地块容量的结果。
> 1km 控制已实际产生旧城窄街宅、更新带公寓/街面和新区中层，
> 住宅/岗位/商业/服务需求闭合，`scale-audit pass`、`quality-report pass`。
> 证据见
> [`DISTRICT-DEMAND-ALLOCATION-20261021.zh-CN.md`](DISTRICT-DEMAND-ALLOCATION-20261021.zh-CN.md)。

> **2026-10-21 V3 审核通过并进入实现**：项目负责人已确认自然基线、取消普通
> 建筑固定完成数、地形驱动建筑尺寸、新旧城 epoch、网络中心、分级市场、
> 48 城特征复刻和宏观工程弱门限。已落地自然基线冻结/最终差分、自然节点
> 多路网、建筑原型、分级市场实体和入口自身碰撞修复。当前 1km 候选为
> `examples/natural-evolution-core-city-1km.manifest.json`，实烘焙输出
> `bevy/work/rk-x/v3-w1w5-natural-evolution-1km-final16/`；详细证据与剩余
> 入口缺口见
> [`NATURAL-EVOLUTION-W1-W5-20261021.zh-CN.md`](NATURAL-EVOLUTION-W1-W5-20261021.zh-CN.md)。

> 生成于本轮会话结束时。**基线需求**始终是
> [`requirements-and-progress-master.zh-CN.md`](requirements-and-progress-master.zh-CN.md)，
> 本文件是它的"当前状态快照 + 待办 + 操作手册"，不替代基线。

> **2026-10-21 追加**：五项可见问题的控制场景收口、白色伪影根因、
> 现代建筑/道路/树木/水体证据见
> [`FIVE-ISSUES-CLOSURE-20261021.zh-CN.md`](FIVE-ISSUES-CLOSURE-20261021.zh-CN.md)。
> 现代湖滨控制当前为 `scale-audit pass`、`road-network-audit pass`、
> `quality-report warn`（剩余 966m² 庭院坡地未实现面）。

> **2026-10-21 迭代城区追加**：first vertical slice of
> established/renewal/new district growth is implemented. See
> [`ITERATIVE-DISTRICTS-20261021.zh-CN.md`](ITERATIVE-DISTRICTS-20261021.zh-CN.md).

> **2026-10-21 审核稿**：用户要求重新按“纯自然 → 地形道路 → 聚落 → 新老城”
> 设计，当前原型不作为该需求的最终方案。审核文档：
> [`NATURAL-TO-URBAN-REDESIGN-REVIEW-20261021.zh-CN.md`](NATURAL-TO-URBAN-REDESIGN-REVIEW-20261021.zh-CN.md)。

> **2026-10-21 核心城区审核稿 V2**：进一步否决“固定建筑完成数、整城平均
> 数值、硬同心圆和只换材质的新旧城”。新版审核入口：
> [`CORE-CITY-NATURAL-EVOLUTION-PLAN-REVIEW-20261021.zh-CN.md`](CORE-CITY-NATURAL-EVOLUTION-PLAN-REVIEW-20261021.zh-CN.md)。
> 该稿状态为待审核；审核通过前不得把 P0–P7 技术切片当作最终城区方案。

> **2026-10-21 核心城区审核稿 V3**：当前最新审核入口为
> [`CORE-CITY-V3-NATURAL-HUMAN-EVOLUTION-REVIEW-20261021.zh-CN.md`](CORE-CITY-V3-NATURAL-HUMAN-EVOLUTION-REVIEW-20261021.zh-CN.md)。
> 该稿要求先冻结纯自然基线，再由自然节点生成地形成本路径与聚落，随后按
> epoch 生长旧城/更新/新区/郊区/旅游/农牧区；普通建筑数量、尺寸和用途必须由
> 需求、街区—地块容量、建筑原型及入口条件产生，菜场/超市/周期/批发市场
> 使用分层网络模型。审核通过前不要继续扩大普通城区实现。

> **2026-10-21 W0 证据矩阵**：48 个代表核心切面均已有公开来源入口，
> 全部 48 个完成首轮约 1km² 的 OSM 观测，并已登记证据覆盖状态；
> 没有案例达到完整 `measured-core-window`。可驾驶中心线密度中位数
> 20.31 km/km²（12.10–36.70），建筑覆盖率中位数 0.3165（0.022–0.569）。
> 42 个窗口完成市场型设施步行网络校准，中位时距 4.40 分钟，10 分钟覆盖
> 中位数 96.1%；缺失设施标注的窗口不得判定为零供给。见
> [`URBAN-CASE-EVIDENCE-W0-20261021.zh-CN.md`](URBAN-CASE-EVIDENCE-W0-20261021.zh-CN.md)
> 与 `scripts/check-urban-case-evidence.mjs`。
> 历史阶段统计见 `assets/data/urban-history-patterns.json`：
> 40/48 首段为水港/河湖/治水，26/48 中段出现铁路/轨道，26/48 第三段出现
> 更新/保护/重建；该统计仍待逐案史料核实。
> 土地压力分组见 `assets/data/urban-pressure-patterns.json`：city 地形稀缺
> 中位 1.42、megacity 1.38、town 样本 1.72，说明规模与地形稀缺不能合并。
> 核心库基线 `glyphweave-core --lib` 实测 854 passed / 0 failed / 34 ignored，
> 耗时 478.09s；ignored 均需外部已发布世界或显式单次实验。

> **2026-10-21 V2 生成链接入**：自然地理草图派生、48 城 v2 先验、切面关系
> 语义、网络规划史、土地压力系数以及道路/建筑最终实体化已落地。核心切面
> `derived-core-city-slice-1km` 通过 scale/quality，147 道路段、86 段内部道路
> 拓宽、1 个公园、1 个混合中心广场、6 栋最终建筑、6/6 入口连通、断点 0；
> 2 栋入口失败建筑和 1 座金融中心塔具名进入 materialization report。
> 证据见
> [`CORE-CITY-PIPELINE-V2-20261021.zh-CN.md`](CORE-CITY-PIPELINE-V2-20261021.zh-CN.md)。

> **2026-10-21 自然—城市重构状态**：审核稿已完成 48 城核心区机制、自然地理、
> 规划史、大都市切面、土地紧张和生鲜市场辐射模型，并把执行顺序细化为
> P0–P7。P0 纯自然切片已通过零人工痕、工程/道路/开发计划为零、scale/
> quality/visual gate，并用 109 个可比文件复烘焙确认确定性；证据见
> [`NATURAL-ONLY-P0-20261021.zh-CN.md`](NATURAL-ONLY-P0-20261021.zh-CN.md)。
> P1 地形道路切片也已通过地形搜索、曲线平滑、8% 纵坡、6m 填方和
> bridge/viaduct 显式记录，见
> [`ROUTE-EVOLUTION-P1-20261021.zh-CN.md`](ROUTE-EVOLUTION-P1-20261021.zh-CN.md)；
> quality 因道路条带和植被仍为占位视觉保持 warn。
> P3 容量规划切片由需求与地块容量产出 15 栋建筑，不是固定 count；见
> [`SETTLEMENT-CAPACITY-P3-20261021.zh-CN.md`](SETTLEMENT-CAPACITY-P3-20261021.zh-CN.md)。
> 该计划尚未转回最终 AccessGraph 实体。
> P5 市场网络切片已用 280 容量市场覆盖 260 需求、15 个需求点零缺口，见
> [`MARKET-CATCHMENT-P5-20261021.zh-CN.md`](MARKET-CATCHMENT-P5-20261021.zh-CN.md)。
> P4 静态规划史切片含 5 个历史基座、3 modify、2 demolish、2 带父 ID
> replace，见
> [`EPOCH-EVOLUTION-P4-20261021.zh-CN.md`](EPOCH-EVOLUTION-P4-20261021.zh-CN.md)。
> P6 宏观工程预算切片含 2 路堤 + 3 高架，工程系数 0.9515，局部硬合同
> 不变，见
> [`ENGINEERING-SCALE-P6-20261021.zh-CN.md`](ENGINEERING-SCALE-P6-20261021.zh-CN.md)。
> P7 单地理完整链控制已通过，见
> [`FULL-CHAIN-P7-20261021.zh-CN.md`](FULL-CHAIN-P7-20261021.zh-CN.md)。
> 48 城全量机器数据/烘焙、P3 最终 AccessGraph 接入和统一工程账本仍未完成；
> 审核稿 §20 是当前状态准绳，不能把旧 B/C 完成标志当成新要求完成。

## 0. 2026-10-20 最终快照（本文件以下内容是上一轮历史）

| 项 | 当前值 |
|---|---|
| 分支 / HEAD | `refactor/authoritative-baked-audit` / `a097e36`，已推送 |
| A | 最大图 complete-ground：240 chunks / 623 entities / **gate pass 0 failures**；第二次烘焙 worldRevision 相同，1777 个数据文件逐字节相同；第二次完整 workspace 测试 **exit 0**（core 805 passed / 34 ignored） |
| B | 512m 供应控制 **24/24 沿街 realized-with-access + collector/local 实网 + coverage pass**；joint-plains **18/18 住宅 + 4/4 服务 + gate pass**；B-03 最大图 50,538,180m² ground + ledger 60,000,000/0；B-05 最大图两次确定性 pass |
| C | C-N1 来源矩阵 + habitat 接口；C-E1 最大图 **1,734 挡墙**；C-E2 **734 改向节点 / 54 raised terminals**；最大图工程版 **gate pass**；C-L1 两项来源已解析；Godot 4.7.2 runtime smoke pass |
| 预览 | 战略图/2.5D 已实测；2.5D 加载时显示 loading overlay；最大图全战略图无 console error，非港场景 2.5D 改为内容 focus，主画面无黑洞/错觉边界 |
| 仍开放 | 偶发 `0xC0000005`；1km/120 户属于输入供给压力；Quaternius 精确历史下载日期未知 |

完整证据入口：

- [`ABC-STATUS-20261020.zh-CN.md`](ABC-STATUS-20261020.zh-CN.md)
- [`B01-JOINT-PLAINS-COMPLETE-20261020.zh-CN.md`](B01-JOINT-PLAINS-COMPLETE-20261020.zh-CN.md)
- [`B02-TIER-PROVENANCE-20261020.zh-CN.md`](B02-TIER-PROVENANCE-20261020.zh-CN.md)
- [`B04-SERVICE-COVERAGE-20261020.zh-CN.md`](B04-SERVICE-COVERAGE-20261020.zh-CN.md)
- [`B05-MAXIMUM-COMPLETE-20261020.zh-CN.md`](B05-MAXIMUM-COMPLETE-20261020.zh-CN.md)
- [`C01-ENGINEERING-20261020.zh-CN.md`](C01-ENGINEERING-20261020.zh-CN.md)
- [`C02-MAXIMUM-ENGINEERING-20261020.zh-CN.md`](C02-MAXIMUM-ENGINEERING-20261020.zh-CN.md)
- [`C03-GODOT-SMOKE-20261020.zh-CN.md`](C03-GODOT-SMOKE-20261020.zh-CN.md)
- [`HTML-PREVIEW-SMOKE-20261020.zh-CN.md`](HTML-PREVIEW-SMOKE-20261020.zh-CN.md)

## 1. 状态快照

| 项 | 值 |
|---|---|
| 分支 | `refactor/authoritative-baked-audit` |
| HEAD | `74cdc1a` + 2026-10-20 A-closure 证据提交 |
| 工作树 | **干净**（`git status --porcelain` 为空） |
| 本会话提交 | **50 次**（`0edabb3..HEAD`），全部通过一次性 `git commit -F`，无 merge |
| 仓库总提交 | 292 |
| 测试规模 | 整包 **26 条 `test result` 全 ok**；core lib **781 passed / 34 ignored** |
| 偶发缺陷 | 整包测试约 **1/27** 概率 `0xC0000005`（见 §6） |

## 2. 本会话做成了什么（按主题）

### 2.1 供给与地形工程（B-01 主干）

| 能力 | 开关 | 关键数字 |
|---|---|---|
| 沿路用途转换（rural→urban） | `frontageProgram.ruralConversion = "explicit"` | 1km 曾把临街实现从 1 提到 67 |
| 有界挖方（地块 + 道路） | `frontageProgram.terrainPolicy = "bounded-cut"` + `maxCutMm ≤ 2000` | 1km 削方台账 27,178 格 / 6,598 m³，与栅格复算逐格相等 |
| 半挖半填纵断面 | 同上 | 山城道路削方 675 格 / 150.7 m³，半挖半填站 12% |
| 填方环按填深推导 | `terrainPolicy.earthworkBlend = "derived-from-fill"` | 最大环 126 m → 19.6 m，环包络缩 29 倍 |
| 台地有界收敛 | 随 bounded-cut | 实体平台抬升可在 2 m cap 内加深，否则具名拒绝 |
| 分级入口走廊 | `terrainPolicy.entryCorridor = "graded"` | 能力已接三条 gate 路径，**真实世界 0 次接受**（见 §5） |

**不可动清单（本会话一次没破）**：6 m 填方、8% 道路纵坡、25% 可走入坡、2 m 引道、
退界域 8..=32、岸线净距 4 m、保护区/水体/岸线带零土方、`MAX_CUT_MM = 2000`、
两条链金标 `c3_base_identity_*` / `w4_chain_digest_*`。

### 2.2 滨水（新能力）

- `automaticRoadExtensions.allowStructuredWater = "bridge-or-causeway"`：滨水可跑 road-first
- 跨水必须登记桥/堤（长度、湿格、桥面、起坡点），**水面逐格零变化**
- 滨水地块站在**自己路面的高程**上；岸线保留 **4,096 m² 连续公共通行带**（4 m = 既有审计净距）
- 滨水商业排在**离岸 18–20 m 的第一排街**（原 150 m 硬编码已退役）
- `river` 有了 `assets/objects/river.object.toml`，规则引擎真正审它（425/425）

### 2.3 分区与形态（B-03/B-04）

- `landUseProfile.urbanGrowth = "axis-and-node"`：城市用地沿**主干路轴（80 m）+ 600 m 生活圈**生长，
  面积**精确等于**径向预算（40,898 m² 不变），urban 区内建筑占比 13.3% → 37%
- `morphology_compile`：`frontInsetM` 派生 `setbackM`；`publicSpaceRatio` 单独就能生成公共空间
  （512m 夹具 0 → 16,387 m²，账本守恒）
- 审计可信度：修掉 4 处"测量口径"缺陷（编译态 style、候选顺序、削方 stamp、off 臂未真关）

### 2.4 内容与账本

- `developmentAudit` 区分"地块产物"与"规则生成建筑"（`ruleBuildingGenerated` 计数 + 可伪造检测）
- 用途预算封顶：转换按 `planned − committed − residual` 花钱，超额具名拒绝
- 转换只允许发生在**城市边缘 64 m 带内**（`FRINGE_M`，1km 上转换 24.4% → 100% 在带内）
- 生态配额：短少 patch 在确定性晶格上**重排**，达标 patch 永不触碰（A 例外 #4 的修复）

### 2.5 性能

- 接受期接入检查：carve **32 m 均匀桶索引** + 窗口 memo + 早退
- 1km bake 199 → 124 s，gate 115 → 68 s，整包 1966 → 1371 s，**判定零改动**（逐字节验证）

## 3. 三个固定世界（可直接复现）

| 世界 | 清单 | 关键数字 | 复现方式 |
|---|---|---|---|
| **1 km 探针** | `examples/b01-1km-probe-optin.manifest.json`（+137 字节、4 个开关） | 43 pad / 15 户 / 24 店；接入失败 0；gate 7 条；release bake 17–18 s | `#[ignore]` 测试 `the_optin_manifest_replays_the_probes_published_numbers`（`GLYPHWEAVE_AUDIT_WORLD` 可跳过烘焙） |
| **山城** | `examples/hillside-town-512.manifest.json` | 22 pad / 15 户 / 6 店；削方 7 pad、道路 675 格；dev 失败 0；gate 4 条 | `cargo test -p glyphweave-cli --test hillside_cut_city`（4 tests） |
| **滨水** | `examples/waterfront-city-512.manifest.json` | 28 pad / 22 户 / 4 设施；60 路 / 7 桥 / 273 m 跨水；岸线带 4,096 m²；gate 3 条 | `cargo test -p glyphweave-cli --test waterfront_city_512`（1 test） |

1km 的关闭态基线：`examples/b01-1km-probe.manifest.json`（无开关）；两者**除四个开关外逐字节相同**（有测试钉住）。

## 4. 常用命令

```powershell
# 全量（约 23–26 分钟，偶发崩溃见 §6）
cargo test --manifest-path bevy/Cargo.toml --workspace --no-fail-fast

# 单世界烘焙 + 质量门
cargo run --manifest-path bevy/Cargo.toml -p glyphweave-cli --release -- generate-world <manifest> <out>
cargo run --manifest-path bevy/Cargo.toml -p glyphweave-cli --release -- quality-report <out>
cargo run --manifest-path bevy/Cargo.toml -p glyphweave-cli --release -- world-quality-gate <out>   # 非 0 退出表示 gate fail

# 只看某个夹具
cargo test --manifest-path bevy/Cargo.toml -p glyphweave-cli --test hillside_cut_city
```

## 5. 仍存在的问题（按优先级）

| # | 问题 | 现状 | 需要什么 |
|---|---|---|---|
| 1 | **A 例外 #4（生态缺 1 株）已关闭** | 2026-10-20 6×10km A-closure：**255.941s、240 chunks、623 entities、342,380/342,380/0**；gate 仍 fail 4 项，均为 land-use | 保留证据；land-use 缺口转 B-03/B-05 |
| 2 | 1km 住宅 15/18（整体差 3；joint recovery 6/18，差 12） | `urbanCoreRatio 0.02`（规划城市地 40,898 m² ≈ 4%）已耗尽 | 提高该比例（作者输入），或加相邻台面高差合同 |
| 3 | 相邻台面高差无上限 | 两块 24×24 相距 32 m、台面差 2.3 m 时，各自环之间必成谷/崖 | 新增合同（"相邻台面 ≤ X m"）或挡墙 |
| 4 | 分级走廊 0 次接受 | 1km 三次尝试全撤回：checked route 落在更近的街 / 天然坡 31.6% | 选路规则优先指派街；或允许走廊削坡 |
| 5 | 山城临街 7/24 | 已证明是**输入上限**（24 需求 vs standalone 上限 10；21/33 条路无纵断面） | 作者改输入（更高需求→更多街道/更低住宅最小宽） |
| 6 | 滨水 3 条 land-use 欠账 | frontage demand / purpose-area / landcover | 内容目标问题，非缺陷 |
| 7 | 偶发 `0xC0000005`（~1/27） | 已解析到 road-network JSON serde 派生序列化；WER dump 显示读地址 −1；仓库零 `unsafe` ⇒ 疑依赖或编译器；本次 A-closure 单次烘焙成功不等于崩溃已修 | 整包循环取证（`tools/crash-hunt/`） |
| 8 | C 已实施但仍有工程深化项 | C-N1 来源矩阵、C-E1 挡墙/涵洞、C-E2 工程后路由、C-L1 来源解析、C-V1/G1 runtime 与最大图工程 gate 均有实现；现代控制已显示树/湖/桥 | 挡墙基础、排水沟、竖曲线、洪泛/水量模型仍开放，不属于“C 尚未开始” |

## 6. 偶发崩溃的操作纪律（重要）

整包测试约 **1/27** 概率以 `0xC0000005` 崩溃，**不是**本轮引入：

- 栈深实测峰值 192–224 KiB（默认 2 MiB，余量 8×），**不是深栈**
- 崩溃点在 `road-network-plan` 的 serde 序列化
- 证据与工具：`docs/collaboration/rk-x/KNOWN-ISSUE-intermittent-crash.zh-CN.md`、`tools/crash-hunt/`

**纪律**：任何"套件全绿"的结论应以**多次连续运行**为准；崩溃时重跑并在报告里注明。

## 7. 文档索引（本会话新增/更新）

| 文件 | 内容 |
|---|---|
| `docs/collaboration/requirements-and-progress-master.zh-CN.md` | 基线账本（§5 进度、§6 七条待决策项，已追平到 43 次提交时） |
| `docs/collaboration/rk-x/ROUND-LOG.zh-CN.md` | 逐轮证据日志（记到 Round 12；之后证据在提交正文与测试常量里） |
| `docs/collaboration/rk-x/A-EXCEPTION-4-ECOLOGY.zh-CN.md` | 生态例外的证据状态与关闭记录 |
| `docs/collaboration/rk-x/A-REBASELINE-20261020.zh-CN.md` | A-closure 6×10km 完整命令、数字、gate 与未验证项 |
| `docs/collaboration/rk-x/KNOWN-ISSUE-intermittent-crash.zh-CN.md` | 偶发崩溃的已知事实、工具、下一步 |
| `docs/collaboration/rk-x/DECISION-COST-SHEET.zh-CN.md` | 六项"需授权"改动的代价表 |
| `tools/crash-hunt/` | 崩溃取证脚本（dump 捕获、偏移符号化、循环复现） |

## 8. 交接给下一位的前三件事

1. **跑一次基线确认**：`cargo test --workspace --no-fail-fast`（崩溃则重跑）+ 三个夹具测试，
   确认 §3 的数字仍成立。
2. **A 例外 #4 已关闭**：保留 A-closure 证据；下一步按 B-02 → B-03 修复真实
   collector/local 拓扑与 123,760m² 未实现，再进 B-04/B-05。
3. **B 接口稳定后再做 C**：先做"台地挡墙"（`terrainPolicy.retainingWalls`，opt-in，
   高差 ≥ 阈值才生成、占地入账本、审计可复算），再做排水/涵洞/竖曲线。

## 9. 注意事项（别踩）

- **不要**改两条链金标、`WALKABLE_GRADE`、`ENTRANCE_APRON_RUN_M`、审计容差、
  `MAX_CUT_MM`、`FRINGE_M`、`development.rs` 的判定，除非有明确授权。
- **不要**用 `RUST_MIN_STACK`、`catch_unwind`、放宽断言去"修"偶发崩溃。
- **不要**为了让数字好看而挑 seed；seed 列表要事先固定并全部报告。
- 数值改动一律走 **opt-in + 关闭态逐字节冻结**（本会话所有能力都是这个模式）。
- 提交用 `git commit -F <文件>`（PowerShell 多行 `-m` 会失败）；`bevy/work/` 在 gitignore 里。
