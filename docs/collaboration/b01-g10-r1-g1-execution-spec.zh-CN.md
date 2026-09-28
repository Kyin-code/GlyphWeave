# B01-G10-R1 / G-1 执行规格卡（仅规格，未实现、未运行）

* 状态：`spec_ready_for_review`；**implementation = not_run｜cargo = not_run｜natural = not_run｜carveReplay = not_run｜realize/bake = not_run**；旧证据不改。B partial、C 不推进。
* 本卡只定义"若获批准要做什么、用哪条规则、验证范围与预算"，**不构成可行性结论**。

## 0. 规则来源（第 1 项）：测试专用 proposed G-1 rule

| 选项 | 说明 | 能否称"不改变候选生成语义" |
|---|---|---|
| 现行生成规则（`worldgen.rs:4239-4351`） | 候选连接 carve 无条件生成：中心 `((ax+rx)/2,(az+rz)/2)`、half `(|Δx|/2+2, |Δz|/2+2)`、blend 2、priority 2、target = `entity.ground_height_m()` | 是；但这等于现状（B1），本卡不采用 |
| **测试专用 proposed G-1 rule（本卡唯一规则）** | 只在 cfg(test) 内构造、只用于诊断 harness 的 carve 列表 | **否**：它**改变候选生成语义**，**不是生产通过**，也**不能**当作"不改生成语义"的候选 |

结论：本卡采用**测试专用 proposed G-1 rule**，并明确它**不是**生产语义；任何"生产已支持/语义未变"的表述在此作废。

## 1. 规则定义（第 2 项，伪代码 + 坐标集合）

只改**一个组件**：候选的**前侧连接 carve（connection）**；pad 与入口 apron 一律不动。

```
连接起止：anchor = front_anchor(site)  →  road_point = site.frontage_road_point
北：anchor(270,163) → (270,180)；南：anchor(270,197) → (270,180)
道路平带：BC 的整平带（graded_flat_top_half_extents）= 沿向 60 / 横向 7（rot 0，12m 路）
  ⇒ 平带边界：北侧 z = 180 − 7 = 173；南侧 z = 180 + 7 = 187

G-1a（唯一规则）：
  z_end = if anchor.z < 180 then 173 else 187            # 平带边界，不进入保护带
  cz    = (anchor.z + z_end) / 2                          # 北 168 / 南 192
  half_d= |anchor.z − z_end| / 2                          # 北 5 / 南 5
  half_w= 2（不变）; blend = 2（不变）; priority = 2（不变）
  target = site.ground_height_m()（不变，= 2390）
  追加约束：carve 覆盖集合 ⊆ 候选自身 parcel（frontage::parcel_boundary(site) 的闭区间）
            ∧ ⊆ 平带以内（|z − 180| ≤ 7）
  若上述交集为空或与 anchor 不连通 → 规则视为不可表达，直接判规格阻断，不做局部修补
```

坐标集合（唯一、可复算）：

* `P_own` = 候选自身 parcel（北/南各一，来源 `frontage::parcel_boundary`，实现时从保存行读取，不在此假定）；
* `B_flat` = `{ (x,z) : 176 ≤ x ≤ 288, |z − 180| ≤ 7 }`（BC 平带）；
* `S_protect(road)` = `{ (x,z) : 173 ≤ x ≤ 291, 171 ≤ z ≤ 189 }`（BC 支撑带 = bounds+3）；
* G-1a 的覆盖集合 = `P_own ∩ B_flat`（**与 `S_protect \ B_flat` 不相交**，因此不再改写旧支撑采样）。

**禁止事后修补**：不以"把旧支撑数值改回 6502/6987"作为规则的一部分；旧支撑等值若在任何一格不成立，视为该规则失败（见 §4 停止条件）。

规则来源与 revision：实现只允许放在 `bevy/crates/core/src/worldgen/auto_roads.rs` 的 `#[cfg(test)]`；若日后要进生产，必须另立任务、另给 version 绑定（本卡不提供生产开关）。

## 2. 验证范围（第 3 项）：四点只是哨兵

| 范围 | 定义 | 本轮状态 |
|---|---|---|
| 哨兵 4 点 | (268,171)、(268,188)、(268,173)、(268,187) | 已有 G9 观测（before 6502/6987/2230/2230） |
| **完整旧支撑受影响交集** | `C_conn(G-1a) ∩ (∪_old S_protect(old))`，其中旧实体集合 = 舞台计划中**候选之外**的全部实体（35 旧路 + 2 新段 + 21 义务） | **未枚举 = unknown**（需实现阶段按上述公式枚举，逐格比较 before/after 等值） |
| **完整 pad** | 候选 building+2m 的**全部 255 格**（17×15，闭合）必须 == 2390 | **未验证 = unknown** |
| **实际 access** | exit_point(270,162.85)/(270,197.15) 与 facade(270,162)/(270,198) 相对 ground 的 0.30m；路径中心与 ±0.75m 带内坡度 ≤0.25；避障/避水；`route` ≤120m/6000 expansion 且 `link.road_id` == 指定 BC | **未验证 = unknown**（需要候选最终栅格） |

失败细类与"有无观测依据"：

| 细类 | 观测依据 | 说明 |
|---|---|---|
| 旧支撑变化 | `before != after` 的**格点坐标与数值** | 有依据（逐格比较即产出） |
| pad 不平 | 支承矩形内 ≠2390 的格点 | 有依据 |
| 入口门槛超差 | exit/facade 与 ground 的差值（mm） | 有依据 |
| 路径坡度超限 | 采样带内相邻高差 >0.25×步长的位置 | 有依据 |
| 障碍/水阻断 | `segment_clear`/`route` 返回的状态 | 有依据 |
| 未连指定道路 | `link.road_id` 与 `link.status` | 有依据 |
| 预算未穷尽 | 是否触及 120m 或 6000 | 有依据 |
| 「路径在现实意义上不可走」以外的语义 | — | **无观测依据 → 保留 unknown，不写结论** |

## 3. 测试专用实现白名单（默认关闭，第 4 项）

| 文件 | 允许改动 | 默认关闭机制 |
|---|---|---|
| `bevy/crates/core/src/worldgen/auto_roads.rs` | 仅 `#[cfg(test)]`：G-1a 连接 carve 构造函数、被 `#[ignore]` 的诊断 harness、schema/身份断言 | harness `#[ignore]`（默认不跑）+ 规则只存在于该 harness 内；**生产路径无开关、无默认值改动** |
| 其它生产文件 | **不允许**（`worldgen.rs`/`frontage.rs`/`road_network.rs`/`access.rs` 等一律不改） | — |

单次预算（获批后执行时使用）：**自然 harness 1 次、计划重建 1 次、候选 2 个、`candidate_final_terrain` 1 次（路径验证所需）、baseline realization ≤1、candidate realization ≤2、measure 0、bake 0、runtime ≤120s**；编译与合成测试单列。

停止条件：任何自然失败、序列化不完整、profile 点数/身份绑定断言失败、旧支撑任一格不等值、pad 任一格 ≠2390、入口门槛或坡度超限、预算未穷尽 → **立即停止并如实输出该细类**；禁止改规则后自行重跑。**单次失败只否定该输入/该几何，不得外推为所有连接几何不可行。**
