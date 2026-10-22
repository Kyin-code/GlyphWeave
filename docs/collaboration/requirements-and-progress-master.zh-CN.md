# GlyphWeave 需求与进度总表（master）

生成日期：2026-10-18；最近更新：2026-10-23（核心密度、道路碰撞、建筑朝向与入口复核）。性质：**需求台账 + 进度台账**，不是验收裁定，不替代任何合同文件。
读者：项目负责人、接手的主 Agent、实现 Agent、独立检查 Agent。

> **2026-10-23 道路纵断面/跨线/碰撞复核**：r169 修复了道路设计面与最终
> Heightfield 脱链的问题。r155 曾有 2,451/2,588 个中心线采样低于地面、
> 最大埋深 14.716m；现在任意朝向分段发布 `routeGrade`，Rust 同步执行道路
> 走廊挖填，冲突交叉与共线高差自动发布为合并后的 `overpass`，低位道路保留
> 自己的地形和独立 OBB 碰撞链。r169 为 4,462 实体，`scale-audit=pass`；
> 609 个地面道路采样中位数偏差 -0.0004m，最大埋深降至 2.817m，超过
> 0.75m 的剩余采样 16 个；41 个合并跨线 span。规划历史的 8m→32m 隐式
> 单侧拓宽已停用，避免实体宽度/路线宽度/可见横断面分裂。详见
> [`ROAD-GRADE-COLLISION-20261023.zh-CN.md`](rk-x/ROAD-GRADE-COLLISION-20261023.zh-CN.md)。

> **2026-10-23 建筑体量与可见碰撞**：r178 普通建筑 373–374 栋，接近
> 48 城核心样本中位数 410；中心 150m 为 71 栋，约为样本均匀折算的 2.4 倍。
> 当前主要短板转为单体 footprint（中位约 93m²），不再把“只增加小盒子”
> 当作密度修复。低层住宅已取消固定 2×3 微型房屋切分，改为按 6/12/20m
> 自适应主体数量；坡地基础和年代基色不再越出 Rust OBB，接触阴影收紧到
> 1.05 倍。组合地块放大实验会降低有效建筑数并制造断连入口，已撤回为显式
> 后续任务。`scale-audit=pass`，`quality-report=warn`，仍有 30 栋入口全断。
> 详见
> [`BUILDING-MASSING-VISUAL-20261023.zh-CN.md`](rk-x/BUILDING-MASSING-VISUAL-20261023.zh-CN.md)。

> **2026-10-23 核心密度/道路碰撞复核**：统一了 Rust OBB、入口逆变换和
> HTML Three.js 的正 Y 旋转；建筑 root 保留权威 yaw，街面朝向只转动子节点。
> 道路横断面改为同一设计标高，快慢路断面由宽度和等级发布，并保留独立
> 碰撞链；建筑用途恢复办公、店面、混合住宅、市场的确定性混合。r155 为
> 4,371 实体、335 最终建筑、309 入口楼梯、34,811.2m² footprint、
> 1,475 道路段、393 树；
> 中心 150m 为 62 栋。`scale-audit`、最终实体几何审计和市场辐射通过；
> 所有建筑碰撞、悬空、穿地、落水均为 0，跨 Chunk 最大高差 2.834m；
> 人行道/隔离带已在交叉口内裁断，交叉口增加封闭盖面，街景焦点开始使用
> 道路 OBB 距离，建筑/市场 approach 已改画为 4m 步行铺装。
> `quality-report=warn`：仍有 23 栋建筑没有严格可走入口；单体体量、
> 路口边缘模板和建筑连接仍未完成。参考审计见
> [`REFERENCE-AUDIT-20261023.zh-CN.md`](rk-x/REFERENCE-AUDIT-20261023.zh-CN.md)，
> 实现状态见
> [`URBAN-DENSITY-ROAD-COLLISION-20261023.zh-CN.md`](rk-x/URBAN-DENSITY-ROAD-COLLISION-20261023.zh-CN.md)。

> **2026-10-22 城区密度/道路/立面复核**：已按每条路线自己的宽度释放街巷
> 临街空间，取消“全城只消费一条旧城街墙”，补齐道路车道断面、显式朝向、
> 市场入口步道与最终草木碰撞检查。香港 1km r65 为 3,293 实体、
> 140 最终建筑、17,767m² footprint、686 道路段、434 树；中心 150m
> 半径 46 栋，整体由 r28 的 53 栋增至 140 栋。6/6 市场物化，
> 10 分钟覆盖 93.46%；建筑—建筑、建筑—道路、建筑—草木、悬空、穿地、
> 落水均为 0；`scale-audit` 与 `quality-report` 均为 pass。详见
> [`DENSITY-ROADS-FACADES-20261022.zh-CN.md`](rk-x/DENSITY-ROADS-FACADES-20261022.zh-CN.md)。

## 0. 怎么用这份文档

> **2026-10-21 48 城完整性审计**：`pnpm check:urban-cases` 已把自然地理、
> 三阶段历史、城市四层结构、土地压力、来源和测量窗口纳入机器门禁。当前
> 48/48 字段与证据入口完整；自然地理 48/48 仍为 `mechanism-prior`，
> 尚未逐案升级到 `source-verified`。证据见
> `docs/collaboration/rk-x/URBAN-CASE-COMPLETENESS-AUDIT-20261021.zh-CN.md`。

> **2026-10-21 香港中环机制切面**：首个“自然山体 → 旧市场核 → 更新台地 →
> 新区 → 填海金融水岸”的 1km 特征切面已实烘焙。r1w 需求为 900 户、
> 2,600 岗位、20,000m² 商业；35 个规划建筑中 34 个通过严格入口并物化，
> 另有 23 个短梯 + 3 个长直梯、6 个分级市场，AccessGraph 41 连通/0 断点。
> 市场现在避让已规划建筑，
> 办公/住宅补齐地形 support carve，入口锚点改为立面中点，并加入 2m 门前
> landing、路口建筑标高同步、门区障碍清理和完整 world skyline/waterfront
> 预设。scale、quality、visual feedback 均 pass。直梯落地后拒绝入口从
> 7 降至 1 且具名；市场计划已从 35 栋规划需求重算为最终 34 栋实际需求，
> 原计划与最终计划
> 分开保存。证据见
> `docs/collaboration/rk-x/HONG-KONG-CENTRAL-MECHANISM-CUT-20261021.zh-CN.md`。

> **2026-10-21 V3 审核已通过**：自然先行、普通建筑取消固定完成数、
> 地形驱动建筑尺寸、新旧城 epoch、网络中心、分级市场、48 城代表切面、
> 以及“普通局部弱门限 + 特征地区宏观工程授权”的原则均已确认。
> W1/W2/W4/W5 第一版实烘焙证据见
> `docs/collaboration/rk-x/NATURAL-EVOLUTION-W1-W5-20261021.zh-CN.md`。

> **2026-10-21 分时代需求预留**：`settlementEvolution.districtDemand`
> 已在首轮按旧城、更新带、新区、外围分配住宅/岗位/商业/服务预算，未使用
> 预算才进入全局回填。1km 控制已实际产生三类时代原型并保持需求闭合，
> `scale-audit pass`、`quality-report pass`；证据见
> `docs/collaboration/rk-x/DISTRICT-DEMAND-ALLOCATION-20261021.zh-CN.md`。
> 入口台阶/坡道工程、市场完整入口和街区级拆合仍是开放项。

> **2026-10-21 HTML 街景运行态修复**：修正街景 road wrapper 导致的相机
> NaN、routeFocus 覆盖街景点、approach 误选、店铺朝向后被通用 yaw
> 覆盖和办公塔 fallback。香港 r6 控制 3,011 实体、41 连通/0 断点，
> scale pass；战略/街景/天际线均非空，但 `visual-feedback=warn`：
> 前景留白、立面材质偏简、战略图仍偏 3D 沙盘。证据见
> `docs/collaboration/rk-x/HTML-STREET-CAMERA-FACADE-20261021.zh-CN.md`。

> **2026-10-21 市场落地审计**：`market-materialization-report.json` 已把
> 计划市场、最终保留数、失败市场 ID、位置、容量和 AccessGraph 状态固化。
> 首版控制保留 3 个；后续占地去重叠与毫米标高修复后保留 4 个。市场专用
> 入口台阶/台地工程仍开放。证据见
> `docs/collaboration/rk-x/MARKET-MATERIALIZATION-AUDIT-20261021.zh-CN.md`。

> **2026-10-21 街景空屏修复**：street 2.5D 焦点由边缘单店改为城区实体
> 质心，并修复 renderer/camera/postFX resize。r6 运行态战略图与 street
> 均可见，AccessGraph 断点 0，`visual-feedback.json=pass`，scale/quality
> 均 pass。证据见
> `docs/collaboration/rk-x/PREVIEW-STREET-CAMERA-FIX-20261021.zh-CN.md`。

> **2026-10-21 坡地入口台地**：聚落建筑改用道路设计标高作为 pad 目标，
> 并以 `bottomOffsetM` 保留毫米地面；控制切面启用 `maxPadCutM=12`。
> 1km 最终普通建筑 12→16、市场 3→4、AccessGraph 20 连通/0 断点，
> scale/quality/visual feedback 全部 pass。仍余 8 个真实坡地入口待
> stairs/switchback 工程，均具名。证据见
> `docs/collaboration/rk-x/TERRACED-ENTRY-ENGINEERING-20261021.zh-CN.md`。

> **2026-10-21 市场占地去重叠**：市场候选按实际建筑 footprint 检查，并保留
> 2m 净距；重叠候选改选下一节点。1km 控制最终 4 个市场，`market.4` 从
> `(374,346)` 改选到 `(378,506)`，AccessGraph 20 连通/0 断点，
> scale/quality/visual 均 pass。见
> `docs/collaboration/rk-x/MARKET-FOOTPRINT-OCCUPANCY-20261021.zh-CN.md`。

> **2026-10-21 已知道路缺陷**：自然道路存在大量 45°/135° 斜向段，当前
> Core 已改为任意角度道路局部矩形 carve，并同步 carve 索引、bounded-cut、
> support/transport 和桥面审计；0°/90° 合同保持原边界。香港 1km 重烘和
> 视觉复核已完成：r3c 为 3,010 实体、34/35 建筑、6 市场、25 楼梯、
> AccessGraph 41/0，scale/quality pass；街景相机已沿真实道路取景。
> 城市密度/材质仍为 visual `warn`。详见
> `docs/collaboration/rk-x/ORIENTED-ROAD-CARVE-20261021.zh-CN.md`。

> **2026-10-21 新城/旧城实体继承**：聚落建筑物化时已把
> `districtStage` 写入最终 `districtId` 与 `constructionCohort`。香港 r3d
> 的 34 栋实体为 old-core 10、renewal 15、new-growth 9，periphery 0；
> scale/quality pass。详见
> `docs/collaboration/rk-x/DISTRICT-ENTITY-PROVENANCE-20261021.zh-CN.md`。

> **2026-10-21 工程账本统一**：bounded-cut 现在覆盖门前 landing、非 frontage
> 弱削方建筑与最终道路实体；道路 shoulder 方向错误已修复。6 个旧失败中 5 个
> 关闭，R31 1km 重烘成功且 scale-audit pass。剩余 1 个 frontage diagnosis
> 拒绝原因分类差异已具名。详见
> `docs/collaboration/rk-x/ENGINEERING-LEDGER-UNIFICATION-20261021.zh-CN.md`。

> **2026-10-21 沿街跨阶段重放修复**：frontage 在正式生成中先于
> `plan.roadCutMm` 发布执行。只读诊断与离线重放现会恢复这一前置快照，
> 补齐候选提交的 `naturalMinM/naturalMaxM` 采样，并统一按
> `profile_roads` 顺序构造既有工程集。唯一 Core 回归已关闭，未合并拒绝原因
> 或放宽门限。1km 控制为 89 实体、4 户/16 店，AccessGraph 31/0，
> scale-audit 与 road-network-audit 通过；quality 仍因旧探针缺地标和完整
> 土地实现为 warn。证据见
> `docs/collaboration/rk-x/FRONTAGE-STAGE-REPLAY-20261021.zh-CN.md`。

> **2026-10-21 新旧城连续街墙**：香港 1km 控制不再把旧城预算分散到
> 两条街侧的孤点。生成器按时代预算优先消费同一街侧，并拒绝单栋旧城孤点；
> 旧城 7 栋形成连续街墙，覆盖率 0.771、最大缺口 4.37m。最终实体写入
> `buildingOperation/buildingParentId`，renewal 19 栋全部为带父对象的
> replace，new-growth 7 栋为 new。需求仍闭合，AccessGraph 38/0，
> scale、quality、visual feedback 均 pass。证据见
> `docs/collaboration/rk-x/EPOCH-STREETWALL-20261021.zh-CN.md`。

> **2026-10-21 48 城先验正式消费**：`referenceData.caseIds[0]` 现在会在
> morphology 编译前自动补齐自然地理、feature-cut、社区类型和案例历史记录；
> 未手写 `districtDemand` 时还会从三段历史推导四类时代预算份额，并标记为
> 启发式；审计字段已能被 `settlementEvolution` 严格消费。手写字段保留优先。
> 未知 case id 失败关闭，旧 Manifest 无 caseIds 时保持原语义。证据见
> `docs/collaboration/rk-x/URBAN-CASE-PRIOR-COMPOSITION-20261021.zh-CN.md`。

> **2026-10-21 切面焦点进入布局**：`urbanSlice` 的锚点、特殊因子和
> `focusFeature` 现在编译为有限、可审计的 `focusBias`。推断时代预算、中心
> 角色优先级、道路拓宽、公园需求和宏观工程预算都会随焦点改变；手写预算与
> 局部地形/入口合同不变。特区、填海、造岛、山地和中央规划切面可进入无绝对
> 土方上限的规划授权模式，但每项工程仍必须经过预算与工程审计。证据见
> `docs/collaboration/rk-x/FOCUS-DRIVEN-PLANNING-20261021.zh-CN.md`。
> 香港 r2 实烘焙 2,998 实体、39 入口连通/0 断点、5 市场、497 树，
> scale/quality 均 pass；作者手写 7/19/7/0 时代预算保持不变。

