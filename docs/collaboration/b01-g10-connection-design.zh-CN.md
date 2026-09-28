# B01-G10 设计：station18 候选连接方式（设计就绪，未实施）

* 任务：`docs/collaboration/ds-v41f-b01-g10-connection-design-task.zh-CN.md`；上游审查：`docs/collaboration/b01-g9-local-support-provenance-inspection.zh-CN.md`、`bevy/work/collaboration/inspector-b01-g9-20260919/offline-owner-recovery.json`
* HEAD：`fb36dc4f9fdd38e6aa9c5a89676a65b96e5001b1`；**implementation = not_run｜naturalValidation = not_run｜realIncrement = none**
* 状态：`design_ready_for_review`。本轮自然 harness / plan 重建 / design / append / infer / measure / carve 重放 / realize / bake **全 0**；仅最小 cfg(test) 映射辅助 + 合成回归 + 离线 JSON/源码阅读。B partial、C 不推进。
* **不主张方案 B 唯一或必然可行**；下方给出的是"可用现数据表达但缺一项验证"与"现合同下存在明确必要矛盾"两类结论。

## 1. 工作 A：归属资料一次性收口

采纳检查员离线恢复（58 实体 / 79 base carves / 144 条数值关系逐项匹配），派生文件见 `bevy/work/collaboration/ds-b01-g10-connection-design/attribution.json`：

| 生产排序索引 | 真实 owner / component | 作用 |
|---|---|---|
| 2 | `generated.roadside...scene-0.10-11.1` / `pad`（旧住宅，中心(256,120)，half 13×13，prio 1，target 8m，blend 120m） | 北冲突 2450→**6502** |
| 49 | `generated.road-network.scene-0.25-26` / `road_pad`（旧道路，中心(320,224)，half 36×5.5，prio 10，target 8m，blend 120m） | 南冲突 5861→**6987** |
| 78 | `generated.road-supply.g9.ST18-R1.bc` / `road_pad`（新增 BC，half 60×7，**有 profile**） | 两控制点 →**2230**（= BC 在 x=268 即 station 20 的自身 profile 值） |

作废字段：G9 run 的 `ownerEntityId/ownerKind/component`（全部）、`schema.ownerMappingVerified`、`candidateSites.roadCarveBlendM` 与 `cells[].bandNotes.bcRoadCarveBlendM`。**BC 真实 blend = 97.6259999871254m**（约 97.626），120 属于其它 carve；两冲突点仍在真实 blend 范围内，核心机制不变。哈希口径：SHA256 一律对文件原始字节；run 内 BLAKE3 对**规范化 JSON 值**（例如 reservation 的文件字节 BLAKE3 为 `686e9cf0…`，run 内记录的 `7715eabd…` 是同一文本的字符串编码值）。源码分版：`runGeneration = pre-mapping-fix`（**无快照，故该修订的 hash 记 unknown**），`currentSource = post-mapping-fix`。

## 2. 工作 B：连接方式设计（station18 北/南，固定 ST18-R1）

### 2.1 平面与高程定义（把三类值分清）

| 要素 | 取值 | 来源 |
|---|---|---|
| 建筑入口（前锚点） | 北 (270,163) / 南 (270,197)；该处必须落在 pad 上 | `road_network.rs:130-147`（front = depth/2+1 = 6） |
| 连接起点→终点 | 前锚点 → 道路点 (270,180)（BC 中线上的站点） | `frontage::candidate_site` 的 `frontage_road_point` |
| 连接段（现有实现） | carve 中心 (270,171.5)/(270,188.5)，half (2,10.5)，blend 2，**priority 2，target = 建筑 ground height = 2390** | `worldgen.rs:4315-4328`；G9 记录几何 |
| pad 设计高程 | **2390mm**（= BC profile station18） | G8/G9 实测 |
| 被保护的旧采样 | 北 **6502**、南 **6987**（距轴 9/8m，位于道路 carve 的 blend 环内、整平带外） | G9 实测 + 本轮归属恢复 |
| 路面/整平带值 | 7m 边界处 **2230**（BC 自身 profile，station 20）；路中是 2390（station 18） | 本轮归属恢复 |

**关键区分**：6502/6987 **不是路面高程**，而是旧住宅 pad / 旧道路 pad 的**过渡环填方**；2390/2230 才是 BC 自身的硬支撑面（profile）。三者不可混用。

