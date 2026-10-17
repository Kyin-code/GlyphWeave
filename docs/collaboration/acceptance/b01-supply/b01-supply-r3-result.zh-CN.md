# B01-SUPPLY-R3 结果：接齐真实 final 上下文与旧 parcel 来源

- 任务：`docs/collaboration/ds-v41f-b01-supply-r3-task.zh-CN.md`；先读的独立审查：`docs/collaboration/b01-supply-r2-inspection-20260917.zh-CN.md`
- HEAD：`fb36dc4f9fdd38e6aa9c5a89676a65b96e5001b1`；工作区 `G:/difyllmwiki/GlyphWeave`
- 状态：**partial（ready_for_review）**。A 事务、B 滚动承诺、C1 共享道路审核按审查结论**未再改动**；本轮只做 C2、C3 两项，另有 R2 文档/回执口径勘误。
- 未做：无 Git 写操作、无其他 agent、无 bake（0 次）、未跑冻结 128 扫描、未跑 carve 诊断、未跑 80 点时钟扫描、未改全局 carve/自然算法/fill/坡度/支撑容差、未自行签收 B/C。

## 1. C2：真实生成上下文到达实际供给入口 —— done

### 1.1 设计：来源是**声明**，不是抽样猜测

- `worldgen.rs` 新增 `NaturalSource`（`pub(crate)`）：
  `AuthoredSeed { seed, precise }` / `SuppliedRaster` / `Undeclared`。
- 同一路径贯通：`bake_world` → `road_network::plan_with_style_declared` →
  `auto_roads::append_declared` → `run_supply_phase` → `frontage_supply::realize_candidate` →
  `candidate_final_terrain` → 生产编码器 `generate_chunk_encoded`。
  `verify_with_style_declared` 让**正式审核的来源复算**用同一个声明，避免“复算换 seed”。
- **删除**了原来的 17 点采样分类 `final_raster_source`。`Realization::final_source` 现在只由声明决定：
  `production-encoder`（`AuthoredSeed`）或 `supplied-raster-carve`（`SuppliedRaster`，原名 `natural-carve` 更名以名副其实）。
- 缺上下文**明确拒绝**：`Undeclared` + version 3 → `Err("version 3 supply requires a declared natural source …")`；
  `candidate_final_terrain` 对 `Undeclared` 同样直接报错。v1/v2 的 legacy 调用语义保留
  （`plan_with_style` / `append` / `verify_with_style` 仍可用，只是无法再对 v3 静默兜底）。
- `bake_world` 是唯一声明点：`NaturalSource::AuthoredSeed { seed: manifest.world.seed ^ scene.seed_offset, precise }`
  （`precise` 即 `height_encoding_from_style(...) == Millimetre`）。
- 两个真实调用点（原审查指出的 baseline 与 candidate 两处）现在都用**同一个 `source` 变量**，
  不再有一处硬编码 `0`。

### 1.2 证据（真实入口，非独立 helper）

证据：`bevy/work/collaboration/ds-b01-supply-r3/c2-entry-context.json`、`test-worldgen-auto_roads.log`。
观测点在 `candidate_final_terrain` 内部（测试期记录器），因此看到的是**入口实际传给编码器**的值：

| 用例 | 结果 |
|---|---|
| 生产天然小场景（有效 seed 42、`scene.seedOffset = 42` ≠ 0），走 `plan_with_style_declared` | 观测到 `seed=42, precise=true, source=production-encoder`；未声明时的旧行为（seed 0 + 栅格兜底）不再出现 |
| 候选调用点（`supply_fixture` 家族，声明 `SuppliedRaster`） | 观测 2 次（baseline + 候选），全部 `supplied-raster-carve`，且 `acceptedSegments = 1`（候选确实走完实现链） |
| 同计划正式路径一致性 | 供给 final 与独立调用的生产编码路径**内容 hash 完全相同**：`d08cfe4b06ecde3b67e69287964b61d5e5fccd3ce9cf6a7ca6ff1bff01acd2c0` |
| 声明优先于抽样 | 把 (3,3) 单格抬高 2m 后，**17 个旧探针点仍然全部匹配**（旧启发式必然判成 production-encoder），但声明为 `SuppliedRaster` 时编辑被原样保留（62207mm vs 60207mm），标签为 `supplied-raster-carve` |
| 缺上下文 | `Undeclared` 在声明入口与 legacy 包装入口都返回明确错误，不静默用 0 |

