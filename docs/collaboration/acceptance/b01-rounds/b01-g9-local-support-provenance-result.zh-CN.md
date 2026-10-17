# B01-G9 结果：station18 局部支撑来源诊断（机制确认，附一处映射缺陷披露）

* 任务：`docs/collaboration/ds-v41f-b01-g9-local-support-provenance-task.zh-CN.md`；上游审查：`docs/collaboration/b01-g8-st18-first-increment-inspection.zh-CN.md`
* HEAD：`fb36dc4f9fdd38e6aa9c5a89676a65b96e5001b1`
* **状态：`diagnosis_ready_for_review`**。未改 carve/保护/审核合同，未实现任何修复；measure / realize / candidate_final_terrain / bake 全 0；B partial、C 不推进。
* 固定 ST18-R1（B=3110、C=−382），只诊断 4 个坐标；未跑 R2、未重跑 G8、未做全场采样。

## 1. 一次诊断运行（唯一一次）

| 项 | 结果 |
|---|---|
| 预约 | `run-reservation.json` 在重建之前以 `create_new` 写入（再次执行会拒绝） |
| 冻结基线 | 35 边 / 27 节点 / 21 义务，固定入口 8000mm，基线 plan hash `f9fff91c…` |
| G8 交叉核对 | R1 的 AB 85 点 / BC 113 点与新重建**逐点相等**；homes 与 shops 在 station18 的中心/支撑范围/设计**完全相同** |
| 生产等价性 | 4 点用**生产 `plan_terrain_carves` + `apply_terrain_carves`**（前缀重放 + 单项探针）；两个冲突点的镜像值与 G8 记录**逐项一致**：north before 6502 / after 2390，south before 6987 / after 2390 |
| 调用 | 计划重建 1、`plan_terrain_carves` 4、`candidate_site` 2、`road_extensions::append_one` 2、`frontage::append` 1；**measure 0、baseline/candidate realize 0、candidate_final_terrain 0、bake 0** |
| 耗时 | harness 内部 2.1565341s（上限 90s）；编译与 3 个合成测试单列 |

## 2. 四个坐标的来源链（数值为生产函数实测）

| 坐标 | 位置 | natural | base sample(before) | 决定性 base 步骤（生产顺序索引 / 优先级 / target_h_m） | 候选覆盖 | after |
|---|---|---:|---:|---|---|---:|
| 北冲突 (268,171) | 距 BC 轴 **9m**，7m 整平带**之外**、18m 名义 blend 之内 | 1690 | **6502** | idx2：优先级 **1**、target 8000mm、`full=false`,`inBlend=true`，2450→6502（**过渡环填方**，非天然、非路面） | 连接走廊 `full=true`（outside=(0,−10)） | north→**2390**、south→不变 |
| 南冲突 (268,188) | 距轴 **8m**，整平带之外 | 1709 | **6987** | idx49：优先级 **10**、target 8000mm、`full=false`,`inBlend=true`，5861→6987（**另一既有硬支撑实体的过渡环填方**） | 连接走廊 `full=true`（outside=(0,−10)） | south→**2390**、north→不变 |
| 北整平带边界 (268,173) | 距轴 **7m**（精确整平带边界） | 1666 | **2230** | idx78：优先级 **10**、**带 profile**（该点 profile 值 2230），6358→2230 | 连接走廊 `full=true` 但被更高优先级路面 carve 压住 | **2230（不变）** |
| 南整平带边界 (268,187) | 距轴 **7m** | 1695 | **2230** | 同上 | 同上 | **2230（不变）** |

两个控制点说明：**冲突不在精确整平带内**——7m 边界处由既有**路面 carve（优先级 10）的硬支撑**主导（2230mm，既不是 BC 的 2390 也不是过渡填方），候选连接走廊虽完全覆盖却被压住、采样不变；冲突发生在整平带**之外**（8/9m）、道路 carve 只以过渡环触及、而候选连接走廊完全覆盖的位置。

保护窗口：4 点命中的既有实体只有 **本态新增 BC 道路**（`bounds+3m` → `[173,171,291,189]`），不是更宽的保护带；因此拒绝来自"道路支撑带 + 过渡环"的组合，不是整平带本身。

## 3. 机制结论（先确认候选机制）

