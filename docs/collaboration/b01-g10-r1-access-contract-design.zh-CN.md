# B01-G10-R1：access 合同纠正与连接设计修订（设计就绪，未实施）

* 任务：`ds-v41f-b01-g10-r1-access-contract-design-task.zh-CN.md`；上游：`b01-g10-connection-design-inspection.zh-CN.md`、`inspector-b01-g10-20260919/evidence.json`
* HEAD `fb36dc4f…`；**implementation = not_run｜naturalValidation = not_run｜realIncrement = none**；状态 `design_ready_for_review`。本轮 0 自然调用、0 生产语义/阈值/接口改动、未 realize/bake、未重跑 G9；B partial、C 不推进。

## 1. T1：撤回过强结论，准确写 access 合同

| 项 | 取值/规则 | 来源 |
|---|---|---|
| authored anchor | 北 (270,163)、南 (270,197) | `road_network.rs:130-147`（front = depth/2+1 = 6） |
| `access::exit_point` | 北 (270,162.85)、南 (270,197.15) | `access.rs:175-196` |
| facade（`closest`） | 北 (270,162)、南 (270,198) | `access.rs:65-…` |
| **0.30m 门槛** | **只对 exit_point 与 facade 两点**相对建筑 `ground_height_m()` 判断 | `access.rs:334-337` |
| 路径中心与两侧检测带 | `HALF_WIDTH = 0.75m` | `access.rs:9` |
| 寻路 | 2m 网格 8 邻域（`STEP = 2.`）、`MAX_LENGTH = 120m`、`MAX_EXPANSIONS = 6000` | `access.rs:10-12, 196-277` |
| 坡度/障碍 | 相邻高差 ≤ `0.25 * 步长`；`segment_clear` 避障、避水 | `access.rs:133-166` |
| frontage 接受 | 还要求连到**指定 BC**（`link.road_id == site.road_id`） | `frontage.rs:686-705` |

**撤回**："0.30m 限制使旧肩部高点必不可跨"、"B2 牺牲真实连通已证明"、"保持与连通存在明确必要矛盾"、"只有 C 或拒绝"。事实是：x268 的两个冲突点属候选 carve 的**边缘**，**不在** x270 路径中心/±0.75m 检测带上，也不在 exit_point/facade 两点；已有四点只能说明**对应局部段很陡**，**既未证明可达、也未证明全域不可达**。因此正确状态是 **access 未验证**（不是"已证不可行"，也不反向声称 B 可行）。未采样路线的坡度/障碍不得编造；候选最终工程面必须由该候选自身的最终栅格给出，现有 JSON/自然数据不能替代。

## 2. T2：按四条不变量重写设计

设计必须同时满足：

1. **旧支撑等值**：所有既有受保护 support 必须 `before == after`（等值，不只是"不下降"）；
2. **新 pad 等值**：候选 building+2m 整个 pad 必须等于 site 设计高程 **2390**（不只入口中心）；
3. **真实通行**：exit_point 与 facade 满足 0.30m，且存在连续可行路径并**连到指定 `road_id`**；
4. **不变量不放宽**：natural no-cut/fill/zone/water/障碍/parcel/owner/yard 合同不变；未运行的后续审核仍 **not_run**。

**首选（1 个）**：**几何候选 G-1** —— 连接 carve 只覆盖"路面平带内 + 本候选自身 parc​el 内"的格点，且不再出现"候选硬支撑覆盖既有实体支撑带"的情形；硬支撑作用域限本 parcel，blend 仍可外扩但**不得改写**既有支撑采样（保持等值）。

* 与四点证据的关系：x268 两点落在候选 carve 边缘且距 x270 路径中心 ≥2m，**不在** ±0.75m 检测带 → 该设计**可能**同时满足第 1、3 条，但**必须**由候选最终栅格上的路径采样验证；本轮**不声称已满足**。
* 仍未知：入口 (270,163)/(270,197) 到路面在**最终栅格**上的实际高差与障碍；plan 内没有这些采样。

