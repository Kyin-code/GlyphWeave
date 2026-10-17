# G-1 替代几何卡：离线构造性检查结果（无可行构造，停止）

* 状态：`spec_blocked_no_construct`；本轮 **源码修改 / cargo / 自然重建 / design / append / infer / measure / carve 重放 / final terrain / realize / bake 全 0**，未改旧卡与旧回执。B partial、C 不推进。
* 允许变量：**仅 connection 矩形几何与 blend**；`target=2390`、`priority=2`、pad/apron、道路/建筑/parcel/入口位置与全部审核阈值不变；禁止格点 mask、禁止事后恢复旧高程、禁止改全局 carve 合成语义。

## 1. 先读保存坐标（不重算）

| 项 | 值 | 来源 |
|---|---|---|
| 北 parcel z | 148..166 | 保存的 G8/G9 行 |
| 南 parcel z | 194..212 | 同上 |
| BC 整平带 z | 173..187（横向 half=7） | 生产规则 |
| BC 保护带 | x 173..291, z 171..189（bounds+3） | 生产规则 |
| anchor / exit_point / facade | 北 (270,163)/(270,162.85)/(270,162)；南 (270,197)/(270,197.15)/(270,198) | `road_network.rs:130-147`、`access.rs:175-196`、`access.rs:65` |
| 已知冲突点 | (268,171)、(268,188) | G9 观测 |

## 2. 构造性判定

* **G-1a（P_own ∩ B_flat）**：parcel（z≤166 / z≥194）与平带（173..187）**无交集** ⇒ 覆盖集合为空 ⇒ 按卡自身停止条件**阻断**；不得改用交集/并集的另一种解释。
* **G-1b（同构造，仅改矩形）**：`cz=168/192, half_w=1.0, half_d=5.0, blend=2, priority=2, target=2390`。非空 ✓、入口到平带连续 ✓、**不再完全覆盖** (268,171)/(268,188)（x=268 在矩形外，`outside_x=1`）✓。**但仍不可提交**：矩形必须覆盖 z=171..172（北）/188..189（南）才能连到平带，而这些格**落在 BC 保护带内**；候选 carve 会成为它们的完全覆盖者，末段 `full_target` 分支会改写它们 → 违反不变量 1（旧支撑等值）。它只是把改变从 x=268 挪到 x=269..271。
* **族级论证**：入口（z=163/197）与平带边缘（z=173/187）**跨过**保护带条带；任何连续矩形要跨过它就必然覆盖 z=171（北）/188（南）在自身 x 范围内的格，而该范围的 x 含 anchor 的 x=270。收窄 `half_w` 只会移动被改格的 x；扩大/缩小 `blend` 无用（环内只在 `target > base` 时填方，触发机制来自**完全覆盖**）。
* **结论：在允许变量集合下不存在可行构造**。要避开该条带，需要 (a) 非矩形的允许集合（格点 mask，**已禁**）或 (b) 合成/语义变更（C-1，**不自动转入**）。本卡在此**停止**，把决策交回检查员。

## 3. 若日后有构造，验证范围（本轮仍然 unknown）

旧支撑交集 = `C_conn(rule) ∩ (∪_old S_protect(old))`（候选之外的全部实体）逐格 `before == after`；完整 pad = building+2m **全部 255 格** == 2390；access = exit_point/facade 0.30m + ±0.75m 带坡度 ≤0.25 + 避障/避水 + `route` ≤120m/6000 且连**指定 BC**。范围必须同时包含 pad、apron、connection **矩形与 blend**；几何检查通过**不等于**支撑/access 通过，未采样项保持 unknown。

## 4. 测试专用局部反事实接口（只设计，不实现）

* **不可用**：`frontage_supply::candidate_final_terrain`——它从 plan 派生实体并内部调用 `plan_terrain_carves`，会把 proposed carve 列表丢掉、重新生成旧 connection carve，因此**不能**作为"验证了新规则"的证据。
* **可接受设计**：cfg(test) 专用 helper，入参为 `(scene, natural terrain, zones, 显式 carve 列表)`，用**生产 encoder + `apply_terrain_carves`** 在该显式列表上生成地形；support/pad 比较用同一列表。不改生产接口、不改默认值。
* 状态：仅设计；在没有构造获批之前不实现。