**确认**：候选**前侧连接走廊（connection）carve**在两个冲突点达到 `outside ≤ 0`（完全覆盖），而 BC 道路 carve 在该处只处于 **blend 过渡环**（`outside>0`）；在 `graded_influence = true` 且存在完全覆盖 carve 时，`apply_terrain_carves` 末段的 `full_target` 分支（`worldgen.rs:4518-4521`）把 terrain **重置为 `max(natural, 候选 target)` = 2390mm**，从而**丢弃**了此前由其它实体过渡环提供的 6502 / 6987 填方。逐项剔除验证：移除该候选 carve 后数值**回退到 before**（`removingCarveRevertsToBefore=true`）；主 pad（优先级 1，half 8×7）与入口 apron（优先级 1，half 2×2）在该点分别为 `full=false`，**不能**单独造成该变化。

因此"生产硬支撑覆盖重置到 natural+candidate target、并移除其它对象先前的 blend fill"这一候选机制成立，且有 owner 类别、具体组件与数值支持；但**具体是哪一个既有实体提供了 6502/6987**，见 §5 的缺陷披露，本轮**未能定名**。

## 4. 缺陷如实披露（本轮 run 的 owner 标签不可靠）

诊断输出中的 **`ownerEntityId` / `component` 字符串不可信**：我的映射函数用"排序前的 push 序号"去索引**已按 priority 排序的**生产数组，导致标签整体错位（例如把 BC 道路的 carve 标成站点点位、把连接走廊标成 apron）。证据中的以下部分是**有效**的，因为它们直接来自生产数组本身：索引、`priority`、`hasProfile`、`targetHMm`、`outside/full/blend` 关系、单 carve 值、前缀重放序列、镜像 before/after、`removingCarveRevertsToBefore`、`protectionScope`。

修复与校验（仅测试代码，未再自然运行）：改为由**生产 push 规则预测优先级**、再做稳定排序，并对每个索引断言"预测优先级 == 生产 carve 优先级"；同时修正组合顺序语义——`preflight` 用的是 `base_carves ++ proposed`（`frontage.rs:540-542`）**而非全局重排**。修正后的映射已通过合成 fixture（`g9_preflight_provenance_schema`，含逐项优先级校验）验证。

由此可**离线恢复**候选侧的正确组件归属（证据中 `candidateSites.*Carves` 带几何）：idx0 = pad（half 8×7，blend 103.17，优先级 1）、idx1 = **入口 apron**（中心即前锚点 (270,163)/(270,197)，half 2×2，blend 2，优先级 1）、idx2 = **连接走廊**（中心 (270,171.5)/(270,188.5)，half 2×10.5，blend 2，**优先级 2**）——即上文"决定性候选组件是连接走廊"的结论成立。**基座侧**的决定性实体则**保持 unknown**（同一缺陷使标签错位，且当时未记录 base carve 的 cx/cz；要定名需修复后的 harness 再跑一次或从冻结世界离线重建，均需另行授权）。

## 5. 修复设计建议（只提建议，本轮不实施）

| 方案 | 做法 | 会否改变生成结果/版本/合同 | 需要新授权 |
|---|---|---|---|
| A 现状不动作 | 保持既有合成语义 | 不变；该类地块持续被 `existing_support_changed` 拒绝 | — |
| **B 候选连接高程/连接方式** | 使前侧连接 carve 的 target 与所连道路的硬支撑面一致，或让连接组件不伸出路面平带（不改审核语义） | **改变生成几何**（走廊高程/范围），需版本与旧证据标注；**不改审核合同** | 是 |
| C 显式合成语义 | 在 carve 合成中显式区分"硬支撑"与"过渡填方"并定义优先级（例如过渡填方不得被候选硬支撑覆盖，或候选硬支撑仅在自身 parcel 内生效） | **改变 carve 合同**，会改变既有世界生成结果，必须新版本 + 全量重跑 + 旧证据标注 | 是（合同级） |
| D 收窄保护窗口 | 把道路保护带由 `bounds+3m` 改为运输包络等 | 不解决本冲突：冲突点在 8/9m，仍落在 ≤9m 的运输包络内；且削弱既有保护 | 不建议 |
| E 抬高 B/C、减 setback/院落、提高 fill、放开 no-cut、扩大 junction | 已被 G7/G8 否决或越界 | 越界 | 否（不采纳） |

推荐：先做完 B 的**设计**（不改代码），因为它是唯一在**不改审核合同**前提下可能消除该类的方向；C 属合同级变更，必须单独立项。本轮**不实现任何一项**。

## 6. 预算、文件与边界