### 1.3 C2 的明确边界（未降低要求）

- 生产天然场（自带起伏）**无法**把候选送过测量筛选：所有候选都以
  `profile_infeasible` / `existing_road_crossing` / `no_real_parcel_supply` 被拒，
  与冻结 W1 自然案例同因（W1 也是 128 状态 0 接受）。因此“候选调用点”的证据用**显式声明为
  `SuppliedRaster` 的小 fixture**取得：它证明的是候选调用点**遵从声明**、而不是沿用旧的抽样分类。
  两个调用点共用同一个 `source` 变量（编译器强制），seed 的贯通由天然场景的 baseline 观测证明。
- 未改任何数学地形/carve/量化阈值；未放宽毫米误差；正式 final 仍走既有 `generate_chunk_encoded`。
- A 无 v3 路径不受影响：未引入版本边界变更。

## 2. C3：完整旧 parcel 基线，新地块不能消费旧缺额 —— done

### 2.1 设计

- `SupplySession` 增加 `protected_parcels: parcel → 进入阶段的 yard 缺额` 与
  `protected_owners: entity → parcel`，由 `adopt` 从**已验证 realization 自己的** `parcel_by_entity`
  建立（`or_insert`：最早写入的 owner/额度为准，后续轮不得改写或抬高）。
  因此保护集合是**进入阶段的全部 development parcels**（含 21 栋既有义务建筑），
  不是只含 frontage 请求的 `session.commitments`。
- `commit_decision` 的 yard 判定改为**逐 parcel**：受保护 parcel 的缺额不得超过它自己的保护额度
  （不与其他旧 parcel 相抵）；未受保护的**新 parcel 必须为 0**；受保护实体不得换 owner、
  不得从审计 map 消失；landcover 审计未真正跑完（`rejected`/`unplanned`）时，空 map 不得被读成“全部已解决”。
- 只读复用已验收机制：landcover 的 `unresolved_yard_by_parcel` 与 `parcel_by_entity`，
  `settlement/landcover.rs` **本轮零改动**（hash 与 R2 相同）。

### 2.2 证据：真实 parcel 映射与四个反例

- 生产派生 fixture（`worldgen::frontage_supply::tests::frozen_baseline_parcels_are_protected_and_new_parcels_must_be_resolved`，
  ignored 有界探针，只读冻结世界、不写盘）：从**真实** development + landcover realize/verify 取得旧 map。
- `c3-parcel-map.json`：`protectedParcelCount = 21`、`protectedEntityCount = 21`、
  `finalSource = production-encoder`、`baselineYardUnresolvedM2 = 13.0`，
  两个缺额 parcel 恰为审查方 `yard-probe` 的两块：
  `…road-network.scene-0.20-27.1.parcel = 9m²`、`…road-network.scene-0.4-11.-1.parcel = 4m²`（9 + 4 = 13）。
  同一份证据里 `requestCredit = [["b04c1-homes",0],["b04c1-shops",0]]`：
  **原 21 义务进了保护表，但仍不是 frontage 信用**。
- 四个反例（同一真实 fixture，经真实守卫 `commit_decision`）：

