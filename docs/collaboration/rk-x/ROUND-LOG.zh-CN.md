# B/C 主线执行日志（自主轮次）

记录规则：每完成一项功能改动 = 一次提交 + 独立测试证据。不以文档数量代替验收。
基线需求见 `../requirements-and-progress-master.zh-CN.md`。

---

## Round 1 · 2026-10-18 · 提交 `0fe6ebd`

**问题**：`baked_audit::joint_program_recovers_housing_minimum_and_preserves_source_obligations`
期望 18 户、实际 1 户。

**实证**（不是读文档推断）：

| 事实 | 数值 / 出处 |
|---|---|
| 冻结夹具生成的世界 | `bevy/work/rk-x/joint-a01`（512m，29 边，22 站点） |
| 带 frontage point / 分级基准的站点 | **0 / 22** |
| 已实现住宅的连通情况 | 17 个 `connected=true`，但 **roadId 全部不是被指定的那条** |
| 基础边 profile 形状 | 1000 → −600 → 1000 mm（1.6 m 落差） |
| 引道可承载落差 | `WALKABLE_GRADE × ENTRANCE_APRON_RUN_M` = 0.25 × 2 = **0.5 m** |
| 受控实验：拒绝全部基础边分级 | **18 / 18 通过** → 病根是分级，不是垫层规则 |
| 锥形夹具每条边最小落差 | **0.627–0.640 m**（5% 坡），同样超过 0.5 m |

**改动**：`road_network.rs` 新增生成侧闸门。有聚落需求的场景里，若一条基础边的分级使其
24×24 路边垫层在任何站位都无法被 2 m 引道进入，则该边分级被**具名拒绝**
（`grading_leaves_no_enterable_pad`，带实测最小落差），并保持平坦的既有 `level`。
无需求的纯规划场景保留全部几何合法分级（其路边扫描是投机性的）。

**为何只在有需求时生效**：全场景生效会改变 `c3_base_identity_is_provenance_and_the_contract_locks_profiles`
与 `w4_chain_digest_and_golden_summary_are_stable` 两条金标，而 T-0013 明令"牵动金标即停下"。
两次尝试（给中心退化点补 frontage → 0/18；全场景生效 → 5 个 core 测试红）都被实测否决并回退。

**独立测试**：`baked_audit` 26/26（第 7 条 18/18）；core lib 626/626，29 ignored；两条金标未改。

---

## Round 2 · 2026-10-18 · 提交 `7a713c2`

**问题**：`morphology_compile::the_real_binary_binds_and_rejects_a_tampered_effective_document`
无法烘焙控制世界。

**根因**（T-0012 已定位，本轮核实）：`assets/objects/storefront.object.toml` 要求立面到路
≤ 8 m 且 `fallback = "skip"`；夹具给商铺的 `setbackM = 12`，立面间隙约 14 m，必然违反。
`engineered_placement::reconcile` 把**任何**带 `frontage_road_point` 的实体当 fixed，
命中拒绝直接 `Err`，不读描述符自己的 fallback，因此一个小卖部让整张世界烘不出来。

**改动**：`engineered_placement.rs` 遵守 `fallback = skip`——**只有** `generated.` 前缀、
非地标、非基础设施、且描述符声明 skip 的实体可被跳过；跳过写入 `skips[]`（entityId/kind/
round/reason/descriptorFallback）并把 `removedItems` 计入报告。authored 实体、地标、
道路/桥梁/铁路/运河/堤道/人行道，以及要求 move/shrink 的描述符仍是硬错误。

**独立测试**：
- `morphology_compile` **2/2**（原 1/2）
- `baked_audit` 26/26
- core lib 626/626，29 ignored
- **`cargo test --workspace` exit 0：全部 target 全绿**（44 / 101 / 26 / 1 / 2 / 1 / 2 / 4 / 1 / 626）——
  这是 T-0013 的 AC-1 首次达成。

---

## 下一步（Round 3 计划）

1. **垫层模型决策（需项目负责人）**：平板垫层 + 纯填方 + 2 m 引道在坡度 ≥3.6% 的地形上
   数学上不可建（0.5 m ÷ 14 m 窗口）。这解释了 b8 真实 1 km 世界只有 1 个 frontage site。
   三选一：(a) 只做平地；(b) 有界挖方台地（违反现行"只填高"合同，需授权）；
   (c) 垫层随坡 + 阶梯基础（改动最大）。未决前不动这项。
2. **生态缺 1 株**（A 例外 #4）：在 1 km 冻结输入上复现并定位该株的合法候选与拒绝原因。
3. **B-03 用途闭合**：先只做"可数值化"的部分——把 50.23 km² 未实现按 urban/rural/nature
   分项报出 realized/unresolved，不引入新地表类型。