> **2026-10-22 最终实体硬约束回归已闭合**：旋转 AABB、真实 OBB、
> 道路拓宽后净空、建筑台地 leveling、入口旋转约定、市场物化位置和
> `final_scene_audit` 已统一。香港 r26 为 2,901 实体、53/53 入口连通、
> 5 市场/99.80% 覆盖，建筑—建筑、建筑—主道路、悬空、穿地、落水均为 0；
> 街景默认加载邻接 Chunk，并按规划 polyline 绘制连续路面/人行道，消除了
> 单 Chunk 截断和 32m 路段重复叠绘。边界最大高差 0.957m，
> scale/quality/visual feedback 均 pass。证据见
> `docs/collaboration/rk-x/HARD-OCCUPANCY-GROUNDING-REGRESSION-20261022.zh-CN.md`。
>
> 复核同时修复了 `quality-report` 的有效程序重编译输入：生成与审计现在都
> 使用 case-composed style，避免带 `urbanCase`/社区模板的合法世界被自审
> 误判。

- **要"项目到底要做什么"** → 读第 4 章（需求域表，19 个域，逐条带来源与状态）。
- **要"现在做到哪了"** → 读第 5 章（进度）与第 6 章（当前卡点）。
- **要"下一步该改哪里"** → 读第 7 章（缺口与优先级）、第 8 章（需要人拍板的决策）。
- **要证据** → 第 9 章给路径；验收与检验文档已集中到 `docs/collaboration/acceptance/`。

**口径声明（重要）**

1. 本表的状态判定依据**文档所述的证据层级**（见 2.2），除特别标注外，我**没有**独立重跑世界、gate 或测试（例外：Round 51 的 1 km 复现，见 5.4 读法 3）。
2. 数字一律抄自原文，不做单位换算、不跨口径相加。凡原文未给的，写"文中未说明"。
3. 需求条目保留原始文档的编号（R01–R16、A-01–A-10、B-G1–B-G8、C-N1–C-G1、FUT-01–06）；
   我新增的域编号 `D1.1` 只用于本文导航，不是新的验收 ID。
4. 本文不发明"完成度百分比"。用状态词（2.1）表达，避免把测试数/文档数当进度。

---

## 1. 需求谱系（按时间，谁定义了什么）

项目实际有**两条源头**，同在 2026-08-28，一条成为主线、一条被搁置：

| 日期 | 文档 | 性质 | 当前位置 |
|---|---|---|---|
| 2026-07-07 / 07-11 | `docs/superpowers/plans/` 与 `docs/superpowers/specs/` 下的 `2026-07-07-bevy-p1-foundation*`、`2026-07-11-gemap-v3-voxel-storage.md` | Bevy P1 底座与 gemap v3 体素存储设计 | 已实现，成为 A 的底座 |
| **2026-08-28** | **`docs/game-plan.md`、`docs/game-todo.md`** | **殖民挑战沙盒游戏（Flood Fortress）**：建造/挖壕/防洪/铜银金评级，`GameCommand` 命令层，ASR/本地模型接缝，`pnpm bevy:fps` ≥150 workload FPS | **被搁置**，但代码仍在 `bevy/crates/core/src/gameplay/`（command/sim/state/snapshot/mod） |
| **2026-08-28** | **`docs/glyphweave-world-todo.zh-CN.md`** | **世界生成闭环的产品约束清单**（尺寸/坐标/分块/Manifest/LOD/消费者/资产/多场景/自动验收） | **当前主线的源头** |
| 2026-08-28/29 | `west-lake-scale-audit`、`nanjing-yangtze-bridge-scale-audit`、`modern-mainland-morphology` | 真实尺度核对与形态研究 | 参数来源素材 |
| 2026-08-30/31 | `procedural-world-generator`、`mapgen4-hydrology-plan` | 生成器技术方案；Mapgen4 水文计划 | C-N 的来源候选 |
| 2026-09-01 | `urban-natural-generation-plan`（46KB）、`rules-engine-mvp` | 现代世界总体方案（明确高质量渲染为非目标）；规则引擎 | B/C 的设计母本 |
| 2026-09-02～04 | `20260902-check-1..25`、`20260903-check-28..31`、`20260904-check-32..37`、`20260902-acceptance-list-1` | 早期逐轮自检/验收清单 | 历史证据 |
| 2026-09-03 | `terrain-render-contract`、`terrain-visual-baselines`、`html-rendering-handoff` | 地形渲染合同、视觉基线、HTML 交接 | 消费者层约束 |
| 2026-09-05/06 | `refactor-bake-cli-audit`、`access-v2-acceptance-status`、`access-surface-and-earthwork`、`planning-partition-v2`、`lowland-road-network`、`road-first-development-ownership`、`landcover-candidate-plan`、`yard-surface-realization`、`agriculture-surface-realization` | 工程地形/访问图/规划分区/低地路网/道路先行开发/地表实现 | B 的实现设计层 |
| 2026-09-07 | `r23`–`r28`、`r37`–`r40`、`r44`–`r46`、`r48`、`r49`、`20260906-natural-human-development-roadmap`（75KB） | 逐轮实现记录，**数值合同的主要出处** | 见第 4 章 |
| 2026-09-08 | `milestone-a-agent-task`、`milestone-c-agent-task`、`20260905-modern-world-generation-architecture`、`20260906-human-settlement-morphology-research`、`20260906-natural-terrain-generation-research`、`r41`–`r43`、`r47`、`dual-agent-collaboration`、`release-reproducibility` | 里程碑任务书、架构、形态/自然研究、最大图基线、发布可复现 | 合同与护栏 |
| 2026-09-08→09 | `docs/collaboration/status.zh-CN.md`（735 行调度板）+ `a-*` 系列 | A 的多轮验收 | `acceptance/a-milestone/` |
| 2026-09-09 | `a-main-acceptance-20260909` | **A 正式签收** | 同上 |
| 2026-09-16/17 | `milestone-b-plan`、`milestone-b-acceptance`、`ds-v41f-b01-b05-runbook`、`b01-supply-r1..r4` | B 计划/合同/runbook/供给链 | `acceptance/b01-supply/` |
| 2026-09-20 | `b01-building-street-neighborhood-direction` | 用户补充要求（建筑/街道/新老城/区域工程/双塔/夜市/机场港口等） | `docs/collaboration/` |
| 2026-09-22～28 | `b01-g11`…`b01-g34`、`dsh-b01-g*` | G 轮次（工程连接、预览消费者、HUD、相机、小地图…） | `acceptance/b01-rounds/` |
| 2026-09-29 | `abc-goals-realignment-and-roadmap`（R01–R16）、`abc-follow-up-execution-plan`、`external-review-*`、`project-handoff-abc-b-mainline`、`c-post-c-six-capabilities-plan`（FUT-01–06） | **主线纠偏**：回到 B 生产链；登记六项未来能力；两份外部审查（K3/HY4） | `docs/collaboration/` |
| 2026-09-30 | `dsh-autonomy-charter`、`dsh-b01-t2c-r11-goal-task` | 自主权章程 + 目标式任务（R1–R10 步骤清单作废） | `docs/collaboration/` |
| 2026-10-01/02 | `dsh-b01-t2c-r11-inspector-note-1/-2` | R11 期间两次检查员便条（1km 额度 2/6） | `docs/collaboration/` |
| 2026-10-04～09 | `bevy/work/collaboration/rk/T-0010`…`T-0013` | **T 工单流**：重构后 baked_audit 9 红 → 收敛到 1 红 | `bevy/work/collaboration/rk/` |
| 2026-10-18 | 本文 + `acceptance/` 归档 | 需求/进度总账 | 本文件 |

> 结论：**"最初的需求"是 2026-08-28 三份**。其中 `game-plan`/`game-todo` 的殖民游戏方向已被
> 2026-09-01 现代世界方案取代（`abc-goals-realignment` R05 明确"游戏旧文档不自动覆盖现代世界路线"），
> 但**其代码与 FPS 护栏仍在仓库里**（见 D1.5/D1.6），这是容易被忽略的历史包袱。

---

## 2. 状态口径

### 2.1 状态词（互不替代，按强弱排序）

| 词 | 含义 |
|---|---|
| `未开始` | 只有需求登记 |
| `仅设计` | 有文档/字段设计，无生产代码路径 |
| `字段已接` | 结构体/JSON 有字段，但生成器未真正消费 |
| `模块已接·输入受限` | 有真实代码路径，缺输入或覆盖面窄 |
| `仅 fixture` | 只在测试夹具里通过 |
| `生产可用` | 当前源码在真实输入上跑通该环节 |
| `真实世界通过` | 固定 1km 真实世界的该项审计通过 |
| `跨尺度通过` | 512m + 1km + 6×10km 均有证据 |
| `独立验收通过` | 独立检查者复算输入/hash/结果后裁定通过 |

### 2.2 证据层级（`project-handoff` §10，8 级）

`static` < `compile` < `unit` < `fixture` < `production-world` < `cross-scale` < `consumer` < `independent`。
**高一级不能替代低一级**：compile 不能替代 fixture；fixture 不能替代 production-world；consumer 画出来不能证明语义正确；旧世界不能证明新源码。

### 2.3 三条硬纪律（违反即返工，与功能进度无关）

1. **质量判定必须同时看 exit code 与 JSON `status`**（只读 exit 0 曾误放过 fail）。
2. **数值合同不许调松**：坡度、填方上限、走廊宽度、退界、审计精度、容差、需求数量、zoning、保护区、尺寸、比例、seed。
   判据：改完之后，一个真实缺陷是否更难被发现？是 → 越线。
3. **失败必须具名持久化**：拒绝写进权威产物，不许静默丢弃/静默压平/改名成"有意留白"。

---

## 3. 里程碑与合同矩阵

### 3.1 三个里程碑

| 里程碑 | 一句话 | 合同 | 状态 | 已知例外 |
|---|---|---|---|---|
| **A 稳定底座** | 世界可以难看，但必须可复现、可追溯、消费者不能造物 | A-01～A-10（`acceptance-contract.zh-CN.md`） | ✅ 2026-09-09 `milestone-A-accepted`；`worldQualityStatus=fail` | **六条内容失败**（见 5.3），明文说不许继承到 B/C |
| **B 真实街区/聚落** | 每个实体都能反查到 道路→地块→建筑→入口→账本，且面积守恒 | B-G1～B-G8（`milestone-b-acceptance`） | ✅ 核心里程碑完成 | 512m 24/24 沿街 + collector/local 实网；joint-plains 18/18 + 4/4；最大图两次确定性 gate pass。1km/120 户压力作为输入/预算上限记录 |
| **C 自然—工程—人文** | 自然决定选址，工程反过来改地形水文，消费者读同一版本 | C-N1/N2、C-E1/E2、C-H1/H2、C-L1、C-V1、C-G1 | ✅ 核心里程碑完成 | C-N1 来源矩阵 + habitat 接口、C-E1 挡墙/涵洞、C-E2 工程后路由对比；最大图工程版 gate pass；C-L1 来源解析；Godot 4.7.2 runtime smoke pass |

### 3.2 B 的五个批次（真正的实施里程碑）

| 批次 | 交付 | 前置条件 | 状态 |
|---|---|---|---|
| B-01 独立地块几何 | 真实前/侧/后院临路 parcel，替代"楼框+2m"；512m 小场景 | — | ✅ 完整切片（joint-plains 18/18 住宅 + 4/4 服务 + gate pass） |
| B-02 需求与道路容量 | 24 段自动路预算 vs 882 目标的供给限制显式化；主次支路与农田可达 | B-01 工程自测 | ✅ 独立供应控制：3 条自动路、collector+local 实网、24/24 realized-with-access、road/tier audit pass、disconnect 0 |
| B-03 用途—实际覆盖闭合 | 城市未建地/院落通道/农田田埂农路/自然保留地的实际表面 | B-02 在小场景证明接口 | ✅ 当前控制闭合（512m unresolved 128,542→0；最大图 50,538,180m² ground + ledger 60,000,000/0 + gate pass） |
| B-04 形态与公共服务 | 紧凑混合/低密住宅/村落农田的可辨认差异；设施服务范围、步行联系 | B-01～B-03 接口可用 | ⚠️ profile-driven 核心（heightM/serviceRadiusM 已驱动；coverageRatio 由 `morphologyCoverage=audit` 逐地块审计；生成器不静默缩放作者几何） |
| B-05 集成与生态尾差 | 多 seed 7/42/2026、非零 origin、最大图一次集成、生态尾差 | 上述接口稳定 | ✅ 最大图门槛（complete-ground max gate pass；second bake worldRevision identical, 1777 files byte-identical；工程最大图 gate pass） |

### 3.3 需求 ID 交叉对照

| 源 | ID 范围 | 含义 | 在本表位置 |
|---|---|---|---|
| `abc-goals-realignment` §3 | R01–R16 | 追溯式总需求（最完整的一份） | 第 4 章逐条落位 |
| `acceptance-contract` | A-01–A-10 | A 的验收边界 | **5.1**（逐条现状）+ D18 |
| `milestone-b-acceptance` | B-G1–B-G8 | B 的验收边界 | D9 / D10 / D11 / D17 |
| `milestone-c-agent-task` | C-N1…C-G1（10 项） | C 的最终要求 | D7 / D8 / D11 / D13 / D14 / D15 |
| `c-post-c-six-capabilities-plan` | FUT-01–FUT-06 | 未来能力（已登记，未实现） | **D19** |
| 本文 | D1.1–D19.x | 仅导航用 | 第 4 章 |

---

## 4. 需求域表（19 个域）