**备选（1 个）**：**C-1 显式合成语义**（硬支撑 vs 过渡填方分离，或候选硬支撑仅作用于自身 parcel）。它**更可能**同时满足 1/2，但属**合同级**变更：改变既有世界最终栅格与审核结果，必须绑定新 revision，旧证据按原版本保留。**不在本轮实施**，也不作为"因局部失败就必须改全局"的主张。

## 3. T2 续：B2/B3 的真实接口需求（区分内部结构与可配置接口）

内部 `TerrainCarve` 的 `half_w/half_d/blend_m/target_h_m/priority` **不是** manifest 已有可配置接口：manifest 只提供 `automaticRoadExtensions` / `frontageProgram` / `roadExtensions` 等作者声明（见 `w1-collector-local.manifest.json` 的 `style` 键），没有"连接 carve 作用域/语义"字段。因此：

| 方案 | 能否用现数据表达 | 需要的最小接口 | 解析/编译/前检/bake/audit 一致性 | 版本 |
|---|---|---|---|---|
| B2 裁短/收窄连接 | **几何可表达**（改连接 carve 的 half/blend 计算来源） | 无需新字段，但需**规则来源**明确（由哪个作者声明控制） | 五处必须读同一规则：`plan_terrain_carves` → 前检 `base+proposed` → road audit → `candidate_final_terrain` → bake | 不改合同也改变生成结果 → 需 revision 绑定 |
| B3 整平带内侧接入 | 几何可表达，但入口侧仍需跨 blend 带（未证明） | 同上 | 同上 | 同上 |
| C 合成语义 | **不可用现数据表达** | carve 语义标签（`hard_support`/`transition_fill`）或候选作用域字段；schema + 编译 + 五处消费者 | 需同时改：schema → 编译 → 前检 → audit → final terrain → bake | **合同级**，新 version + 旧证据标注 |

## 4. T3：profile 身份绑定修复（合成回归，已通过）

`verify_carve_identity` 原先只比较"是否带 profile"。现补 **profile 内容全字段比较**（`version`、`road_width_m`、`start`、`end`、`max_fill_mm`、`elevations_mm` 逐点），并新增合成反例 `g10_r1_preflight_profile_content_binding`：同外形/中心/目标不变，**仅把非端点非中心的一个采样 +1mm** → 修复后**必须拒绝**（旧的"仅存在性"比较会放过）。另有 `g10_preflight_identity_binding`（同优先级互换必须报错）与 `g9_preflight_provenance_schema` 精确复跑通过；三者均为合成、零自然输入。

## 5. T4：待批实验提案（仅写，不执行）

* 首选假设 **H-G10R1-1（不改生产语义）**：在**候选最终栅格**上，对 G-1 几何采样"入口→立面→路径中心与 ±0.75m 带"的实际高差、障碍与到指定 BC 的路径（`exit_point`/`facade` 0.30m、坡度 0.25、`route` 120m/6000），同时验证四点旧支撑等值与 pad 等值。
* 反例分类（不得统称 access 失败）：旧支撑变化 / pad 不平 / 入口门槛超差 / 路径坡度超限 / 障碍或水阻断 / 未连到指定道路 / 预算未穷尽（120m 或 6000 expansion 用尽）。
* 规模：**1 次自然 harness、1 状态（ST18-R1）、2 候选、4 个既有坐标 + 仅必要的入口/路径采样点**；不在本轮执行。
* 若最终证明必须改生产语义（C），先给白名单与默认关闭策略（测试专用开关 + 新 revision），由主检查员另批；**单次方案失败只能否定该方案/输入，不能证明所有 B 连接几何失败**。

## 6. 交付

新证据目录 `bevy/work/collaboration/ds-b01-g10-r1-access-contract-design/`：`synthetic-profile-binding.log`、`synthetic-g10-regression.log`、`synthetic-g9-regression.log`、`source-hashes.txt`、三个检查日志、`receipt.json`。G10 原文只追加勘误（access 适用范围、四点不能外推、接口可分性），旧 G10 receipt/run/attribution 与 G9 证据不改。