| 反例 | 期望 | 结果 |
|---|---|---|
| 旧 4/9 原样不变 + 新 parcel yard 0 + 其余验证成立 | 接受新增信用 | 接受 |
| 旧全部修复（旧 map 空）+ 新 parcel 缺 13 | 拒绝 | `realization_new_parcel_yard_unresolved:13` |
| 一旧 parcel 减少、另一旧 parcel 等量增加（总量不变） | 拒绝并点名增长项 | `realization_existing_parcel_yard_grew:<parcel>:0->13`，错误里含增长 parcel 名 |

  另有注入式小反例（`transaction_guards_reject_regression_identity_yard_and_unrealized_plans` 扩展）：
  受保护实体换 owner → `existing_owner_changed`；受保护实体从 map 消失 → `baseline_asset_missing`；
  landcover 审计被拒 → `landcover_audit_unavailable`；上一轮已提交的新 parcel 被再抬高 → 也按“既有 parcel 增长”拒绝
  （即两轮新增 parcel 都纳入保护）。
- 回退/事务语义未改：A 事务（working copy + 7 处截止检查 + Err 不写回）与 B 滚动承诺全部保留，
  上述守卫只作用于“是否可以提交”。

## 3. 测试、命令与预算（实际执行）

| 命令 | 结果 |
|---|---|
| `cargo test -p glyphweave-core "worldgen::auto_roads::tests::" -- --test-threads=1 --skip resource_clock_expiry_never_publishes_a_success_prefix` | 22 passed / 0 failed，24.8s（新增 5 个 C2/C3 用例并入既有 17 个） |
| `cargo test -p glyphweave-core frozen_baseline_parcels_are_protected_and_new_parcels_must_be_resolved -- --ignored` | 1 passed，8.0s（C3 生产派生 fixture） |
| `cargo test -p glyphweave-core "worldgen::frontage::tests::"` | 10 passed |
| `cargo test -p glyphweave-core "worldgen::road_network::tests::"` | 13 passed / 1 ignored（重复 `#[test]` 已删除，计数回到 13） |
| `cargo test -p glyphweave-core "settlement::landcover::tests::"` | 12 passed |
| `cargo check -p glyphweave-cli` | exit 0 |

预算：bake **0 次**（全部为内存 fixture/只读冻结世界）；未跑全 CLI/worldgen/settlement/workspace、release、浏览器、最大图；
未跑冻结 128 扫描与 carve 诊断；**未跑 80 点时钟扫描**（见 §5）。
`git diff --check` exit 0；`node scripts/generate-tree.mjs --check` PASS。

## 4. 源码 hash 与冻结完整性

`source-hashes-r3.txt`（SHA256 / 字节数）：

```
5c8e4fd19794bca31af3dd6f249efe448c41e9b70a7d5dd2a6d2333cca4100d5  167215  bevy/crates/core/src/worldgen/auto_roads.rs
5f1e01bbd6d994fcc16dc8d375a4fa1bac2a9db942cabe3d85a35fcecf4da91c  50636  bevy/crates/core/src/worldgen/frontage.rs
52622a54ae472b3744df55778450ac41dfc0648960eef975478eb019b6fe2f6b  91683  bevy/crates/core/src/worldgen/frontage_supply.rs
caffa2a0e3a47e88a768256d4496cce31bfd3b47982576f19e37db806ff363ed  97204  bevy/crates/core/src/worldgen/road_network.rs
3007a94ffae8d61420571d172eeaf43b451e1cdb8fdf3d278b02b7a4a9d49432  430090  bevy/crates/core/src/worldgen.rs
9d2808b76b4390b26fb0bec629e96233c04168abfa32580f497ac955106f0d67  60586  bevy/crates/core/src/settlement/landcover.rs
```

`frontage.rs` 与 `landcover.rs` 与 R2 **逐字节相同**（A/B/C1 与 landcover 未动）。
冻结完整性（`frozen-world-integrity.json`）：W1 登记 7 文件 `unchanged=true`；
W1 manifest `930bdc13…`；旧 receipt 仍为 `69ff2c9b…`（L1）、`d84a59be…`（R1）、`b3c39e92…`（R2，未改写）；
无 F 盘/A 冻结材料改动。

## 5. 未完成项与明确限制（如实列出）

