# 交接文档：给下一位实现 Agent（V41F 续行者）

日期：2026-09-29
写作者：本会话的 DSH 实现 Agent（任务书里的执行代号 **V41F**）
性质：**交接说明，不是新的实现授权**。授权范围看任务书，不看本文。

---

## 0. 你是谁、在做什么、先做什么

**你的角色**：DSH 实现 Agent，执行代号 **V41F**。你**只实现和离线自测**；独立验收由检查员/项目经理（Claude）做，项目边界由项目负责人定。**你不能自行签收 B 或 C，不能做 Git 写操作。**

**项目**：`G:/difyllmwiki/GlyphWeave` —— Rust workspace（`bevy/crates/core`、`bevy/crates/cli`）。这是一条真实世界生成主线，**不是渲染/材质/小地图项目**。

**你要做的事（当前唯一优先链）**：

```text
base RoadEdge
→ authoritative RoadProfile
→ RoadNetworkPlan::entities()
→ frontage::stations()
→ candidate site
→ building / ledger 增量
```

**你的第一件事不是写代码，而是按顺序读下面 §1 的四份文档**，然后执行 **R2 批量任务**（W1→W4）。R2 是当前最新、最权威的执行指令。

**性质提醒**：R2 授权**实现与离线验证**。**世界运行、产品 CLI 生成/审计、浏览器均未授权**（CLI 允许构建，不允许运行生成）。

---

## 1. 必读文档（按此顺序）

| 顺序 | 文档 | 为什么读 |
|---|---|---|
| 1 | `docs/collaboration/dsh-b01-t2c-r2-batch-task-20260929.zh-CN.md` | **当前权威任务书**。取代 R1 的 §2-I2 与 §3。四个工作包 W1/W2/W3/W4，一次下发，整包执行，中途不请示 |
| 2 | `docs/collaboration/dsh-b01-t2-production-chain-continuation-20260929.zh-CN.md`（SHA-256 `20DA5EE4…1542`） | 上位任务书。禁止项与 **C4 必测清单**继续有效 |
| 3 | `docs/collaboration/dsh-b01-t2c-r1-midround-inspection-and-task-20260929.zh-CN.md` | §1 中期裁定与 §2 的 **I1/I3/I4/I5 发现仍有效**，须逐条回应（§2-I2、§3 已被 R2 取代） |
| 4 | `docs/collaboration/project-handoff-abc-b-mainline-20260929.zh-CN.md` | 项目级总交接：A/B/C 状态、长期规划、开发纪律 |
| 5 | `docs/collaboration/b01-t2-opt-b-independent-review-20260929.zh-CN.md` | 上一轮独立检查（F1–F5），其中 F1 证明我数错了编译站点 |

另：`docs/collaboration/dsh-inspector-reporting-protocol.zh-CN.md` —— R2 要求 `r2/report.json` 按它的结构写。

**任务书 hash 要先验证再读。** 我这两轮都先算了 SHA-256 并确认 MATCH 再读正文。

---

## 2. 我这一轮实际做了什么（已被检查员部分否定，务必读第 3 节）

证据目录：`bevy/work/collaboration/ds-b01-t2-production-chain-continuation-20260929/`

**本轮的产出**：
- **C0 基线**：用 `--message-format=json` 抓原始编译诊断（`logs/test-compile.stdout.jsonl`，exit 101）。
- **C1 编译恢复**：5 处 `RoadEdge` 字面量补 `profile: None`；新诊断字段又需 5 处 `RoadNetworkPlan` 字面量机械初始化。测试编译现 **0 错误**。
- **C2 部分修复**：`RoadNetworkPlan` 新增 serde 向后兼容的 `base_profile_rejections`（edgeId/category/reason），**调用点改为持久化**而非 `?;` 丢弃（F2）。数值边界改为 **checked_mul 到 i64 + 显式 -32000..=256000 毫米域**（对 level 与每个样本各查）+ 比较前**显式有限性/四向检查**（F5 部分）。
- **`report.json` / `write-set.json` / `checkpoint.json`** 已写。