> 读法：`状态` 用 2.1 的词；`来源` 给出编号或文档；`证据/缺口` 只写有据可查的。
> 标 ⚠️ 的行是本项目最容易"以为做了其实没做"的地方。

### D1 产品形态与范围

| ID | 需求 | 状态 | 证据 / 缺口 |
|---|---|---|---|
| D1.1 | 默认输出 `2.5d`，可选 `2d`，**不做**对外完整 `3d` 生成模式 | 生产可用 | `renderMode: "2.5d"` 出现在每个 `world.json`；3D 只作为只读预览 |
| D1.2 | 2.5D 是类 RTS 斜俯视浏览，**不实现**单位/战斗/建造/AI | 生产可用 | G27～G34 均为只读消费者 |
| D1.3 | 1m×1m 逻辑单元；Heightfield + 地表 + 少量地下体素；默认合并 Mesh，可切 1m³ 方块视觉 | 生产可用 | 高度场 1m 网格；无体素地下层（体素仅 `.gemap` 存储层概念） |
| D1.4 | 风格由用户决定；参考素材不得自动成为正式资源；默认占位用原创程序化几何 | 部分 | 资产为 Quaternius 外部模型 + 程序化占位 |
| D1.5 | **殖民挑战游戏方向**（Flood Fortress：`GameCommand` 命令层、铜银金评级、ASR/本地模型接缝） | 已搁置 | 代码仍在 `bevy/crates/core/src/gameplay/{command,sim,state,snapshot}.rs`；未在 09-01 之后的路线中出现 |
| D1.6 | 大图脚本化检查维持 **≥150 workload FPS**（`pnpm bevy:fps`） | 已搁置但脚本仍在 | `package.json` 仍注册 `bevy:fps`；无当前世界上的最新测量 |

### D2 坐标 / 尺寸 / 分块 / 流式

| ID | 需求 | 状态 | 证据 / 缺口 |
|---|---|---|---|
| D2.1 | 场景尺寸：最小 512×512m，默认 1km×1km，最大 **6km×10km**；任意整数米，不要求 512 倍数 | 跨尺度通过 | A 的最终世界 6000×10000m / 240 chunks；b8 与 R11 为 1000×1000m |
| D2.2 | 边缘分块按 512m 存储，超出实际场景范围标 `void` | 生产可用 | chunk 索引与 `void` 标记在 `scene.json` |
| D2.3 | 流式块 512m；一个流式块含多个 `.gemap v3` **16m 存储 Chunk**；32×32 Region 概念保留；三类名词不得混用 | 生产可用 | `chunk-{x}-{z}.*` 命名；文档层面仍未统一口播"块" |
| D2.4 | 对外坐标 `world_x`(东+)/`world_z`(南+)/`world_y`(米)；`gemap v3` 内部冻结 `(z,x,y)`，转换覆盖正负坐标与分块边界并有单测 | 生产可用 | 多处 `world_x/z/y` 契约；转换测试存在 |
| D2.5 | 场景数默认 1，可扩到 3~5，>5 只给预算警告不失败 | 仅设计 | 实测全部单场景 |
| D2.6 | 正式生成必须**完整烘焙**所有场景与分块，不得只生成局部 | 生产可用 | 1km/6×10km 均全量烘焙 |
| D2.7 | `terrainDomain{originX,originZ,widthM,depthM}`：所有场景须完整位于其中，越界/溢出/未知字段拒绝；同 seed 同域重叠坐标取同高 | 生产可用 | r23 |

### D3 高度精度

| ID | 需求 | 状态 | 证据 / 缺口 |
|---|---|---|---|
| D3.1 | 支持毫米精度（`style.heightPrecision = "millimeter"`），默认仍兼容旧的 0.25m i16 | 生产可用 | r23；当前 1km 与 6×10km 世界均走毫米 |
| D3.2 | `GWHM v1` 头 16B（magic/version/width/depth）+ 行优先 i32 LE 毫米；长度必须 `16+W×D×4`；单块 1..512 | 生产可用 | r23 §GWHM |
| D3.3 | sidecar 必须声明 `heightEncoding` 与 `heightPrecisionM`；旧客户端不得跳过 sidecar 直接读 raw i16 | 生产可用 | 两种编码名并存 |
| D3.4 | LOD overview 的 0.25m 与 chunk `minQuarterM/maxQuarterM` 是**粗摘要**，不能当毫米工程证据 | 生产可用 | r23 |
| D3.5 | 连续性审计直接算米高度差（`maxBoundaryHeightStepM`），不得先舍入到 0.25m 再判 | 生产可用 | r23 |
| D3.6 | 采样误差 ≤0.0005m；编码器拒绝非有限值/超 i32/数量错误 | 生产可用 | r23 §encoder |

### D4 存储格式 / LOD / 渲染流

| ID | 需求 | 状态 | 证据 / 缺口 |
|---|---|---|---|
| D4.1 | `.gemap v3` 为发布/交换/离线导入归档格式；sidecar 为消费者分片读取（JSON 元数据 + 二进制主体） | 生产可用 | `world.gemap` + `sidecar.json` 每世界都有 |
| D4.2 | `world.json` 存范围/场景图/LOD/seed/版本/哈希；`scene.json` 存尺寸/原点/分块索引；chunk JSON 存坐标/有效尺寸/边界摘要/文件名/哈希 | 生产可用 | b8 `world.json` 1KB，字段齐全 |
| D4.3 | LOD0=1m（近景）、LOD1=4/8m、LOD2=32/64m（战略图）；真源始终是 Manifest+seed | 生产可用 | `lod2.bin` 存在 |
| D4.4 | 发布前必须校验 `.gemap` 与 sidecar 一致 | 生产可用 | 独立 gate 含该项 |
| D4.5 | 消费者不得读整份 `scene.json`：必须走内容寻址 render-stream（目录 + 每块 payload） | 生产可用（A-07 后） | r44：367MB scene → 3.24MB 目录 + 240 payload 138.7MB；首块 7.008s→5.311s、JS heap 675MB→61.6MB |
| D4.6 | 发布顺序：序列化+SHA256 → 同内容文件逐字节相同 → 临时文件 flush/rename → 最后替换 `preview-scene.json`；只有 404 才退回旧 `scene.json` | 生产可用 | r44 §发布顺序 |

### D5 输入 / Manifest / Patch / 可复现

| ID | 需求 | 状态 | 证据 / 缺口 |
|---|---|---|---|
| D5.1 | 自然语言与严格 JSON/YAML 都归一化为唯一 `glyphweave.manifest.json`；确认后才生成 | 生产可用（JSON 路径） | 31 份 `examples/*.manifest.json` 是输入矩阵；NL 入口不是 CLI 能力 |
| D5.2 | Seed + Manifest + 生成器版本必须可复现；同输入两次字节一致 | 生产可用 | A 的确定性重跑 1777/1778 文件字节相同，唯一差异是 `generation-report.json.output` |
| D5.3 | 不得覆盖原始 Manifest；每次修改追加**不可变 Patch**；世界版本 = Manifest 哈希 + Patch 链哈希 + 生成器版本 | ⚠️ **未实现** | b8 `world.json` 实测 `"patchChainHash": ""`（空串）；CLI 有 `patch` 子命令但生产链未接 |
| D5.4 | 关键字段缺失必须追问，禁止静默补写；软约束可降级但要写进报告；**硬冲突必须失败** | 部分 | manifest 校验严格（未知字段拒绝），但"追问"无实现 |
| D5.5 | CLI 至少 `generate / patch / validate / pack / unpack / inspect` + 输出 `generation-report.json` | 生产可用 | 实际子命令已扩展为 `generate-world`/`world-quality-gate`/`export-preview-stream`/`design-road-corridor` 等 |
| D5.6 | Agent 不得直接写二进制/改 Manifest；只能产 Patch 由 CLI 重烘焙 | 仅纪律 | 无强制机制 |

### D6 实体身份 / 跨块 / 资源替换

| ID | 需求 | 状态 | 证据 / 缺口 |
|---|---|---|---|
| D6.1 | 每个关键实体唯一全局 `entity_id`；大型实体 anchor/朝向/尺寸/bounds | 生产可用 | 实体命名 `generated.frontage.scene-0....` |
| D6.2 | 跨 512m 分块的实体**只有一份定义**，分块只存引用/局部代理/裁剪 | 生产可用 | 跨块 patch 按 ID 合并、重复 ID 几何必须一致（r44） |
| D6.3 | 入口/交互点/碰撞/传送点用全局坐标 | 生产可用 | `frontageRoadPoint` 即全局点 |
| D6.4 | 资源只引用 `asset_id`；`assets/glyphweave.registry.json` 可覆盖占位；替换不得改坐标/实体 ID | 生产可用 | registry + `licenses/` 存在 |
| D6.5 | 生成器不得覆盖用户 `assets/`；生成物进 `generated/` | 生产可用 | 输出目录约定 |
| D6.6 | 稳定 ID / 存量约束：新增无关需求不得重排他人身份；移动/移除要显式记录 | ⚠️ **未达成** | r47 实测：96 个旧 ID 不再出现、117 个新 ID 出现、125 个同名对象数据变化，仅 218 个几何相同只是改名；A-03 的"存量"仍靠测试而非机制 |

### D7 自然地形（形态与侵蚀）

| ID | 需求 | 状态 | 证据 / 缺口 |
|---|---|---|---|
| D7.1 | 连续、可复现的高度场：同一世界坐标同一地形，邻近格高差受控 | 生产可用 | `value_noise2d`/`fbm2d` + `landform_field`（河谷/平原/丘陵/山地四带） |
| D7.2 | 山脊谷地/盆地/河流/流域/汇流/闭合洼地，多尺度连续 | 部分 | 有 1628 个闭合洼地统计与 4 个未解决平坦终端（r44 gate）；不等于流域水文 |
| D7.3 | 明确侵蚀模型与近似边界；不得给噪声改名冒充真实侵蚀 | ⚠️ 未达成 | 现有 `erosion_carve` 自述"不是粒子模拟"；r28 标注 `domain-approximation` |
| D7.4 | C-N1：每个自然算法的出处矩阵（Mapgen4 / symbios-ground / 自写 / 近似边界）+ 可保留/拆分/替换决策 | ⚠️ 初版完成 | `docs/design/natural-source-matrix.zh-CN.md` 已逐函数列出；Mapgen4/symbios-ground 未 vendor/未复制；新增 `ecology_habitat` 纯接口 |
| D7.5 | 保护/未知用途不得被建设改写；保护边缘填土包络 `final ≤ natural + 0.5 × Chebyshev距离(m)` | 生产可用 | r24 §保护区；实测保护区 9,602,160m²、高度改变样本 0 |
| D7.6 | 现有石头/树/旧路/旧农田/围墙/旧建筑作为 `ExistingFeature` 先落地，声明来源、永久性、保护级别、可移动性与转化策略 | 仅设计 | 形态研究 §171；架构文档 §43-45；代码中无该持久化结构 |
| D7.7 | 版本化快照：`NaturalTerrainSnapshot`（heightfield/climate/drainage/water/landform/hazard/ecology + source/version）与 `EngineeredTerrainSnapshot`（有序操作/土方/结构实体/影响块/排水影响/稳定性/final hash）分别保存、分别审计 | 部分 | `natural-terrain.json` 与 `engineered-terrain.json` 已产出；schema 未达设计文档要求的字段面 |
| D7.8 | `DrainageGraph`：节点/边/流域/流量等级/洪泛区/沉积带；河流必须是图而不是装饰折线 | 部分 | 现有 32m 采样 D8-style 地表流图（标注 `derived-d8-sampled-v1`）；512m 场景 257 节点/248 边/0 无效边/0 环；最大图 1628 个闭合洼地、**4 个未解决平坦终端** |
| D7.9 | 宏观地貌波长 ≥ 短边的 1/5–1/2、山脊谷地长程相关、迎风/背风与岩性影响、边缘外延避免截断 | 仅设计 | 自然研究 §97-104 |

### D8 水文与工程地形

| ID | 需求 | 状态 | 证据 / 缺口 |
|---|---|---|---|
| D8.1 | 最大填方 **6000mm**；纵向坡度 **80mm/m = 8%**；硬支撑优先于软过渡；只填高不挖低自然层 | 生产可用 | r45/r46/r47 一致引用；回归测试"路肩只改 1mm 必须失败" |
| D8.2 | 路面宽 4–32m 偶数、单侧路肩 0–8m（样例 8+3+3=**14m** 支撑） | 生产可用 | r45/r46 |
| D8.3 | 保护区填土衰减包络；水体相交拒绝，不虚构桥梁 | 生产可用 | r45 §拒绝 |
| D8.4 | 平台/挖填/路堤/边坡/挡墙/排水/涵洞作为工程实体并参与土方差量 | ❌ 未开始 | r46 §3.2 明说"没有完成走廊外边坡、路拱、排水、竖曲线舒适性或全域土方平衡" |
| D8.5 | C-E2 工程后水文反馈（阻水/溢流/断流/涵洞正负测试） | ⚠️ 首个实现 | `engineeringDrainage="audit"` 对自然/最终地形重算图并输出 changedFlows/newSinks/lostSinks/raisedTerminals；仍是路由完整性，不是水量/洪泛模型 |
| D8.6 | 工程操作与最终地形/实体/审计共享同一版本 | 部分 | v2 工程快照绑定真实道路实体 ID（r46） |
| D8.7 | 地形只能通过**显式工程操作**改变：cut / fill / retain / embank / reclaim / channel / bridge / culvert / terrace；每个操作记录 footprint、原始自然场版本、目标高程、材料体积、排水影响、边坡稳定性、相邻影响、原因与审计状态；`final = natural + ordered deltas` | 部分 | 目前只有 fill + road profile 进 bake；其余操作**无实现** |
| D8.8 | 工程优先级：保育/水体/灾害 > 主排水 > 桥/堤 > 主路 > 次路 > 地块平台 > 建筑地基 > 装饰性微地形 | 部分 | 硬支撑优先级已实现（道路 > 建筑），其余未实现 |