1. **80 点时钟扫描未重跑**（任务明令）。守卫只可能更严（per-parcel 比较 + owner/missing/audit 三项新拒绝），
   `Ok ⇒ incomplete=false`、`Err ⇒ 完整回退` 两条不变量不受影响；但该扫描的**覆盖断言**
   （“扫描内出现过真实成功之后再超时”）本轮未重新测量。间接证据：同一 fixture 家族上
   `demand_led_choice_prefers_real_parcels_over_distance_alone` 在新守卫下仍 `acceptedSegments = 1`，
   说明真实候选仍能提交。若检查员要求，可单独重跑该扫描。
2. **冻结 W1 的 ignored 探针本轮未重跑**（任务只要求 C2/C3 定向测试）。为通过编译，
   `frozen_w1_stage` 现在显式声明 `AuthoredSeed { seed, precise: true }`（该重建确实逐点重建了作者场），
   因此其内部 final 来源由旧的“隐式 seed 0 + 栅格兜底”变为 `production-encoder`。
   按指令未重跑，故 R1/R2 已归档数值**保持原样但未重新测量**；这不影响 C2/C3 结论，但需在下一轮或复验时确认。
3. **非 yard 地表缺额仍是总量口径**：`unresolved_area_m2` 仍按“不高于基线”比较，尚未按 source/parcel 拆分
   （本轮不扩成全地表逐来源系统）。
4. **C2 候选调用点的证据用显式 `SuppliedRaster` fixture**：生产天然场自身无法让候选通过测量筛选（见 §1.3），
   因此“非零 seed 贯通到候选调用点”是结构性保证（同一变量）而非该 fixture 的直接观测。
5. 仍未追求 24 见证，`bestLegalCandidate = null`；**B 未签收、C 未开始推进**。

## 6. 与 R2 的差异

1. 实际供给入口不再传 `0`/`42` 之类的隐式 seed：改为从 manifest 声明并贯通到编码器；17 点抽样分类删除。
2. “旧 parcel”不再是 `session.commitments`（只含 frontage），而是进入阶段的**全部** development parcels；
   yard 比较改为逐 parcel，并新增 owner/missing/audit 三项拒绝。
3. R2 文档/回执口径勘误已追加到 `b01-supply-r2-result.zh-CN.md` §7：冻结扫描 1 次、carve 诊断 1 次（不得写 0/false）；
   共享叶子测试是重复注册不是两种用例；同名扰动是同坐标重复不是两类位置。

## 7. 勘误（B01-SUPPLY-R4-CLI 轮追加，2026-09-18）

独立审查 `docs/collaboration/b01-supply-r3-inspection.zh-CN.md` §4 指出两处口径问题；本节只纠正表述，不改结论、不改任何旧 receipt 字节。

1. **"本轮执行"与"R2 历史纠错"必须分开。** §5/§6 已说明本轮没有跑冻结 128 扫描与 carve 诊断，但 receipt 顶层 `budget` 仍写着 `frozen128RescanPerformed: true / frozen128RescanRuns: 1 / carveDiagnosisRerun: true / carveDiagnosisRuns: 1`，容易被读成"R3 本轮重跑"。正确口径：**R3 本轮冻结扫描 0 次、carve 诊断 0 次**；那 1 次发生在 **R2**，属于历史纠错条目，不复制进任何后续轮次的 budget。R4 回执已把两者分列（`thisRound` 与 `r2HistoryCorrection`）。
2. **不把"结构更严"写成新的验证结论。** §5 第 1 条原写"守卫只可能更严，`Ok ⇒ incomplete=false`、`Err ⇒ 完整回退` 两条不变量不受影响"。这属于推理而非本轮实测。正确表述：**A 事务的已验收结构保持不变、本轮未复查其覆盖断言**；80 点时钟扫描本轮未重跑，检查员也未重跑，因此不给出任何新的通过结论。
3. 上述两条不改变 C2/C3 的验收内容与 §1–§4 的证据；旧 receipt（`a59c1ad6…`）保持原字节。