### 2.2 保持旧支撑 vs 真实连通：现合同下的必要矛盾

* 现状（不裁切）：连接 carve 在 8/9m 处达到完全覆盖，`full_target` 分支（`worldgen.rs:4518-4521`）把 6502/6987 **改写为 2390** → 旧支撑被改变（正是 `existing_support_changed` 拒绝）。
* 若把连接 carve 裁到不覆盖 8/9m 带：那 2m 的带状区域保持 6502/6987（比 pad 高 **约 4.1–4.6m**），而连接必须是**连续工程面**才能走通——两条要求直接冲突。
* 现有合同**没有**"台阶/梯道"概念可用于跨这种高差：`access.rs:334-337` 要求入口与立面的最终栅格与实体 `ground_height_m()` 之差 **≤0.30m**，其后 `segment_clear` + `route` 要求存在到道路的可行走路径。要让 2390 的入口跨过 4m 高的旧填方再落到 2390 的路面，当前几何与语义都表达不了（除非把 pad/profile 抬到接近旧填方面 ~6500，那超出已批准范围）。
* 结论：**"保持旧支撑 + 真实连通"在现合同下构成明确必要矛盾**，不是"少触发一次拒绝"能替代的问题。以下方案各自指出它改变了什么。

### 2.3 备选设计与其代价

| 方案 | 内容 | 是否保旧支撑 | 是否真实连通 | 变更面 |
|---|---|---|---|---|
| B1 连接目标改为路面 profile 值 | 连接 target = 所在 station 的 BC profile（2390） | **否**（仍会覆盖 6502/6987） | 是 | 不改合同，但等于现状 |
| B2 裁短/收窄连接 carve | 只覆盖路面平带内、不触及 8/9m | 是 | **否**（2m 带 4m 高差无工程面，`route` 无法成立） | 不改合同；**牺牲连通** |
| B3 连接改为"绕行"沿整平带内侧接入 | 连接沿 BC 平带（|Δz|≤7m）纵向进入，只在平带内覆盖 | 是 | 取决于入口能否到达平带边缘：入口在 z=163/197，与平带边缘(173/187)之间仍有 8/9m 的 blend 带要通过 → **同样受阻** | 不改合同；需新的几何证据 |
| C 显式合成语义（候选硬支撑不丢弃既有过渡填方 / 只在自身 parcel 内生效） | 修改 `apply_terrain_carves` 末段语义 | 是 | 取决于改造后最终栅格：若保留 6502/6987，则 B2 的高差问题依旧；若允许局部开挖则改变 no-cut 语义 | **合同级**，改变既有世界生成结果，需新版本 + 全量重跑 + 旧证据标注 |
| D 收窄保护窗口 | 把道路保护带改小 | 是（不再是保护对象） | 不改善 | **合同级**，削弱既有保护，不建议 |
| E 抬 B/C、改 setback/院落/建筑尺寸、放开 fill/no-cut | — | 越界 | — | 不采纳 |

**因此本轮不推荐把 B 当作已可行方案**：B1 与现状等价、B2 牺牲连通、B3 仍缺证据；真正的决策点在 **C（合成语义）** 或 **接受该类地块被拒**。若后续仍希望走 B 路线，必须先回答"2390 的入口如何跨过 4m 高旧填方"这一必要问题。

### 2.4 接口/版本影响（若日后选 C）

* 表达性：现有数据模型**能**表达 B2/B3 的几何（connection carve 的 half/blend/target 已是输入），但**不能**表达"保留邻接过渡填方"这一语义——需在 carve 合成层新增显式语义（例如区分 `hard_support` 与 `transition_fill` 并定义覆盖规则，或限定候选硬支撑的作用域为自身 parcel）。
* 最小变更面：`worldgen.rs::plan_terrain_carves` / `apply_terrain_carves`（合成语义）+ 可能的 schema 字段（carve 语义标签）；消费者为 frontage 前检、road audit、`candidate_final_terrain`、bake。
* 版本/旧证据：该变更会改变**既有世界**的最终栅格与审核结果，必须绑定新 revision/version，旧证据按原版本保留并标注；不得默默重解释旧 run。
* A（里程碑）状态不由本 worker 修改；本 worker 无签收权限。

## 3. 工作 C：待批实验方案（本轮不执行）

**首选（唯一首选，1 个备选）**