### D9 道路

| ID | 需求 | 状态 | 证据 / 缺口 |
|---|---|---|---|
| D9.1 | 低地骨架路网真实连通并独立审计 | 生产可用 | r47：246 条基线边；`lowland_road_network_is_real_connected_and_independently_audited` |
| D9.2 | 自动生长：Agent 只给用途/长度/预算，Rust 决定线路；预算耗尽保留未满足目标 | 生产可用 | r47：`maxNewSegments` 样例 24、尝试预算 4096、长度 {512,256,128,64}、`reachDistanceM` 160 |
| D9.3 | 目标判定必须"footprint 正面积相交 **且** 分区中心 Manhattan 距离 ≤ reach"；只有边界接触不算 | 生产可用 | r47 §74-78（曾出现"目标满足但区内无路"的假阳性） |
| D9.4 | 显式扩展走廊：轴向、偶数米长、最小 16m、最大 10km；端点须为已有节点且标高一致；平台 ≥8m；路口 8m；跨保护/水体拒绝 | 生产可用 | r46 |
| D9.5 | 纵断面 `roadProfile`：整数毫米站点、按世界坐标插值；**整条路不能用一个 worldY 推高度** | 生产可用 | 扩展路；⚠️ 基础边在 T 系列重构后也带 profile，正是当前回归源（见 §6） |
| D9.6 | 毫米道路审计容差 **0.00005m**（旧平路 0.26m）；1mm 篡改必须失败 | 生产可用 | r46 §4 |
| D9.7 | 共享路口只允许双方声明的端点；隐式 T 形连接拒绝，不自动拆分原路 | 生产可用 | r47 §95-97 |
| D9.8 | 主次支路分级、小巷、真实转向/路口几何、立交 | ⚠️ **部分** | collector/local 宽度、road tier、任意朝向 routeGrade 挖填与 41 个合并跨线 span 已进入生产；路口仍以同高铺装补面过渡，Rust 路口并集多边形、桥头挡墙和完整转向几何未完成 |

### D10 地块 / 建筑 / 入口 / 院落 / 设施

| ID | 需求 | 状态 | 证据 / 缺口 |
|---|---|---|---|
| D10.1 | 地块边界、建筑 footprint、院落、公共空间各有独立几何来源（不得"楼框+固定 2m"当所有类型地块） | ⚠️ 部分 | 当前仍是 `development::plan` 把 site bbox 各扩 2m；B-01 就是来取消它的 |
| D10.2 | 院落：1m 单元切分、25% 坡地上限、草地 0.025m 覆盖偏移、面积守恒、拒绝具名 | 生产可用 | r21：208m²/1817m² 实现；1km 有 55m² 因 `yard_grade_exceeds_25_percent` 未实现 |
| D10.3 | 临街程序 `style.frontageProgram` v1：station 间距 8–128m 偶数、setback 8–32m 偶数、单请求 ≤1024/总 ≤2048/≤64 请求、候选预算 ≤65536 | 生产可用 | r48；必须 road-first + 毫米 |
| D10.4 | 建筑必须同时具备：平台整地、真实步行入口、指定道路连接面（4m 宽硬支撑） | 生产可用 | r48 `frontageRoadPoint`；门前凸起 1.5–5m 的失败已修 |
| D10.5 | 住宅最低数 `housingMinimum`（0–65536，按场景）：设施优先后必须补足，不许改低 | 生产可用（fixture 18/18） | r27；`examples/service-program-joint-plains.manifest.json` = 18 |
| D10.6 | 设施程序：类型/尺寸/数量显式，按原尺寸规划不缩小；三层分离（请求 / 计划候选 / 有入口实现） | 生产可用 | r26：请求 4、计划 3→4、实现 3→4；尺寸范围 store 6–40/6–30/3–12、school 30–128/26–120/4–30、church 12–80/12–80/3–40、town_hall 30–128/24–120/8–60、market 36–128/30–128/3–20 |
| D10.7 | 联合审计：实体 ID 唯一、类型/尺寸/变换/入口与计划一致；保存的 audit 声称 pass 不能覆盖实际数据 | 生产可用 | r27 §5；CLI 从 manifest+自然重建 |
| D10.8 | 地块四朝向、非零 origin、跨 zone 分摊、无重叠守恒 | 部分 | 有测试；大规模真实世界未验 |
| D10.9 | 形态学驱动的街区差异（紧凑混合/低密/村落）、多入口、共享步行网络 | ⚠️ **部分** | morphology compile、compact/village 控制和 2026-10-21 新老城阶段控制已落地；商业/住宅按中心距离与建设阶段变化；多入口/共享步行网络仍未完成 |
| D10.10 | ⚠️ `engineered_placement::reconcile` 必须遵守对象描述符声明的 `fallback`（skip 就跳过），不得把不合规小卖部升级成整图失败；跳过必须具名 + 计数 | **未修**（T-0013 授权项） | T-0012 §2：`assets/objects/storefront.object.toml` 要求 road 距离 8m，而 fixture `setbackM = 12` → 立面间隙约 14m，必然不满足 |
| D10.11 | 可通行合同：最大纵坡 **25%**（`access::WALKABLE_GRADE = 0.25`，`worldgen/access.rs:16`）、入口引道 **2m**（`ENTRANCE_APRON_RUN_M = 2.`，`worldgen.rs:4258`）→ 引道容差 0.5m；入口台阶 0.30m；通道 1.5m 宽、2m 搜索网格、120m 预算、≤6000 展开节点 | 生产可用 | 一手核对常量；这组数字是当前回归的判据来源（见 §6.1） |
| D10.12 | 任何建筑（authored 或 generated）都必须有真实入口：缺入口/入口不在边界/被挡/台阶超限各有独立状态码 | 生产可用 | AccessGraph v2；`quality-report` 从最终 heightfield 与实体重算，不信任 scene 里的计数 |

### D11 用途与地表实现 / 账本

| ID | 需求 | 状态 | 证据 / 缺口 |
|---|---|---|---|
| D11.1 | Landcover 候选五角色：`managed_yard` / `agriculture_candidate` / `conservation_candidate` / `future_development` / `unmanaged`；候选**不等于**实现 | 生产可用 | r19；已规划道路/建筑 footprint 不再进 remainder，避免重复消费 |
| D11.2 | 用途账本四行（urban/rural/nature/unassigned）× 七列物理表面（buildings/roads/managedGrass/accessPavement/cultivatedSoil/fieldHeadland/farmTracks） | 生产可用 | r25 |
| D11.3 | 三条守恒等式：行 `realized = Σ物理列`；行 `planned = realized + unresolved`；全图 `sceneArea = Σrealized + Σunresolved` | 生产可用 | r25 §41-43；独立 gate 校验 `landUseLedger.status` |
| D11.4 | `allocationStatus` 与 `realizationStatus` 分离：分配在容差内 ≠ 实现；剩余面积归零才算 realized | 生产可用 | r25 §107 |
| D11.5 | 口径版本标识必须一致：`disjoint-owned-surfaces-v1/v2/v3`、`disjoint-purpose-cover-v1`、`legacy-entity-proxy` 不得混加 | 生产可用但易错 | r25/r21/r22 三代标识并存；`byKindM2` 已降级为 `entityFootprintDiagnostic` |
| D11.6 | 庭院实现：1m 单元、25% 坡上限、草地 0.025m 偏移、入口铺装求交去重、面积守恒 | 生产可用 | r21：208m²/1817m²；1km 有 55m² 因 `yard_grade_exceeds_25_percent` 拒绝 |
| D11.7 | 农业实现：田块尺寸可配 16–128m（4 的倍数，默认 24×24）、耕作面 (w−8)×(d−8)、田面 12% 坡、4m 通道/8%/160m、有界绕行 A*（4096 节点） | 生产可用 | r22/r40/r41；seed42 田块 23→29，seed7 仍 24 |
| D11.8 | 自然保留地实际地表：16m 网格分类 ground/forest_floor/wetland，面积守恒，水体相交不生成 | 生产可用（示例） | r28：49152m² = 40960+4352+3840 |
| D11.9 | `future_development` / `unmanaged` / `reserved` 一律不得自动获得 realized 信用 | 生产可用 | r18 §61-70 明确列账 |
| D11.10 | 最大图用途实现闭合 | ⚠️ **部分闭合** | 新增 `residualLandProgram.coverage="complete-ground-v1"`；512m 控制 unresolved **0**（128,542m² 变为真实 ground mesh）；最大图仍待重烘焙，A 例外 #2/#5/#6 尚未在最大图关闭 |
| D11.11 | landcover 实际表面全部实现 | ⚠️ **部分闭合** | 512m 控制 `landcover-audit.status=pass`、unresolved 0、ground cover 128,542m²；旧世界默认关闭；最大图与 CLI 重算 gate 仍缺 |
| D11.12 | 保护区填土距离查询精确且高效（Chebyshev，非欧氏） | 生产可用 | r42：8.52–10.75× 加速，1238 个保护矩形、200000 次查询逐项等价 |

### D12 生态

| ID | 需求 | 状态 | 证据 / 缺口 |
|---|---|---|---|
| D12.1 | 密度按面积驱动（单位：实例/ha）：forest 420（canopy 40%/understory 30%/ground 30%）、wetland 280（emergent 70%/understory 30%）、shrubland 190（shrub 60%/ground 40%） | 生产可用 | r38；不是冠幅覆盖率或生物量 |
| D12.2 | 用**累计取整**而非逐 patch 上取整（100×256m² 森林 = 1075 而非 1100） | 生产可用 | r38 §36-39 |
| D12.3 | 严格等式：patch 目标 = Σ层目标 = 已生成 + 缺额 | 生产可用 | r38 §49-51 |
| D12.4 | 定位用精确实体矩形净空（旋转矩形、1e-9 容差、仅边界接触不算重叠），失败在 16m patch 内按 1m 步长找 289 个备用格点 | 生产可用 | r37 |
| D12.5 | 实例 ID = sourcePatchId + layer + ordinal；用 blake3(seedOffset, id) 取位置扰动，Y 仍从 `BakedTerrain` 采样 | 生产可用 | r38 §57 |
| D12.6 | 植物有缺额时，即使地表 100% 实现，`SurfaceAudit.status` 也保持 `incomplete` | 生产可用 | r38 §77 |
| D12.7 | 最大图配额 **342,380** 株 | ✅ 已关闭 | A-closure 2026-10-20 实测 **342,380 / 342,380 / 0**；历史 r43 缺 3 → r46/r47/r49 缺 1 保留为历史 |
| D12.8 | 渲染用实例化：每活动块/种类一个 `InstancedMesh`，失活块 dispose | 生产可用 | r39：1915 实例 → draw calls 1915→**3**，三角形数不变 21630，geometry 1915→3 |
| D12.9 | 物种级群落/演替/湿地水位适宜性/林冠真实覆盖 | ❌ 未开始 | r38 §92 自述未完成 |

### D13 形态与文化案例

| ID | 需求 | 状态 | 证据 / 缺口 |
|---|---|---|---|
| D13.1 | 案例规模：**中国 12 + 美国 12 + 欧洲 12**（各 5 城区 + 4 乡村 + 3 荒野）+ **日本 6 城区 + 香港 6 城区**，共 **48** | 仅研究 | 形态研究 §40/64/88/107/120。⚠️ 历史表述对日/港"6 城区"是**合计还是各自**存在歧义（`milestone-b-plan` §5），定稿前不得宣称已覆盖 |
| D13.2 | 证据等级 A（政府导则/官方数据/法规）、B（OSM/公开卫星/GIS）、C（学术论文）、D（旅游网页，仅观察线索） | 仅研究 | 形态研究 §20-23 |
| D13.3 | 参数按分布而非"国家开关"：街区尺度、路网层级、地块深度、退界、混合度、绿地比例、坡地工程、滨水公共性、历史叠加 | 仅研究 | 形态研究 §16 |
| D13.4 | **6 种城市形态**：`dense-core` / `river-delta` / `coastal-bay` / `mountain-valley` / `temperate-plain` / `low-density-suburban`，只改分布先验不改硬约束；路网尺度 260/360/460m | **已实现** | 一手确认：`CityForm` 枚举与字符串解析在 `worldgen.rs:589-604`，有 `profile_entities` 与多主题测试 |
| D13.5 | **12 种聚落原型**（河谷线性镇/海湾港口/山前台地/平原网格/轨道新城/工业铁路带/湖滨旅游/三角洲堤防/岛屿多中心/林地村镇/农田道路聚落/旧城新区拼接） | ⚠️ **1 个垂直切片** | `urbanDistrictProgram` 已实现“旧核心/更新带/新城环”的可执行阶段结果；仅覆盖 1 个原型方向，不等于 12 种全部实现 |
| D13.6 | 每个参数的 landRatio 与 density 分开（landRatio 是场景面积比，density 是同类地块内强度），不能混用 | 部分 | 参数基线表存在（道路 0.08–0.22/0.55–1.00 等），未逐条落地 |
| D13.7 | `MorphologyProfile` 14 字段；每个值须标 `hard_constraint`/`soft_preference`/`observed_source`/`inference`/`confidence` | 仅设计 | 现有 `settlement/morphology.rs` + `morphology_compile.rs` 是编译层，不是该 14 字段合同 |
| D13.8 | 12 步生成顺序：自然与水文 → 灾害/保护区 → 历史要素 → 基础设施 → 城乡分区 → 主干路与公交节点 → 街区/地块/公共空间 → 建筑入口院落 → 商业/学校/市场/夜市 → 街树/林带/公园 → 人为自然影响 → 全局审计 | 部分 | 现有顺序为 自然→分区→道路→地块→建筑→地表→生态；缺历史要素/公交/公共空间 |
| D13.9 | 生成器必须能回答"每个对象为什么在这里"（父节点/约束/评分/替代候选/拒绝原因） | 部分 | 各 `*-audit.json` + 决策日志；无统一血缘字段 |