**我没做完的**（R2 会要求你补）：
- **C3 未开始**（base 身份与 trusted contract）
- **C2-F4 未做**（真实 `entities()` 输出的 `validate_entity`）
- **C4 / C5 未开始**
- **一处临时脚手架仍在源码里**，必须删除（见 §3）

---

## 3. 最重要的警告：我的归因方法有缺陷，我的结论**已作废**

我在 `worldgen/road_network.rs` 里放了一个临时探针：

```rust
if std::env::var("B01T2_DISABLE_BASE_PROFILES").is_ok() { ... return Ok(result); }
```

我用它做了对照（关闭 = 10 passed/4 failed；开启 = 11 passed/3 failed），并据此在报告里写"那 3 个 `road_network` 失败是**先前存在**的"。

**检查员指出：这个探针提前 `return`，跳过了 `roadside_sites` 生成**，所以两条对照臂根本不是同一个生产路径 —— **我的归因作废**。

**R2 的 W1-1 明确要求**：删除该探针（`road_network.rs:491–500`），记录删除前后 hash，**并在报告中更正该归因**。R2 同时要求改用**真正的隔离副本**（`r2/isolated-pre-t2/`，独立 `CARGO_TARGET_DIR`，用 `source-backup/*.pre-t2optb` 替换三文件）来做对照。

**而且**：R2 的 W1-2 裁定 —— **T3 必须全绿，不设"既有红项豁免"**。项目负责人已否掉 R1 的豁免规则。所以那 3 个测试**无论是否先前存在，本轮都必须修绿**。

**教训请继承**：做对照实验时，**先确认两条臂走的是同一条生产路径**；一个提前 `return` 的探针不是对照。

---

## 4. 已经就绪的东西（可直接复用）

| 资产 | 位置 | 说明 |
|---|---|---|
| 生产实现：`RoadEdge.profile` + `entities()` 三级优先级 | `worldgen/road_network.rs` | 优先级：① 边自身 profile → ② 旧 `extensions` 反查（扩展路）→ ③ 无 |
| 扩展路显式 `profile: None` | `worldgen/road_extensions.rs` | 保持 `RoadExtension.design` 是扩展路 profile 的**唯一来源** |
| `base_edge_profile` / `attach_base_edge_profiles` | `worldgen/road_network.rs` | 单边 → `CorridorRequest` → **真实 `AlignmentContext::design`** → `RoadProfile` → `validate_shape` → 几何一致性。**12 个具名拒绝类别** |
| 持久拒绝字段 | `RoadNetworkPlan::base_profile_rejections` | serde 向后兼容；**不是** profile 注册表 |
| B01-R1 T1 修复（非对称退界随朝向旋转） | `worldgen/frontage.rs` | 红→绿已证明；frontage **23/0** |
| 前轮 pre-edit 备份 | `ds-b01-r1-t2-opt-b-profile-20260929/source-backup/*.pre-t2optb` | W1-1 隔离副本要用 |
| 夜间 b8 基线世界 | `ds-b-mainline-resume-20260929/runs/b8-current-a01/` | W3 冻结输入的对照基准 |

---

## 5. 硬边界（违反即污染证据）

- **不改** `F:\modenmapbox`
- **不做任何 Git 写**：不 commit / checkout / stash / reset / 切分支 / push
- **不回写历史证据**：旧 report / checkpoint / manifest / 台账只读；更正写在新报告里
- **不跑世界**：世界生成 0、产品 CLI 生成/审计 0、浏览器 0。**512m 已耗尽**；**1km 最后一次额度未释放**；**6×10km 禁止**
- **不改** zoning / reserved / 自然 / 保护 / 水域判据 / 用户需求 / 地块尺寸 / 退界政策 / reachDistance / urban ratio / zoning cell / 坡度上限 / 填方上限 / 走廊宽度
- **`road_alignment.rs`、`road_profile.rs` 核心只读**。若根因确在其中，停下并写出具体函数、行、输入值与违反的不变式，**不得绕过**
- `frontage.rs`、`frontage_supply.rs`、`engineering_connections.rs`、`development.rs`、`agriculture.rs`、`landcover.rs` **只允许 test-only 与机械同步**
- 不启动子代理
- 写集内**每个文件唯一列一次**，标 `mechanical-test-sync` / `semantic` / `test-only`

---

