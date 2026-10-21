# 交接文档（GlyphWeave · B/C 里程碑推进）

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