### D14 消费者（HTML / Godot / 预览服务）

| ID | 需求 | 状态 | 证据 / 缺口 |
|---|---|---|---|
| D14.1 | HTML 战略图用 Konva/Canvas，只加载 LOD2 与场景/分块元数据 | 生产可用 | `adapters/html` |
| D14.2 | Three.js 2.5D 近景只加载视口附近块（3×3 或 5×5），读高度场/地表/实体索引/地标轮廓 | 生产可用 | render-stream 按活动集加载与释放 |
| D14.3 | 支持 scene_id/chunk_x/chunk_z 与 world_x/z/y 显示、按坐标定位 | 生产可用 | G29～G30 读图栏 |
| D14.4 | 第一版 HTML 不直接编辑大世界二进制；改动只能经 Patch 回 CLI 重烘焙 | 未实现 | 无编辑通道（依赖 D5.3） |
| D14.5 | `glyphweave preview <world-dir>` 只读静态结果、不触发生成；可部署到普通静态服务器 | 生产可用 | 多轮用 `preview` 端口 88xx 查看 |
| D14.6 | 消费者不得自行解释"房屋=院子"、不得再贴地、不得丢毫米偏移 | 生产可用（有回归） | r21/r48 各修过一次同类问题 |
| D14.7 | Godot 4.7 + GDScript：只读 sidecar、运行时建 Mesh/碰撞/实体节点、相机附近加载卸载、启动校验哈希、缺失块属发布错误 | 最小运行通过 | Godot 4.7.2 最大图 headless：scene-0、chunk 0,1、loaded 6、exit 0、无脚本错误；碰撞/发布导出仍未展开 |
| D14.8 | 开发版显示坐标 HUD / 作弊传送；发布版隐藏调试但保留剧情传送点 | 部分 | HUD 有；传送/发布版区分无证据 |
| D14.9 | A-07：全 240 块加载/卸载/重访与来源一致 | 独立验收通过 | A 签收：240/240 chunks、3 warm、9 revisit、网络重试、过期响应 pass |

### D15 资产 / 许可 / 发布

| ID | 需求 | 状态 | 证据 / 缺口 |
|---|---|---|---|
| D15.1 | 生成器不得覆盖用户 `assets/`；生成物进 `generated/`、参考素材进 `references/`、正式素材进 `assets/` | 生产可用 | 输出目录约定 |
| D15.2 | 格式：3D 优先 `.glb`，兼容 `.gltf + .bin + textures`；Godot 支持 `.tscn`/`.tres`；2D 支持 `.png`/`.webp` | 部分 | 当前资产为 `.obj`+`.mtl` 与 `.gltf`；`.glb` 无 |
| D15.3 | 每个素材登记 `asset_id`、格式、单位、原点、朝向、缩放、碰撞规则；`glyphweave.registry.json` 按 id 覆盖占位 | 部分 | `assets/glyphweave.registry.json` 存在；字段面未核对 |
| D15.4 | C-L1 开源合规：顶层 MIT 核实；依赖/复制代码/vendor/模型/纹理/GIS/算法版本与许可清单、NOTICE、**未决项清零** | ⚠️ 来源已解析、历史日期未证明 | `workerd-windows-64` 为 Apache-2.0；**Quaternius 建筑资产**已定位到 **Ultimate Buildings Pack (Dec 2019)**，页面 CC0-1.0，本地为改名/单材质派生；精确下载日期仍未由仓库历史证明，当前 QAL v1.0 禁止单独再分发原包 |
| D15.5 | 引入外部代码/算法前先做来源矩阵与许可预检（如 Mapgen4 Apache-2.0） | ⚠️ 初版完成 | `docs/design/natural-source-matrix.zh-CN.md` 已登记当前算法来源与未复制声明；workerd 已解析，Quaternius 仍未解析 |
| D15.6 | 视觉 `warn/fail` 不得被 Agent 解释成生成成功；视觉反馈须经 preview 写入 `visual-feedback.json` | ⚠️ **控制通过·全量未签收** | 2026-10-21 现代湖滨控制已写 `visual-feedback.json`；白色伪影、草场圆形和城镇固定盒子已修；全量世界人工长漫游仍未完成 |
| D15.7 | 正式版本存档：证据归档、runtime 与 world 冻结、可复现说明 | 部分 | `release-reproducibility.zh-CN.md` 有要求；A 签收时明确"本次尚未执行证据归档或 Git 提交" |

### D16 性能与资源

| ID | 需求 | 状态 | 证据 / 缺口 |
|---|---|---|---|
| D16.1 | 最大图生成与独立 gate 各 **600s / 8192MiB**（private bytes），不得事后提高 | 生产可用（多次触线） | r43 379.267s；r46 418.541s/183.121s；r47 452.017s/216.332s；r49 388.459s/176.504s；**r48 曾 600.088s 被预算中止** |
| D16.2 | 24 段自动路是**内容供给预算**，不是计算预算，两者不得混淆 | 生产可用 | r47 §150-154；B-G7 |
| D16.3 | 性能优化必须同输入等价对照（linear vs indexed） | 独立验收通过 | A-05：生态 linear/indexed blake3 相同，检查量 4,583,094,907 → 48,484 |
| D16.4 | 地表阶段耗时须可分解（trace） | 生产可用 | r49：访问图 0.037s / 院落 0.648s / 农业 9.651s / 自然覆盖 0.039s / **生态 59.675s** |
| D16.5 | 全文读取大 JSON 不得成为瓶颈（流式/紧凑表示） | ⚠️ 风险 | r47 自述权威 JSON 已接近 Node 全字符串读取容量风险区 |
| D16.6 | 150 workload FPS（游戏侧） | 无当前证据 | 见 D1.6 |

### D17 多场景与转接点

| ID | 需求 | 状态 | 证据 / 缺口 |
|---|---|---|---|
| D17.1 | 显式 `scene_graph`，每场景独立 scene_id/尺寸/原点/seed/分块索引 | 部分 | `world.json` 有 `sceneGraph` 字段，实测均单场景 |
| D17.2 | transition（道路出口/渡口/山口/洞穴/传送门）含源/目标/坐标/朝向/落点 | ❌ 未开始 | 无实测场景 |
| D17.3 | 相邻场景仅在 Manifest 声明后做连续生成，并校验高度/河流/道路/地表连续性 | ❌ 未开始 | — |
| D17.4 | 分块可并行但必须读相邻边界摘要校验；冲突则终止发布 | 部分 | 跨 chunk 高度连续性有测试与审计 |
| D17.5 | 单场景内跨块实体不重复、不遗漏 | 生产可用 | r44 跨块 patch 按 ID 合并 |

### D18 验收与流程纪律（贯穿全项目）

| ID | 需求 | 状态 | 说明 |
|---|---|---|---|
| D18.1 | 证据分 8 级，互不替代 | 执行中 | 见 2.2 |
| D18.2 | 质量判定同时看 exit code 与 JSON `status` | 执行中 | 曾因只看 exit 0 误接受 fail |
| D18.3 | 不得调松任何数值合同；判据是"真实缺陷是否更难被发现" | 执行中 | 章程 §3 第 4 条 |
| D18.4 | 拒绝必须具名、持久化、可序列化往返；不许静默丢弃/静默压平 | 执行中 | r26/r27/r48 均把拒绝写进权威 JSON |
| D18.5 | 测试/世界运行额度逐进程记账，超支保留不抵消 | 执行中 | G 轮次大量超支案例 |
| D18.6 | 金标准回归：`c3_base_identity_*`、`w4_chain_digest_*` 不得改期望值 | 执行中 | T-0012 §4 明列为不可动 |
| D18.7 | 世界生成后目录即冻结；预览只能在副本上操作 | 执行中 | 章程 §4 |
| D18.8 | A 的六条内容例外不得继承为 B/C 的通过条件 | 执行中 | `milestone-b-plan` §8 |
| D18.9 | 主 Agent 只做范围/裁定/集成，不代写实现；worker 不自行签收 | 执行中 | 多份裁定反复强调 |

### D19 未来能力（FUT-01～06，全部未实现）

| ID | 能力 | 分期 | 当前状态 |
|---|---|---|---|
| FUT-01 | 自然语言局部 Patch 编辑 | C 最小版本 | 未开始（且依赖 D5.3 Patch 链） |
| FUT-02 | LLM 生成非内置 Recipe + 有作用域规则 | C 接口，C 后实现 | 未开始 |
| FUT-03 | 公里级复合设施拆分组装、跨 chunk 身份与连接 | C 基础接口 | 未开始（机场/港口/高铁等只有需求登记） |
| FUT-04 | 地理/气候/季风/洋流/地质背景的事实—推断—假设分层 | C 自然主线 | 未开始 |
| FUT-05 | 现实地名/GIS 分层近似重建 | C 后独立导入包 | 原型存在：`tools/gis-fetch-overpass.js`、`examples/shenzhen-bay-gis.manifest.json`（2000×2000m、132 建筑+145 道路+1 桥）；许可未审 |
| FUT-06 | 纯代码/preset 种子与 LLM 冻结计划重放分离 | C 先定合同 | 未开始 |

---

## 5. 进度

> **最近更新：2026-10-19**。依据 `git log 0edabb3..0d3be23`（**43 次提交**，2026-10-18～10-19）、`rk-x/` 四份文档，
> 以及 `bevy/crates/cli/tests/` 里钉死的常量。上一版的 §5 记到 Round 4，本版覆盖 Round 5 起至 HEAD。
>
> - §1–§4 一字未改；**§3.2 的批次状态是 10-18 快照，以 5.5 为准**；§7 的两条 P0 已于 10-18 关闭，优先级待下一版重排。
> - `rk-x/ROUND-LOG.zh-CN.md` 只记到 **Round 12**；Round 13 之后的证据只在提交正文与测试常量里，故本篇凡引提交均给 hash。
> - 测试数是**单次**运行；偶发访问违例约 4%（6.1 第 3 项），单次全绿不等于签收。

### 5.1 A 里程碑（A-01～A-10）签收后的实际状态

A 于 2026-09-09 签收，但**签收范围是"当时记录的候选与证据"**，不是"今天源码仍然通过"。

| 合同 | 当时的证据 | 今天是否仍成立 |
|---|---|---|
| A-01 源码/输入/runtime 对应 | R2/R3 构建来源 + U1 两 JS 增量 hash | ⚠️ 源码已改（T 系列重构 + 本轮 50 次提交），旧 runtime 不再对应当前源码 |
| A-02 固定实体/入口/访问图 | frontage 144/144 有入口 | ⚠️ 需求仍是 144；Round 53 当前为 **43 pad**（15 户、24 店）、入口未验证 **0**、gate **7 条**；144/144 已不可复现 |
| A-03 确定性/稳定 ID/存量 | 已绑定 stable-v1 / inventory-preserving 回归；R2 权威世界与冻结世界字节等价 | ⚠️ stable-v1 与 inventory-preserving 机制已存在；r47 的 96 ID 消失/117 新 ID 仍说明全量存量回归未闭合 |
| A-04 6×10km + 独立 gate | 240 chunks、gate 退出 1、六类失败 | A-closure 2026-10-20：240 chunks、gate 退出 1、4 类失败；内容缺口仍在 |
| A-05 性能等价 | 生态 linear/indexed blake3 相同（4,583,094,907 → 48,484 次检查） | ✅ 源码未动该路径 |
| A-06 权威数据/投影闭包 | header/scene/auditInput 闭包修复 | ✅ |
| A-07 240 块生命周期 | 240/240、3 warm、9 revisit、2 retry | ✅ 消费者侧；后续 r44 又加了 streaming |
| A-08 自然/保护/最低需求 | 保护区高度改变样本 0 | ✅ |
| A-09 许可库存 | Cargo/pnpm 清单 + Three LICENSE | ⚠️ 一项未解析（Quaternius 建筑包）；workerd-windows-64 已解析为 Apache-2.0 |
| A-10 正负回归 | Rust 470、Vitest 170、Node 252 | ⚠️ 数量已变：workspace **993 passed / 35 ignored**（`0d3be23`）；且受 6.1 第 3 项影响 |

### 5.2 6×10km 最大世界的数字演进（同一类口径，逐轮可比）

| 指标 | r43 基线 | r46 显式走廊 | r47 自动生长 | r49 沿路开发 |
|---|---:|---:|---:|---:|
| 实体 | 623 | 625 | 644 | 786 |
| 道路边 | 246 | 248 | 270 | 270 |
| 农田 | 270 | 331 | 501 | 428 |
| 地表 patch | 37,462 | 37,656 | 38,318 | — |
| 植物 | 342,377 | 342,379 | 342,379 | 342,379 |
| 植物缺额 | 3 | 1 | 1 | 1 |
| 城市实际用地 m² | 0 | 4,006 | 106,384 | 190,782 |
| 城市建筑 footprint m² | 0 | 0 | 0 | **72,000** |
| 农村 m² | 434,668 | 484,472 | 597,784 | — |
| 全图已实现 m² | 9,464,668 | 9,518,478 | 9,734,168 | **9,769,950** |
| 全图未实现 m² | 50,535,332 | 50,481,522 | 50,265,832 | **50,230,050** |
| 生成耗时 s | 379.267 | 418.541 | 452.017 | 388.459 |
| 独立 gate 耗时 s | 166.997 | 183.121 | 216.332 | 176.504 |
| gate 结果 | fail | fail | fail | fail |

> 2026-10-20 A-closure：同源码快照重新烘焙 6×10km，**255.941s / 240 chunks / 623 entities**；
> 生态 **342,380 / 342,380 / 0**，gate fail **4**（purpose-area / landcover / land-use planning / road-first）。
> 本次同口径未实现 **50,538,180m²**；历史 r49 的 50,230,050m² 保留为旧快照，不混用。
> 城市建筑从 0 到 72,000m² 仍是 r49 的真实增量（120 户 + 24 铺），未实现面积只降 0.6%。