## 6. 几条会让你少走弯路的经验

1. **不要只读 exit code。** `quality-report` 进程可能 exit 0，而报告 **JSON `status` 是 `fail`**。我犯过这个错，检查员明确纠正过。世界级审计要**同时**看 exit 与 JSON status。
2. **"测试数为 0" 不是 pass，是 `notObserved`。** 上一轮 `road_profile` 过滤到 0 个测试，我一度当成通过。R2 的 W2-2 专门要求区分。
3. **编译错误数要来自编译器的主 span，不要正则扫散文。** 我曾把 5 处报成 28 处（正则匹配了所有 `-->` 行）。
4. **改公共结构前先全量枚举字面量。** `RoadEdge` 加字段会影响所有 Rust 字面量点。
5. **已编辑过的文件不能覆盖最初的 pre-edit 记录**；**未编辑的文件现在登记 hash 是正常前置登记，不是回溯造证**（R2 已明确授权这一点，也授权了同类机械同步，不必为它停一整轮）。
6. **`mem::take` 本身不是正确性论据** —— 我靠"无条件放回 `nodes`、错误在 plan 完整后才传播"来保证不半更新，这个顺序要在代码里显式、可读。
7. **`saturating_mul` + `abs()` 是错的**：饱和会把离谱值夹成看似合法，`abs()` 对 `i32::MIN` 溢出。用 checked 运算 + 显式域。
8. **验尸优先于猜测**：我最初想用"是不是既有失败"来免除修复责任，正确做法是 R2 要求的**隔离副本对照**得出根因，然后**照样修绿**。

---

## 7. 你的下一步具体动作

```
1. 读 §1 的五份文档，验证 R2 任务书 hash
2. W1-1：删除 B01T2_DISABLE_BASE_PROFILES 探针（road_network.rs:491–500）
        记录删除前后 hash；在新报告里更正我上轮的归因
        建 r2/isolated-pre-t2/ 隔离副本（独立 CARGO_TARGET_DIR）
        在副本中 --exact 跑那 3 个测试，存日志与副本关键文件 hash
3. W1-2：写 r2/red-tests-diagnosis.json（哪条走廊候选、被哪条约束拒绝、具体数值）
        然后把这 3 个测试修绿 —— 有诊断、不削弱断言、不放宽合同
4. W1-3 F4 → W1-4 F5 → W1-5 C3（先复现 base_only_plan/trusted_base_contract 反例，再修）
5. W1-6 C4 fixture 矩阵（含 5 个关键点的 mutation 敏感性证明）
6. W2：T3 15 模块全绿 → 全库 → 构建 CLI（只构建）
7. W3：1km 候选离线准备（冻结输入、脚本、对比器、预注册判据），用 b8 自测对比器
8. W4：确定性合同 + 黄金 fixture 摘要
9. 写 r2/report.json（按 inspection reporting protocol）、checkpoint、write-set、diff
```

**预算**：cargo 软计划 **30 次**（隔离副本计入），达 75%（23 次）时在 checkpoint 汇报用量与剩余诊断。`r2/runs.jsonl` 逐次记账（attempt/command/cwd/CARGO_TARGET_DIR/start/end/exit/pid/日志路径）。**上轮我无法恢复的调用标 `not_instrumented`，不补造。**

**状态只能二选一**：`delivered_for_inspection`（W1–W4 全完成、T3 全绿、CLI 已构建绑定、1km 准备包齐全）或 `partial`（列出每个未完成项 + 一个具名阻塞 + 最小下一测量）。

**不得宣称**：B01 通过、城市产能增长、T4 已就绪、1km 已授权。

---

## 8. 事实与未观察项的边界（请照此对待我的产出）

**已观察（有原始日志）**：编译错误数与其主 span；C1 后的 0 错误测试编译；各定向套件结果；前轮 T1 的红→绿。

**未观察，不要当已知**：
- 世界级行为：**未观察**，本轮没跑世界
- `auto_roads` 供给是否真的排除带 profile 的 base 边：**静态阅读，未复现**
- 那 3 个 `road_network` 失败的根因：**未知**（我的归因已作废）
- 上轮 6 次 cargo：**日志未留，属"声明，未经独立核验"**，不要升级为事实