* 预算：诊断 harness **1 次**、计划重建 1 次、候选 2 个、坐标 4 个；measure/realize/bake **全 0**；内部耗时 2.1565341s（≤90s）。3 个合成测试 + 编译单列。
* 未做：R2/其它状态、station90、全场 carve 扫描、CLI/最大图/gate/release/浏览器、Git 写操作、agent、F/A 变更、生产源修改（仅 `auto_roads.rs` 的 `#[cfg(test)]`）。
* 证据目录 `bevy/work/collaboration/ds-b01-g9-local-support-provenance/`：`run-reservation.json`（预约/预注册）、`run.log`、`run.json`（4 点记录、候选 carve 几何、G8 等价性、schema、预算）、`preflight-schema.log`、`preflight-mapping-fix.log`、`preflight-helper-regression.log`、`build.log`、`source-hashes.txt`（SHA256/BLAKE3 分列）、`git-status.log`、`git-diff-check.log`、`doc-tree-check.log`、`receipt.json`。
* G8 报告已追加四项勘误（§8）：station18 两态合计 8 attempt；R2 st66 设计 150（首缺 289/5、全缺 855/999）；G6-R1 源码绑定为 330309B/`319bdceb…`（294054B/`1994ef49…` 是 G6）；生产前缀 UTF-8 字节数 81685（hash `ba6deb95…` 不变）。旧 run/receipt 不改写。

## 7. 勘误（B01-G10 轮追加，2026-09-19）

独立审查 `b01-g9-local-support-provenance-inspection.zh-CN.md` 裁定 **accepted-mechanism-with-offline-attribution-supplement**：4 点数值、候选 connection 的剔除对照、以及"既有 blend 填方被 full_target 覆盖"的局部机制成立；但**原 run 的 owner 标签无效**，检查员已从冻结计划与 G8 设计离线恢复决定性来源（`offline-owner-recovery.json`：58 实体 / 79 base carves / 144 条关系匹配），**不需要为修标签再自然重跑**。本节只追加，旧 run/receipt 不改。

1. **owner 字段全部作废**：§4 中 `ownerEntityId` / `ownerKind` / `component`（不止两个字符串）以及 `schema.ownerMappingVerified=true` 均不可作为归属证据——该标志实际只检查了 `protectionScope` 是数组、`baseCarvesAtCell` 非空，**不验证身份**。真实来源改为：排序索引 **2** = 旧住宅 `…scene-0.10-11.1` 的 pad（把北冲突推到 6502）；**49** = 旧道路 `…scene-0.25-26` 的 road_pad（把南冲突推到 6987）；**78** = 新增 BC 的 road_pad。
2. **数值污染**：`bandNotes.bcRoadCarveBlendM` 与 `candidateSites.roadCarveBlendM` 记录的 120 属于**其它 carve**；BC 真实 blend 为 **97.6259999871254m（≈97.626）**。§2 与 §5 中"18m 名义 blend / 120m"的表述应以此为准；两冲突点仍在真实 blend 范围内，核心机制不变。
3. **控制点 2230 的来源**：它不是"既有路面 carve 的平台"或"另一实体的台面"，而是 **BC 自身在 x=268（station 20）的 longitudinal profile 值**（由其 road_pad 硬支撑主导）。§3/§4 相关句子应据此更正；2390（station 18）与 2230（station 20）同为 BC 设计值，不是外来值。
4. **`schema.ownerMappingVerified` 与后修代码的界限**：最新 `tag_carves` 只断言优先级相等与标签非空，未做完整几何比较，同优先级 pad/apron 互换仍会通过——该缺陷已在 B01-G10 用**强签名身份绑定 + 同优先级互换反例**修正（仅合成回归，未自然重跑）。**runGeneration = pre-mapping-fix（无源码快照 → 该修订 hash 记 unknown）；currentSource = post-mapping-fix**，不得用当前源码 hash 声称原运行归属已修。
5. **哈希编码口径**：run 内 `reservationHashBlake3=7715eabd…` 是对**JSON 序列化文本字符串**的 BLAKE3，不是预约文件原始字节的 BLAKE3（文件字节 BLAKE3 为 `686e9cf0…`；文件 SHA256 绑定本身正确）。SHA256 一律对原始字节，BLAKE3 对规范化 JSON 值，两者不混用。
6. **里程碑 A 措辞**：回执中 `milestoneA=not signed off` 属回退表述。A 已于 2026-09-09 正式签收；本 worker **不修改 A 状态、也无签收权限**，B 仍 partial、C 不推进。