### 5.3 A 的六条内容例外（#4 已关闭；其余不得继承）

| # | 原文 | 现在 |
|---|---|---|
| 1 | `"scene-0" automatic road targets remain out of reach after "segment_budget_exhausted"` | 历史 r49 为 882 目标 / 满足 69；本次 A-closure manifest 未复现该失败，但 B-02 仍需在带自动路的新候选上单独验证，不能据此宣称 #1 关闭 |
| 2 | `purpose-area allocation or realization remains incomplete` | **最大图 complete-ground 已不再出现**：ledger 60,000,000 / 0 / pass；旧 A-closure 保留历史数值 |
| 3 | `landcover remains unrealized` | **最大图 complete-ground 已不再出现**：planned 59,790,000 / resolved / unresolved 0；512m B-03 也已 pass |
| 4 | `ecology density budget has 1 unfilled plants` | **已关闭（2026-10-20）**：A-closure 6×10km 实测 `342,380 / 342,380 / unfilled 0`；证据见 `rk-x/A-REBASELINE-20261020.zh-CN.md` |
| 5 | `land-use planning or realization remains incomplete` | **最大图 complete-ground 已不再出现**；budget entityCoverageStatus=realized；旧 A-closure 保留历史 |
| 6 | `road-first land-use development remains incomplete` | **最大图 complete-ground 已不再出现**：development audit pass / incompleteReasons []；1 km 探针的供给问题另计 |

### 5.4 三个固定世界的实测（HEAD `74cdc1a` + A-closure）

| 世界 | 规模 | 关键数字 | 证据 |
|---|---|---|---|
| **1 km 探针** | 1000×1000m | **43 pad**（其中 15 户、24 店；需求 120 户 / 24 店）；自动路预算 24 段；入口未验证 **0**；gate **7 条**；由 `examples/b01-1km-probe-optin.manifest.json` 重放（Round 53：烘焙 17–18s、gate 约 9s；提交正文的 124.12s / 67.63s 保留原文不改） | `8d5b799`、`b01_1km_probe_optin`、Round 53 复现 |
| **山城 hillside-town-512** | 512m | **22 pad**，其中 7 块带削方台地；**道路削方 675 格 / 150.657m³**；全场削方 2,237 格 / 907.597m³；路面填方 5.590m；gate **4 条** | `hillside_cut_city.rs`、`d40c2da`、`6038c33` |
| **滨水 waterfront-city-512** | 512m | **22/22 户 + 4/4 设施**；60 条路 / **7 座桥**（273m 水面）；**岸线公共带 4,096m²**；湿面 19,968m² 逐格不变；gate **3 条** | `waterfront_city_512.rs`、`ecb174b`、`c8436b6` |
| **最大图 complete-ground** | 6000×10000m | 240 chunks / 623 entities；**gate failures 0 / status pass**；landcover unresolved **0**；ground cover **50,538,180m²**；ecology 342,380/342,380/0；generate 255.693s / gate 30.61s | `examples/maximum-6x10km-complete.manifest.json`；`rk-x/B05-MAXIMUM-COMPLETE-20261020.zh-CN.md` |

三条读法：

1. **滨水 23 户是历史值**：`ecb174b` 首测 23/23，`90bd9b0` 把商业排上码头后计划改为 22，HEAD 测试钉住 22/22。写 23 会与测试矛盾。
2. **山城的 675 格是道路削方**，与垫层削方分开计（全场 2,237 格 = 道路 675 + 垫层 1,562；`32356e1` 记 22 pad / 7 块台地）。
3. **1 km 探针现在可由仓库重放**：`examples/b01-1km-probe-optin.manifest.json` 是基础清单 `examples/b01-1km-probe.manifest.json` 的逐字节副本加四行声明——`frontageProgram.ruralConversion = "explicit"`、`frontageProgram.terrainPolicy = "bounded-cut"`、`frontageProgram.maxCutMm = 2000`、`landUseProfile.urbanGrowth = "axis-and-node"`；seed 7、origin、尺寸、zoning 与 demand 一字未动（`b01_1km_probe_optin` 的默认测试同时钉住"四个开关都在、且被生成器自己的解析器读成开"和"除开关外逐字段/逐字节相同"）。Round 53 用 release 二进制重放：**43 pad / 15 户 / 24 店 / 入口未验证 0 / gate 7 条**，两次 1 km 烘焙的 126 个文件中 125 个逐字节相同（只有 `generation-report.json` 记录的输出路径不同），烘焙 17–18s、gate 约 9s。跑法：`cargo test -p glyphweave-cli --test b01_1km_probe_optin`（默认两条，秒级），加 `-- --ignored` 会烘焙该清单并断言下列数字；已有烘焙世界时用 `GLYPHWEAVE_AUDIT_WORLD=<world>` 免烘焙。

   **与 `f641015` 正文的差异，以 Round 53 实测为准**：正文记"入口未验证 3 → 0、gate 七条、审计 fail → incomplete"，Round 51 实测为 1 / 8 条 / fail，Round 53 撤回问题地块后为 **0 / 7 条 / fail**。差值全来自同一个地块 `generated.residential-recovery.scene-0.3`：它是 `joint_frontage` 的住宅恢复地块（`services::append` 的 `housing-recovery` 请求）；工作区旧烘焙 `bevy/work/r32/acc-1km-growth`（未入库）里它已是三条未验证之一（46 pad / 18 户 / 24 店 / 8 条），与该提交正文的"之前"列一致。该提交只关掉了两条路边地块（`…roadside…98-99`、`…125-126`），这一条在计划期通过 `assigned_road_gate`，烘焙后却从正面临街点在 120m 预算内走不到任何一条路：`access.links` 记 `connected:false`、`roadId:null`、`status:"no_walkable_route_within_budget"`，而它计划期被指派的道路正是 `generated.road-network.scene-0.125-126`（与那条被拒的路边地块同一条路）。于是开发审计记 `no_verified_assigned_road_access`、联合审计记 `housing missing, altered or without assigned access`，`accessGraph.disconnectedEntities` 仍为 1，gate 的 `"scene-0" has unverified building access routes` 一行随之保留。Round 51 为 3 → 1、gate 8；Round 53 撤回该地块后为 3 → 0、gate 7。

### 5.5 B 的五个批次（当前状态）

| 批次 | 状态 | 本轮（Round 5 起）的可核对增量 | 仍缺 |
|---|---|---|---|
| B-01 独立地块几何 | ⚠️ 部分 | `parcel_geometry`、非对称 `parcelInsets`、yard/ledger、四朝向与 tamper-evident 测试已落地；纯 Rust 24/24；单区 CLI 5/24，双区 9/24；**512m joint-plains complete gate pass：18/18 住宅、4/4 服务、landcover unresolved 0** | 1km 沿路临街 0/24 仍是 B-02 供给问题；24/24 的临街压力探针未闭合 |
| B-02 需求与道路容量 | ✅ | tier id/role 持久化到 edge/extension/entity grounding；`roadTierAudit` 进 gate；独立 512m 控制 3 条自动路（collector+local+local），frontage 24/24 realized-with-access，road/tier audit pass，disconnect 0 | 旧 0/24 控制保留为反例；1km 120 户是输入/供给压力上限 |
| B-03 用途—实际覆盖闭合 | ⚠️ 部分 | 转换机制 + 账本守恒（`bac3b76`）；residual audit 按 source/sub-use 输出 candidate/target/realized/policy remainder/rejected/unresolved；当前 b03 控制 unresolved **128,542m²**；A-closure 最大图未实现 **50,538,180m²** | 历史 land-execution-2 的 123,760m² 是旧 worldRevision；显式子用途、农村公共空间、共享农路/等高条带、development↔landcover 映射、全局 1m 重叠审计仍缺 |
| B-04 形态与公共服务 | ✅ 核心完成 | 六个显式 profile、48 条案例索引、公共开放空间、退界派生、轴/节点生长；`heightM` 已驱动默认建筑高度；village-field 800m 覆盖 18/18 户、4/4 服务；compact 24/24 地块 coverage `pass` | coverage 是只读审计，不强制缩放作者尺寸；serviceRadius 不是人口/容量模型 |
| B-05 集成与生态尾差 | ✅ | 多 seed L2 矩阵 14 次 512m 烘焙 / 194 检查；生态重排 + 24 世界 no-op；最大图 complete-ground 与工程版均 gate pass；第二次烘焙 worldRevision 相同、1777 files 逐字节一致；HTML 最大图战略/2.5D 运行无 console error | 偶发进程访问违例仍属 A 稳定性风险，不阻断 B 内容链 |

结论：**B 的核心内容链已完成**。1 km 的城市天花板仍是 40,898m² 的分配额，
山城的天花板是单机 10 个临街地块（`0d3be23`），两者都不是实现缺陷。

### 5.6 C 的实际状态

C-N1 已产出逐函数来源矩阵与 `ecology_habitat` 纯接口；C-N2 仍是近似。
C-E1 已在山城 512m 控制生成 **232 段挡墙 / 5 个涵洞记录**（`terrain-works.json`），
C-E2 已输出自然 vs 最终地形的路由对比（87 个改向节点 / 5 个抬升终端），
不再是"只有 fill + 道路 profile"。仍缺挡墙基础、排水沟、竖曲线、洪泛/水量模型；
C-H1/H2 的核心自然/工程关系已由分区、地块、设施、公共空间和工程反馈覆盖；
C-L1 的 workerd 与 Quaternius 来源均已解析；**C-G1 已完成**：最大图工程版
1734 挡墙 / 734 改向节点 / gate pass；**C-V1 已完成** Godot 4.7.2 runtime
smoke（scene-0 / chunk 0,1 / loaded 6 / no script error）。C 核心里程碑完成。

### 5.7 2026-10-21 五项可见问题控制

现代湖滨控制 `modern-lakeside-city-512-v6-20261021` 已烘焙并实测：

- 305 entities；29/29 roads、27/27 frontage sites、接入断点 0；
- 68 个显式 tree、1,157 株生态植物（tree 698 / bush 449 / reed 10）、
  湖面 21,419m²；
- `scale-audit pass`、`road-network-audit pass`；
- `quality-report status=warn`，剩余 966m²（约 0.37%）庭院坡地未实现面，
  不冒充全场景 landcover pass。

白色伪影已证实为地形顶点色 sRGB/线性空间重复编码；草场圆形已由
`uniformDensity` 移除；Rust 规划的店铺/住宅块/高层恢复各自渲染分支。
证据与截图见 `rk-x/FIVE-ISSUES-CLOSURE-20261021.zh-CN.md`。

### 5.8 2026-10-21 迭代式新老城区

`iterative-new-old-city-512` 已烘焙并实测：

- 新增 `urbanDistrictProgram`，按中心距离和有序建设阶段展开既有 frontage 请求；
- established / renewal / new 实现 11 / 11 / 9 个地块，平均中心距
  67.51 / 128.40 / 189.33m；
- 商业从中心向外为 7 / 6 / 4，外围以住宅与公寓块为主；
- 道路 29/29、地块 31/31、scale audit pass；`quality-report warn`
  仍来自 955m² 庭院未实现面和 structured-water planner 口径；
- 新模块 `settlement/district_program.rs` 与 `urban-district-audit.json`
  提供机器可复核的阶段结果。

证据见 `rk-x/ITERATIVE-DISTRICTS-20261021.zh-CN.md`。

### 5.9 2026-10-21 自然到现代城市重构审核稿

用户补充要求把主链改为“先完成无人类痕迹的自然世界 → 人类沿山谷/渡口建立
非直线道路 → 聚落涌现 → 多阶段新旧城 → 功能设施与生鲜市场辐射”。审核稿
`rk-x/NATURAL-TO-URBAN-REDESIGN-REVIEW-20261021.zh-CN.md` 已补：

- 48 个核心区样本的自然地理、土地发展层和适合切面，数量为中国/美国/欧洲
  各 12、日本/香港各 6；
- 大都市只生成特征切面，小县城才允许完整聚落；
- 建筑数量改为住户/就业/商业面积与地块容量的结果，禁止固定 count 凑数；
- `founding → old-core → expansion → industrial → modern-new → renewal`
  的保留/拆合/更新链；
- 基于真实步行/道路/公交网络时间的生鲜市场容量、竞争与缺口模型；
- 城市规模、特区、旅游、山地、填海等因子对宏观工程强度的影响，同时明确
  不放宽 6m 填方、8% 纵坡、25% 步行坡、2m pad 削方等局部硬合同。

48 条记录已从 Markdown 转成 `assets/data/urban-case-priors-48.json`；
`node scripts/check-urban-case-priors.mjs` 验证数量、地区分布、ID 唯一和字段
完整性。

当前 P0 纯自然切片已通过：山地湖谷样例 3,426 个实体全部属于自然白名单，
`humanTraceCount=0`、地形工程/道路/开发计划均为 0，scale audit 与 quality
report pass，109 个可比数据文件复烘焙一致。证据见
`rk-x/NATURAL-ONLY-P0-20261021.zh-CN.md`。

P1 地形道路切片也已通过：16m 地形搜索、转弯/坡度/水体代价、曲线平滑、
8% 纵坡、6m 地形填方、bridge/viaduct 显式记录；山地样例两路 pass，
108 个可比文件复烘焙一致，见 `rk-x/ROUTE-EVOLUTION-P1-20261021.zh-CN.md`。

P3 容量规划切片也已通过：需求为 180 户、60 岗位、1,200m² 商业、2 服务
预算，最终由 93 个候选地块产出 15 栋建筑和完整 `unmetDemand=0`；109 个
可比文件复烘焙一致，见 `rk-x/SETTLEMENT-CAPACITY-P3-20261021.zh-CN.md`。
该计划尚未转回最终 AccessGraph 实体，因此只记为“规划切片完成”。