* 假设 H-G10-1：把连接 carve 的语义改为"只抬高、不丢弃邻接过渡填方"（即候选硬支撑不得降低既有采样）后，station18 两窗的 `existing_support_changed` 消失，且**入口仍可通行**（`access` 通过）。
* 反例（预期至少出现其一）：(a) 旧支撑保持但 `access` 因 4m 高差失败 → 证明 B 路线不可行，应转 C 的显式语义或接受拒绝；(b) 旧支撑仍被改变 → 说明覆盖来自其它分支，需重做归属；(c) 入口不可达/没有到道路的路径 → 记录 `link.status`。
* 最小规模：**1 次自然 harness、1 个状态（ST18-R1）、2 个候选（北/南）、4 个既有坐标 + 必要的入口/路线点**；不要 24 个需求、不做全场。
* 一次输出必须含：4 点 before/after 与其 owner 签名（用**修好的**身份绑定）、两候选的 `assess_detailed` 结果与 `link.status/path/length`、以及（仅当预测有增量时）baseline ≤1 + candidate ≤2 的完整审核与逐 parcel yard/access 绑定。
* 停止条件：任何自然失败/序列化不完整立即停止；两次候选均无增量则如实交回；不得为补字段重跑。
* 备选（仅 1 个）：B3 的整平带内侧接入几何，同样 1 状态 2 候选，先只检查 4 点保持与 `route` 结果，不进入 realize。
* 需要主检查员批准的原因：H-G10-1 需要**改生产合成语义**（合同级）；若只做"几何/采样验证"则可在不改生产代码的前提下先做一次**只读**诊断（同样需 1 次自然额度）。两者都涉及新的自然运行，本轮不自动执行。

## 4. 边界与文件

* 本轮 0 自然调用、0 生产源码/接口/默认/carve/no-cut/fill/保护/需求/地形修改；无 Git/agent/F/A；未 realize、未 bake、未发布世界。
* 新证据目录 `bevy/work/collaboration/ds-b01-g10-connection-design/`：`attribution.json`、`synthetic-identity-binding.log`、`synthetic-g9-regression.log`、`source-hashes.txt`、三个检查日志、`receipt.json`。
* G9 结果文档只追加勘误（owner 标签作废、blend 97.626、控制点 2230 = BC 自身 profile、A 状态措辞）；旧 G9 run/receipt 不改。

## 5. 勘误（B01-G10-R1 轮追加，2026-09-19）

独立审查 `b01-g10-connection-design-inspection.zh-CN.md` 保留归属收口与同优先级互换回归，但**不批准 H-G10-1 与任何自然实验**，并指出 §2.2 误用 access 合同。本节只追加，旧 receipt/attribution 不改。

1. **0.30m 只作用于 exit_point 与 facade 两点**（北 (270,162.85)/(270,162)；南 (270,197.15)/(270,198)），**不检查整条路径**；路径另有 `HALF_WIDTH=0.75m` 中心与两侧检测带、2m 网格 8 邻域、坡度 ≤0.25、`MAX_LENGTH=120m`、`MAX_EXPANSIONS=6000` 的有界寻路，并要求连到指定 BC。
2. **x268 两点不在入口点、也不在 x270 ±0.75m 检测带内**，它们只说明"对应局部段很陡"。§2.2 的"0.30m 限制使旧肩部高点必不可跨""B2 牺牲真实连通已证明""保持与连通存在明确必要矛盾""只有 C 或拒绝"**全部撤回**；正确状态为 **access 未验证**（既未证可达，也未证全域不可达）。
3. **保护规则要求旧 support 完全相等**（不是"不下降"），且候选 pad 必须**整块**等于 2390。H-G10-1"只保留填方"不自动满足这两条，更不能由四点通过外推到 access 成功；故 H-G10-1 **未获批准**。
4. **内部 `TerrainCarve` 的 half/blend/target ≠ manifest 可配置接口**：manifest 只有 `automaticRoadExtensions` / `frontageProgram` / `roadExtensions` 等作者声明，没有"连接作用域/语义"字段；B2/B3 的几何可表达但需明确规则来源，C 需 schema + 编译 + 前检 + audit + final terrain + bake 的一致变更与版本绑定（详见 R1 文档 §3）。
5. **profile 身份绑定已补强**：原 `verify_carve_identity` 只比较"是否带 profile"，现比较全部字段（version/width/start/end/maxFill/elevations），并新增"非中心单采样 +1mm 必须拒绝"的合成反例（`g10_r1_preflight_profile_content_binding`）。