P5 市场网络切片也已通过：260 总需求由 280 容量市场沿道路网络覆盖，
15 个需求点全部有最近市场且缺口为 0，110 个可比文件复烘焙一致，见
`rk-x/MARKET-CATCHMENT-P5-20261021.zh-CN.md`。

P4 静态规划史切片已通过：5 个历史基座、3 次 modify、2 次 demolish、
2 次带父 ID replace，15/15 最终建筑有 cohort；111 个可比文件复烘焙一致，
见 `rk-x/EPOCH-EVOLUTION-P4-20261021.zh-CN.md`。

P6 宏观工程预算切片已通过：town 土地压力 0.59、工程系数 0.9515，
2 段路堤和 3 段高架进入预算账本，局部硬合同保持不变；113 个可比文件
复烘焙一致，见 `rk-x/ENGINEERING-SCALE-P6-20261021.zh-CN.md`。

P7 单地理完整链控制已通过：自然、道路、15 栋容量建筑、epoch、市场、
5 项宏观工程共享同一 bake；scale/quality/visual pass，113 文件复烘焙一致，
见 `rk-x/FULL-CHAIN-P7-20261021.zh-CN.md`。

统一排水/生态/建筑基础工程账本、P3 最终 AccessGraph 接入和 48 城全量
机器数据/烘焙仍未完成。
审核稿 §20 明确列出这些边界。

### 5.10 2026-10-21 核心城区设计审核稿 V2

针对“画面仍不像现代社会、建筑数量应变化、先自然后道路再聚落”的进一步反馈，
新增审核稿：

`rk-x/CORE-CITY-NATURAL-EVOLUTION-PLAN-REVIEW-20261021.zh-CN.md`

该稿把审核标准收紧为：

- 48 城只取核心切面机制，禁止用全市平均密度直接铺图；
- 普通建筑取消固定完成数，实际数量由住户/就业/商业/服务需求、地块容量和
  接入条件产生；
- 旧城、更新带、新区、郊区、旅游、放牧、种植和保护地使用不同空间与设施规则；
- 中心性改为历史、道路、交通、就业和公共设施组成的网络场，不再使用同心圆；
- 生鲜市场按容量、真实网络时间、竞争、负载和缺口生成辐射密度；
- 填海、开山、台地、桥隧等只通过显式宏观工程和城市系数管理，局部硬合同不变。

状态：**待项目负责人审核，审核前不继续扩大普通城区实现范围**。已有 P0–P7
切片保留为技术证据，不视为该审核稿的最终验收。

### 5.11 2026-10-21 核心城区 V2 生成链接入

用户进一步要求把“地理背景→自然过程→大都市切面→城市规划史→土地压力”正式
接入生成链。当前已实现：

- `naturalGeography.mode = derived`：由 setting、relief、climate、seed 确定性
  派生构造、水文、海岸、气候、风蚀、冰川、喀斯特和滑坡过程；
- 48 城先验升级到 v2：自然地理、地貌过程、三段历史、旧/更新/新/边缘结构、
  中心体系、社区类型、特殊因子和压力先验；
- Rust Core 可直接读取 48 城并输出自然草图、urban slice 和压力配置；
- `urbanSlice.semanticsVersion=2` 要求切面声明表达关系、主体锚点、上下文锚点
  和场外连接；
- `planningHistory` 以路线网络为中心，输出道路拓宽、旧城保留、再开发候选和
  公园需求；
- 社区规则按区域和旧/更新/新区输出产权结构、停车策略和街面策略，区分中国
  封闭小区、城中村、美国收入混合走廊、欧洲周边街区、日本町家/站城和香港
  屋邨；
- 土地压力/工程系数由城市规模、网络中心、地形稀缺、保护、特区、旅游季节等
  因子组成，局部硬合同不变；
- 道路计划和容量建筑计划已经转成最终 `EntityInstance`，进入 access graph。

核心切面 `derived-core-city-slice-1km` 实测：3,538 实体、147 道路段、86 段
内部道路拓宽、1 个最终公园、1 个混合中心广场、6 栋最终建筑、6/6 入口连通、
断点 0；2 栋计划建筑和 1 座金融中心塔因严格入口合同失败被具名记录；scale
audit 与 quality report 均 pass。证据见
`rk-x/CORE-CITY-PIPELINE-V2-20261021.zh-CN.md`。

仍未完成：48 城代表性全量烘焙、金融塔入口工程、政治中心专属建筑组、
多层级市场实体、统一排水/挡墙/生态/基础工程账本，以及高密度最终视觉。

### 5.12 2026-10-21 核心城区 V3 审核稿

针对“建筑体量仍像固定模板、道路仍像预设路线、城市中心密度不合理、生鲜市场
只有单层模型”的反馈，新增审核稿：

`rk-x/CORE-CITY-V3-NATURAL-HUMAN-EVOLUTION-REVIEW-20261021.zh-CN.md`

该稿把正式生成顺序固定为：

```text
N0 纯自然基线
→ R0 自然节点与地形成本路径
→ S0 聚落候选
→ U0/U1 城市骨架与 epoch 迭代
→ F0 功能建筑
→ M0 多层级生鲜市场
→ E0 宏观工程回写
→ B0 实体化与完整审计
```

它同时明确：

- 普通建筑取消固定完成数，数量由需求、地块容量、建筑原型和入口条件产生；
- 旧城、更新带、新区、郊区、旅游、放牧、种植和保护地使用不同街网、地块、
  建筑与服务逻辑；
- 通过 48 城机制先验指导生成，但现有 48 条仍全部是
  `heuristic-core-cut`，不能冒充官方统计；
- 48 案来源入口已全部审计，下一步逐步推进核心切面米制测量与回归；
- 生鲜市场拆成便利店、社区菜场、邻里超市、大超市、周期市场和批发市场，
  辐射由真实网络时距、容量、竞争、负载和季节需求共同产生；
- 局部 6m 填方、8% 纵坡、25% 步行坡和 2m pad 削方合同不放松，
  开山/填海/台地/桥隧必须进入宏观工程账本；
- 每个中型工作包完成后提交并推送，同时更新本表、HANDOVER 和验收记录。

状态：**待项目负责人审核；审核通过前继续扩大普通城区实现范围属于越界。**

W0 来源调查已开始：48 个代表核心切面均已有至少两个不同发布者的公开来源入口，
81 个来源已登记 URL、访问日期、直接访问状态和支持断言；全部 48 案均为
`source-audited-core-cut`，48 个核心窗口均完成首轮 OSM 观测：可驾驶道路
中心线密度中位数 20.31 km/km²（12.10–36.70），建筑覆盖率中位数
0.3165（0.022–0.569）。42 个窗口可路由市场型设施，市场型中位步行时间
中位数 4.40 分钟，10 分钟覆盖比例中位数 96.1%；北京、上海、芝加哥、
东京湾、拉斯维加斯和华盛顿 DC 窗口因设施标注不足记为数据缺口，不记为
零供给。这些只是当前地图快照，尚无案例达到完整 `measured-core-window`。
机器数据与校验：

历史阶段由 `scripts/summarize-urban-history.mjs` 复算：40/48 首段为水港/
河湖/治水，26/48 中段出现铁路/轨道，26/48 第三段出现更新/保护/重建。
该统计是关键词辅助总结，不替代逐案历史考证。

土地压力由 `scripts/summarize-urban-pressure.mjs` 汇总。样本显示地形稀缺
不随城市规模单调上升：city 中位 1.42、megacity 1.38、town 样本 1.72。
填海组地形中位 1.52、制度压力中位 1.30；山地组地形中位 1.48。当前
`.20` 山地、`.18` 填海、`.22` 特区权重与样本方向不冲突，但仍是待做
sensitivity test 的先验，不是回归结论。

基线验证：`cargo test --manifest-path bevy\Cargo.toml -p glyphweave-core --lib`
于 2026-10-21 实测 `854 passed; 0 failed; 34 ignored`，耗时 `478.09s`。
ignored 均需要已发布世界或显式单次实验。

- `assets/data/urban-case-evidence.json`
- `assets/data/urban-core-measurements/`
- `assets/data/urban-history-patterns.json`
- `assets/data/urban-pressure-patterns.json`
- `scripts/check-urban-case-evidence.mjs`
- `scripts/check-urban-core-measurements.mjs`
- `scripts/summarize-urban-history.mjs`
- `scripts/summarize-urban-pressure.mjs`
- `rk-x/URBAN-CASE-EVIDENCE-W0-20261021.zh-CN.md`

### 5.13 2026-10-22 硬占用、斜向 yaw 与城市剖面闭合

用户复核发现“拟真内容增加后硬检查反而失守”：建筑压在道路上、道路或建筑
视觉悬空、相邻房屋碰撞、街景剖面内容过少。实烘焙与浏览器复验定位到以下
真实缺陷并已修复：

- `spatial_half_extents` / `oriented_local_coordinates` 曾把 45°、135° 等
  任意整数度误判为轴对齐，现仅对 0/90/180/270° 使用正交优化；
- 规则碰撞曾因“任一对象是道路”而跳过建筑—道路重叠，现将道路预装入
  occupancy，仅允许双方均为共享交通面时重叠；
- `quality-report` 现将多阶段工程放置报告中同场景且含 `inputItems` 的权威
  reconciliation 记录交给独立重算；
- `final_scene_audit` 对最终建筑—建筑 OBB、建筑—道路 OBB、完整地形支撑、
  双向接地、地形穿入、落水和道路 profile 失败关闭；
- `?view=street` 加载邻接 Chunk，路线演化按同一 polyline 绘制连续路面、
  人行道和中心线，不再重复叠绘 8m/32m 分段实体。

香港 1km r27 证据：

```text
bevy/work/rk-x/v3-hard-audit-hk-r27-20261022
```

- 2,901 实体、4 Chunk、53 栋建筑、200 段道路、14 段楼梯；
- 483 树 / 310 灌木 / 1,427 草丛 / 412 岩石 / 1 河流；
- 5 个分层市场，覆盖率 1.00；
- 建筑—建筑、建筑—道路、悬空、穿地、落水、不支持 footprint、非法道路
  profile 均为 0；
- AccessGraph 53 连通 / 0 断点；
- 最大 Chunk 边界高差 0.957m；
- `scale-audit pass`、`quality-report pass`；
- 最终 workspace 回归 `1,140 passed / 0 failed / 37 ignored`；
- Computer Use 战略图、街景、探索三视图无空屏，HUD 为
  `1m geometry / 1m data`；`visual-feedback.json` 为 `pass`，包含视觉反馈
  后 quality-report 仍为 `pass`。

运行入口：

```text
http://127.0.0.1:8131/preview/index.html?view=strategic
http://127.0.0.1:8131/preview/index.html?view=street
http://127.0.0.1:8131/preview/index.html?view=explore
```

采用版本：`c965dc6 fix(worldgen): enforce road-first exact occupancy`。

### 5.14 2026-10-22 生鲜市场辐射与缺口闭合

市场计划从单一覆盖比例扩展为三份可互相校验的权威产物：

- `market-catchment-plan.json`：市场、需求点、最近市场、负载和总覆盖；
- `market-radiation.json`：5/10/15/30 分钟覆盖、逐需求分配、竞争压力、
  市场利用率、分配需求和容量/需求比；
- `market-deficit.json`：逐需求缺口、最近市场和
  `no_market_within_travel_limit` / `eligible_market_capacity_exhausted`
  原因。

`quality-report` 现在重新读取三份文件并验证需求/服务/缺口守恒、市场分配
守恒、容量上限、时间带顺序和缺口记录数量；伪造总覆盖率失败关闭。

香港 r28 实烘焙：

```text
bevy/work/rk-x/v3-hard-audit-hk-r28-20261022
```

- 5 个分层市场；
- 总需求 4,250.320，服务 4,241.696，覆盖率 0.997971；
- 5/10/15/30 分钟覆盖 `0.582 / 0.821 / 0.998 / 0.998`；
- 唯一缺口 8.624，原因为容量耗尽，具名保留；
- `scale-audit pass`、`quality-report pass`；
- 两次烘焙 128 个可比较生成文件一致。
- 最终 workspace 回归 `1,141 passed / 0 failed / 37 ignored`。

采用版本：`b3b5502 feat(worldgen): publish market radiation evidence`。
详见 [`rk-x/MARKET-RADIATION-20261022.zh-CN.md`](rk-x/MARKET-RADIATION-20261022.zh-CN.md)。

---

## 6. 当前卡点与待决策项（更新于 2026-10-19）

> **10-18 的两条 P0 已关闭**（`0fe6ebd`、`7a713c2`；`cargo test --workspace` exit 0，两条金标未动）。
> 此后瓶颈转移了两次：沿路用途（Round 4 的 87–96% `wrong_purpose_or_protected`）→ 工程支撑/填方
> （Round 9：233 例中 211 例需挖方）→ **作者输入上限**（1 km 的 4% 城市分配、山城的 24 vs 10）。
> 现在**没有未修的已知代码缺陷卡住主线**；下面 7 项中 **1 项已关闭、6 项仍待拍板**。

### 6.1 决策表

| # | 事项 | 现状（可核对） | 影响 | 需要谁决定 |
|---|---|---|---|---|
| 1 | 授权一次 **6×10km 烘焙**，关闭 A 例外 #4 | **已关闭（2026-10-20）**：255.941s、240 chunks、623 entities、生态 `342,380 / 342,380 / unfilled 0`；gate 仍 fail 4 项，均为 land-use | 不再阻塞 C-G1；land-use 缺口转入 B-03/B-05 | 已执行，无需再决定 |
| 2 | 1 km 是否提高城市分配（`urbanCoreRatio`，现 0.02） | 规划城市 40,898m² ≈ 场景的 4%（`9102d48` 写的"4% urban land"即此）；已用 40,395 + 503 unresolved，**分配额已耗尽**（`5cd21b2`）；实测 15/120 户 | 120 户需求的**结构性上限**；不提高就长期停在 15 户量级 | 项目负责人 + 需求方（改输入还是改需求） |
| 3 | 偶发访问违例（0xC0000005） | 未修；**~4%**（27 次中 1 次）；崩溃点在 road-network 计划 JSON 序列化；仓库**零 unsafe** → 疑依赖的 unsafe（blake3 SIMD / serde_json）或编译器；取证工具已入库 | 任何"套件全绿"都不能作为签收证据 | 项目负责人（继续取证 / 换工具链 / 接受随机红） |
| 4 | 山城临街 **24 需求 vs standalone 上限 10** | 实现 7/24（17 未满足）；诊断工具修好后结论是"搜索已穷尽"：84 次尝试只用 0.66% 预算，拒绝 24 水 / 25 占用 / 6 改平台 / 4 削方环 / 1 走错路（`0d3be23`） | 若 24 是需求 → 必须改输入（更多分级路、窄河道、更小临街面、更密站点）；若是探针 → 接受 7 | 项目负责人（需求归属） |
| 5 | 其余仍 fail 的 gate 项算不算 B 的验收阻塞 | 滨水 **3 条**（frontage demand / purpose-area / landcover）、山城 **4 条**（多一条自动路目标）；1 km **7 条**，可由 `examples/b01-1km-probe-optin.manifest.json` 重放、并由 `b01_1km_probe_optin` 钉住条数（5.4 读法 3） | 决定 B 能签收到什么程度；也决定要不要动 50,538,180m² | 项目负责人（验收口径） |
| 6 | 有界削方是否默认开 | 现为 opt-in（`MAX_CUT_MM = 2000`），关闭态逐字节不变；山城 675 格道路削方、1 km 已有真实削方台账 | 默认开 = 把"只填不挖"改成"有界削填"：旧世界地形全变、须 bump revision | 项目负责人（数值合同） |
| 7 | W1 供给基线 pins 要不要改 | `append`/`diagnose` 新序实测 (148,3)/(148,1)，冻结测试仍钉旧序的 (148,4)/(148,0)（`32330cb`） | 不改则两条实现长期不一致；改则是契约变更 | 项目负责人 |

### 6.2 两条纪律（不是决策）

1. 崩溃未修前，`993 passed / 35 ignored`（`0d3be23`）是**单次**结果；验收须连续多次运行，不得把单次全绿当签收。
2. A 例外 #4 已由新的 6×10km 烘焙关闭；`real_world_dir_linear_indexed_ecology_equivalence`（`#[ignore]`）仍以旧发布世界为断言。若以后改走重放路线，必须随契约更新，不能保留旧的 342,379。

### 6.3 已由"默认关"解决的旧决策

`DECISION-COST-SHEET`（`11728e0`）里最贵的两项已落地，但**都没改默认语义**：1 有界挖方以
`style.frontageProgram.terrainPolicy = "bounded-cut"`（`3f6bb02`）、2 road-first 水体以
`allowStructuredWater = "bridge-or-causeway"`（`d72df65`）实现，默认关、旧世界逐字节不变。
所以这两项现在不是"要不要做"，而是"要不要默认开"（6.1 第 6 项）。同表的 3 coverageRatio、4 serviceRadiusM、
5 terrain、6 stationSpacingM 仍无消费者：6 代价最小，5 收益最低，建议顺序不变。

---

## 7. 缺口清单与优先级

按"挡住多少下游"排序，不按工作量排序。

| 优先级 | 缺口 | 挡住什么 | 建议动作 |
|---|---|---|---|
| P0（审核稿已出） | 生成链仍是“固定路网/frontage 数量优先”，未按“纯自然→沿谷路径→聚落→epoch”生成 | 现代感、合理性、新老城、市场辐射和建筑数量都会继续失真 | 审核 `rk-x/NATURAL-TO-URBAN-REDESIGN-REVIEW-20261021.zh-CN.md` §18；批准后按 §19 从 P0 纯自然快照与零人工痕审计开始实现 |
| P0 | 基础边 profile 可行性闸门（§6.1） | 整个 B-01 收口与后续所有建筑内容 | 执行 T-0013 |
| P0 | `reconcile` 遵守 `fallback`（§6.2） | 世界可烘性；任何不合规对象都能炸整图 | 执行 T-0013 |
| P0 | Patch 链（D5.3） | FUT-01/02/06、D14.4 编辑通道、R02 | 设计 `patchChainHash` 与 CLI `patch` 的真实接线 |
| P1 | 稳定 ID / 存量约束（D6.6） | 增量编辑、迁移、A-03 的真正达成 | 定义 identity 合同 + 迁移记录 |
| P1 | 1km 120 户压力供给 | 压力输入/预算，不再阻塞 B-02 接口 | 用更密道路/更大城市分配或明确需求上限；不得放宽合同 |
| P1 | 农田/未建城市地的地表实现（50,538,180m²） | B-03、A 例外 2/3/5/6 | 分 urban/rural/自然三类逐项报 realized/unresolved |
| P1 | 生态缺 1 株 | A 例外 4、C-G1 | **已关闭（2026-10-20）**：A-closure 6×10km 实测 342,380/342,380/0；保留证据，不再作为阻塞 |
| P2 | 主次支路分级、路口真实几何 | B-04、C-H1 | 需要水平曲线与转向几何 |
| P2 | 工程实体（边坡/挡墙/排水/涵洞）+ 工程后水文 | C-E1/E2 | 挡墙/涵洞/路由对比已落地，最大图工程版 gate pass；下一步补排水沟、竖曲线与消费者可视化 |
| P2 | 形态 profile 编译（D13） | B-04 | 先把 12 原型中的 2–3 个落到参数，不追求全覆盖 |
| P2 | Godot 碰撞/发布导出 | 后续发布工程 | 最小 runtime smoke 已通过；碰撞与导出模板不属于当前 ABC 渲染要求 |
| P3 | 多场景/转接点（D17） | R12、C-G1 | 单场景稳定后再谈 |
| P3 | 许可历史下载日期未证明 | C-L1 发布记录 | 来源包已定位；记录发布日期 CC0 与当前 QAL，不冒充历史日期 |

---

## 8. 需要项目负责人拍板的决策（不是执行问题）

1. **A 例外 1 怎么关**：24 段自动路是**内容供给预算**。是提高这个预算（需前瞻批准，且不能冒充计算预算），还是承认"有限供给"并写进需求？两者都合法，但不能默认 24 段等于够用。
2. **18 户是需求还是能力投影**：若是需求 → T-0013 的生成侧闸门是正解；若连"拒绝 grading 保平坦"也过不了，下一步只能动 `ENTRANCE_APRON_RUN_M`/`WALKABLE_GRADE` 这类**数值合同**，需要你签字。
3. **storefront 8m vs fixture 12m**：改测试期望，还是改 `assets/objects/storefront.object.toml` 的距离？这会立"资产合同 vs 测试"谁优先的先例。
4. **B 的完成是否包含 48 个案例**：文档只说是"研究/参数来源要求"。若要求每个案例可探索，B 的工作量翻倍；若只要求参数有出处，B-04 可以只做 2–3 个原型。
5. **50,538,180m² 缺口里"本质不可能"占多少**：A-closure 已用当前源码同口径重算为 9,461,820/60,000,000；下一步是把 urban/rural/nature/unassigned 的未实现拆成可建、不可建、缺接入与输入上限。
6. **G27～G34 的 viewer 维护责任**：B/C 一改生成，它们的语义可能失效，但现在是冻结状态。

---

## 9. 证据索引

| 类型 | 位置 |
|---|---|
| 验收/检验文档（212 份） | `docs/collaboration/acceptance/`（映射见 `moved-files.json`） |
| A 的机器证据 | `docs/collaboration/acceptance/a-milestone/evidence/` |
| 当前 1km 真实世界（b8） | `bevy/work/collaboration/ds-b-mainline-resume-20260929/runs/b8-current-a01/world/` |
| R1～R11 轮次证据（含 1km 世界） | `bevy/work/collaboration/ds-b01-t2-production-chain-continuation-20260929/r{2..11}/` |
| 当前 T 工单证据 | `bevy/work/collaboration/rk/T-0010/`～`T-0012/`（`T-0013/` 仅有 `pm/`） |
| 最大图历史世界 | `bevy/work/r43-maximum-baseline`、`r46-maximum-extensions-v2`、`r47-maximum-auto-final`、`r49-frontage-maximum` |
| 输入矩阵 | `examples/*.manifest.json`（32 份） |
| 1 km 探针的开启态（唯一可重放入口） | `examples/b01-1km-probe-optin.manifest.json` + `bevy/crates/cli/tests/b01_1km_probe_optin.rs`（默认两条测试校验开关与"除开关外无差异"；`-- --ignored` 烘焙并断言数字） |
| B-02 当前审计 | `docs/collaboration/rk-x/B02-CURRENT-AUDIT-20261020.zh-CN.md` |
| B-03 residual 审计 | `docs/collaboration/rk-x/B03-RESIDUAL-AUDIT-20261020.zh-CN.md` |
| 独立检查脚本 | `scripts/check-*.mjs`（frontage/render-stream/automatic-roads/road-corridor/…） |

---

## 10. 本文的边界与文档卫生问题

**我实际做了什么**：通读 A/B/C 合同、纠偏路线图、交接文档、调度板；精读 r27、r46、r48、r49、形态研究与地表实现三份；
其余 r23–r43、r44–r45 与 r47 及两份研究文档由只读抽取员逐份整篇读取并给出行号出处，我据此转述；
**一手核对过源码常量**：`WALKABLE_GRADE`、`ENTRANCE_APRON_RUN_M`（见 §6.1）、`CityForm` 的 6 种形态（`worldgen.rs:589-604`）、
`baked_audit` 的 26 个测试函数名，以及 b8 `world.json` 中 `patchChainHash` 为空串。
**我没有做**：独立重跑世界/gate/测试；未通读 Rust 业务源码；未复算任何 hash 或面积。

**文档本身的卫生问题（会误导后来者）**：

1. `r28-conservation-ground-surface.zh-CN.md` 正文标题写的是 **r32**，`generatorRevision` 也是 r32 —— 文件名与内容不一致。
2. r 系列**缺 r29–r36**，但 `20260906-natural-human-development-roadmap` 里引用了 r33/r34/r36 的内容。
3. `20260906-natural-human-development-roadmap` 第 10 行**自我校正**：第 20–52 节部分"已完成"声称与实际代码不符 —— 引用该文档时必须先读这一行。
4. 早期 `game-plan`/`game-todo` 与主线无交叉说明，容易被当成仍有效需求（D1.5）。
5. `20260902-acceptance-list-1.md` 是**规则引擎**的验收清单，与 B/C 地表链无关，容易误并。
6. 同一"上限"有三种写法：`6km x 10km`、`6 km × 10 km`、`6000 10000`。
7. `b8 world.json` 的 `patchChainHash` 为空串，说明 Patch 链未接生产；但多份文档仍以"Patch 可重建"表述需求，容易被读成已实现。

---

## 11. 2026-10-23 最新城市核心修订

最新可运行版本：`bevy/work/rk-x/v3-final-r201-20261023`。

本轮关闭的硬问题：

- SAT 旋转轴方向与 Three.js / `contains` 不一致，导致斜街与建筑的真实 OBB 相交被漏报。修复后非对称角度有回归测试，r192 最终建筑-建筑/道路/细节碰撞均为 0。
- 分块 carve 预过滤会在接缝一侧漏掉同一 fill ring，曾造成 4.643m 人为断崖。改为全分块共享全局 carve 集合并依赖 `CarveIndex` 局部筛选；r192 最大跨块高差 1.071m，scale-audit pass。
- 道路总路权现在包含车道、中央隔离、路肩和人行道；HTML 不再把道路画得比碰撞断面更宽。路口盖面由 1,156 个重复圆盘收缩为真实中心线交叉的去重盖面。
- 移除预览中叠加在真实建筑上的未旋转规划盒；修正店面前花箱进入 OBB、住宅基座外扩和潜在双重旋转。
- 内部街廓填充改为 12m 节奏；旧城/更新区/新城使用不同临街组合概率与进深范围，满足“不是固定大小盒子”的方向。

入口门禁已在 r201 关闭：

- 直接可达门面优先于可选楼梯，避免无效楼梯把已合法入口改判失败。
- 四轮修复后增加最终入口结算；无任何可验证入口的生成建筑及孤立 approach/stairs 被具名拒绝。
- `quality-report=pass`，311 栋建筑全部有入口，0 断连；建筑碰撞仍为 0。
- 代价是移除 18 栋无法建入口的生成建筑，建筑数由 329 降为 311，不以 warning 冒充交付。

仍未关闭：

- r201 有 311 栋可见建筑，中心 150m footprint 覆盖率 8.21%，仍低于中环核心样本；单体体量仍偏小。
- Rust 尚未发布完整路口并集多边形、桥面排水/护栏/桥头挡墙和纵向坡度碰撞插值。

证据：`docs/collaboration/rk-x/ROAD-SECTION-OBB-DENSITY-20261023.md`。

### 11.1 建筑入口闭环

`r201` 已关闭上一版遗留的入口质量问题：

- 临街推广楼梯从建筑门槛而不是立面旁地形起算，长坡按多段合法梯段发布。
- 修复“任一可选锚点失败就误删整栋建筑”的清理逻辑；只有所有入口都失败的生成建筑才被剔除。
- 增加 `rear` 服务入口，允许后方高台或道路承担真实接入。
- 最终 311 栋建筑全部具有完整步行入口，18 栋无任何合法入口的建筑按“数量是结果”原则不进入世界。
- `scale-audit=pass`，`quality-report=pass`，建筑-建筑/道路/细节碰撞均为 0。

证据：`docs/collaboration/rk-x/ENTRANCE-CLOSURE-20261023.md`。

