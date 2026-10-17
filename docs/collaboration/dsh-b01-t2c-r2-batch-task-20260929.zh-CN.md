# DSH 批量任务 R2：B01-T2 收口 + T3 全绿 + 1km 候选离线准备 + 确定性/黄金护栏

日期：2026-09-29。下发：检查员/项目经理 Claude。执行：V41F。
性质：**实现与离线验证授权；世界运行、产品 CLI 生成/审计、浏览器均未授权。**

本文件**取代** `dsh-b01-t2c-r1-midround-inspection-and-task-20260929.zh-CN.md` 的 §2-I2 与 §3（执行指令部分）；该文件 §1 中期裁定、§2 的 I1/I3/I4/I5 发现仍有效，须逐条回应。上位任务书 `dsh-b01-t2-production-chain-continuation-20260929.zh-CN.md`（SHA-256 `20DA5EE4…1542`）的禁止项和 C4 清单继续有效；冲突处以本文件为准。

对应需求 ID：B（R06/R07 道路—地块—建筑链）、ABC 共同护栏（外部意见 #16 确定性、#19 黄金语料、#22 机器可核验交付）、执行计划 P0/P1 准备。

**请严格按任务书完成任务，不得因为其他任务无关问题擅自中断或停止部分步骤；上下文压缩、会话切换、摘要丢失或可修复的驱动问题不是跳过必测步骤的理由。需要中断时先写 checkpoint，恢复后继续。**

---

## 0. 总体安排

四个工作包，一次下发，整包执行，中途不请示：

| 包 | 内容 | 依赖 | 阻塞时 |
|---|---|---|---|
| W1 | T2 生产链收口（探针删除、F4、F5、C3、C4、3 个红测修复） | 无 | 单项阻塞：具名记录后继续本包其他项和 W3 |
| W2 | T3 15 模块全绿 + CLI 构建绑定 | W1 | 停在具名红项 |
| W3 | 最后一次 1km 候选的离线准备（冻结输入、对比器、预注册判据） | 无，可与 W1 交替 | 独立推进 |
| W4 | T2 链确定性合同 + 黄金 fixture 摘要 | W1 的 C4 fixture | 独立推进 |

证据根目录不变：`bevy/work/collaboration/ds-b01-t2-production-chain-continuation-20260929/`，本轮全部新产物写入 `r2/`，**不覆盖**已有 report/checkpoint/logs。

**明确不在本批次：** 人口/需求分母、RegionRulePack、过渡带/荒谬规则、市政、命名、GIS、FUT-01～06、渲染/材质/G34/G35、512m、6×10km、1km 实际运行、参数扫描。原因：执行计划 P2 以上均以真实 1km 业务增量为前提，前置条件未满足；提前做会把未闭合的道路—地块缺口分散到多个新系统。

---

## W1：T2 生产链收口

### W1-1 删除探针 + 隔离归因（仅用于诊断）

1. 删除 `worldgen/road_network.rs:491–500` 的 `B01T2_DISABLE_BASE_PROFILES` 分支，记录删除前后 hash。该探针提前 `return` 跳过了 roadside_sites 生成，此前“10/4 vs 11/3 → 既有失败”的归因**作废**，报告中须更正。
2. 建隔离副本 `r2/isolated-pre-t2/`：复制 workspace `Cargo.toml`/`Cargo.lock` 与 `crates/core`；用 `ds-b01-r1-t2-opt-b-profile-20260929/source-backup/*.pre-t2optb` 替换三文件；用存档脚本去掉四个机械同步文件中本轮加入的 `profile: None` / `base_profile_rejections` 字面量行（逐处列行号）。独立 `CARGO_TARGET_DIR`。**不触碰当前工作树，无任何 Git 写/checkout/stash。**
3. 在副本中 `--exact` 跑这 3 个测试，保存日志与副本关键文件 hash：
   - `graded_width_and_direction_matrix_is_supported`
   - `wide_collector_with_local_connection_is_supported`
   - `graded_road_audit_accepts_legal_engineering_and_rejects_real_damage`

### W1-2 3 个红测：无论是否既有，本轮都必须修绿

项目负责人裁定：**T3 必须全绿，不设“既有红项豁免”。** 隔离对照只用于定位根因，不改变“必须修”的结论。

- 失败点 `road_network.rs:1818` “no legal 12m corridor was found at rotation 0”。先给出根因诊断（哪条走廊候选、被哪条约束拒绝、拒绝时的具体数值），写入 `r2/red-tests-diagnosis.json`。
- 若根因是带 profile 的基础路实体与扩展走廊的占用/冲突/接入判定不兼容：这是真实生产缺陷（扩展路无法接到已分级基础路），必须在写集内修复 **判定逻辑**，同时是 C3 的一部分。
- 若根因是测试 helper 的场景/期望本身与现行合同矛盾：可修改 helper 构造的**输入场景**使其重新表达原测试意图，但须在诊断文件中逐条说明原意图、为何原场景已不合法、新场景如何保留同一断言强度。**禁止**删除测试、`#[ignore]`、削弱断言、改期望数值。
- 禁止用以下方式修绿：清空/flatten base profile；放宽走廊宽度、坡度、填方、保护区、水域合同；改扩展搜索预算或算法目标。
- 若根因确实位于只读的 `road_alignment.rs`/`road_profile.rs` 核心：该项停下，写出具体函数、行、输入值与违反的不变式，继续其余工作；**不得绕过**。

写集补充授权：`worldgen/road_network.rs` 测试模块内的 helper、`worldgen/road_extensions.rs` 的走廊/接入判定（仅限与 base profile 兼容相关）。

### W1-3 F4：生产实体校验

生产路径中，在进入 frontage 消费前，对 `entities()` 输出的每个带 profile 基础路实体调用 `RoadProfile::validate_entity`。失败按 I5 分界：单边问题 → `base_profile_rejections`（类别 `profile_entity_mismatch`）、道路保留、无 station；结构损坏 → 返回 Err，公开 plan 不半更新。两种情况各至少一个 fixture。

### W1-4 F5：数值边界测试

i32::MIN/MAX、-32000/256000 mm 边界及 ±1、NaN/±Infinity 旋转、合法 0/90。无 panic、无饱和截断、无放宽 epsilon。

### W1-5 C3：base 身份与 trusted contract

1. **先复现后修**：真实生产入口离线小场景，所有基础边带 profile → 证明 `auto_roads::base_only_plan` 清空 base、`trusted_base_contract` 拒绝；保存修复前红日志。
2. 先 grep 所有按“有无 profile”判道路类别的读取点，清单写入 `r2/base-identity-readers.json`（文件:行:用途），再改。
3. 身份 = 规划来源 + extensions 成员关系（ID 唯一、节点/拓扑一致），不靠字符串前缀授信。TrustedBaseContract 从真实已校验基础计划建立，锁定节点高程、几何、权威 profile、自然 hash。
4. 非平坦毫米地形下，既有 roadside_sites、共享节点、入口平台与 profile 相容；站点若退化，不放宽支撑、不删站点，报告具体越界点。

### W1-6 C4 fixture 矩阵

按上位任务书 §6 全清单实现，登记到 `r2/c4-matrix.json`（测试名、文件:行、命题、helper/下层 solver、mutation 结果）。

检查员重点核验的五个点，每个都要 mutation 敏感性证明（改坏生产代码一处 → 该测试变红，日志存档，改回）：

1. profile → `entities()` → `validate_entity` → `frontage::stations()` 非空 → 至少一个实际 candidate/site；
2. 拒绝边 station 为空，且不贡献 realized；
3. save → load 后 `base_profile_rejections` 仍在且内容一致；
4. base + extension 混合无重复实体；
5. trusted 基线对篡改 profile 样本/端点高程/几何/自然 hash/缺边均拒绝。

---

## W2：T3 全绿 + CLI 构建

1. 15 模块定向测试：road_profile、road_alignment、road_network、road_extensions、auto_roads、frontage、frontage_supply、road_parcel_feasibility、parcel_geometry、services、development、landcover、agriculture、landuse_ledger、zoning。逐模块原始日志。
2. `r2/t3-summary.json`：模块、过滤表达式、passed/failed/ignored、测试数、日志路径、源码 hash 集。**测试数为 0 = notObserved，不是 pass**（上轮 road_profile 就是这样）。ignored 须列出名称及忽略原因是否早于本轮（静态 grep 前后备份）。
3. 最后再跑一次 `cargo test -p glyphweave-core --lib` 全库，确认定向之外无回归；有失败同样须修或具名阻塞。
4. 全绿后构建 CLI（**只构建不运行**），记录 exe 路径、hash、构建时源码 hash 集，写 `r2/cli-build-binding.json`。

---

## W3：最后一次 1km 候选的离线准备（不运行世界）

目的：T3 全绿、检查员通过、项目负责人授权后，1km 运行一次即可得出结论，不因输入/对比口径临时决定而浪费最后额度。

1. **冻结输入**：以 b8 基线 `bevy/work/collaboration/ds-b-mainline-resume-20260929/runs/b8-current-a01/` 为对照，定位其实际 manifest、seed、scene、命令行（读 `baseline-run.json`、`world/glyphweave.manifest.json`、`world/generation-provenance.json`）。复制为 `r2/1km-candidate/frozen-input/`，记录 hash。**候选必须与 b8 同输入，唯一变量是源码。** 不改需求、zoning、尺寸、比例。
2. **写运行脚本但不执行**：`r2/1km-candidate/run.ps1`（或 .mjs），固定 exe（W2 绑定的 hash，运行前校验）、输入、全新输出目录、进程记账（start/end/exit/pid）、之后依次运行 scale-audit 与 quality-report，并**同时记录 exit code 与 JSON status**。脚本中加入防误触：未设置显式授权环境变量/参数时拒绝执行。
3. **对比器**：`r2/1km-candidate/compare.mjs`，只读两个世界目录，输出 JSON：
   - road edges 总数、带 profile 的基础路数、`base_profile_rejections` 按类别计数；
   - frontage road IDs、stations、roadside sites、frontage sites；
   - development plan records、realized buildings、records 与实体锚点不一致数；
   - urban homes/shops 需求与实现；
   - land planned / realized / unresolved / conflict / excluded（字段不存在则 notObserved，不补 0）；
   - scale-audit、quality-report 的 exit 与 status；
   - conservation 复算结果。
4. **用 b8 自测对比器**：对 b8 自身运行（b8 vs b8），输出 `r2/1km-candidate/b8-baseline-metrics.json`，并与交接文档 §3.1 数字逐项核对（46 edges、20 profile、22 roadside、1 frontage、22 records、21 realized、homes 0/120、shops 1/24、realized 334,257 m²、unresolved 665,743 m²、ledger incomplete）。不一致逐项解释口径，不得调整对比器去“凑”交接数字。b8 的 scale-audit/quality JSON 若不在目录内，标 notObserved，**不运行 CLI 补跑**。
5. **预注册判据**（运行前写死，`r2/1km-candidate/preregistration.json`）：
   - 正增量：带 profile 基础路 > 0，且 frontage stations、sites、realized buildings 至少一项相对 b8 严格增加，且 ledger 无新增重复计量/负面积；
   - 空增量：profile 路增加但 stations/sites/buildings 均不增 → 保持 partial，**不扫参数、不重跑**，交根因诊断；
   - 回退：任一项相对 b8 下降、或 scale-audit/quality 出现 b8 没有的失败类别 → 记为回归；
   - 无论结果，JSON status=fail 不得按 exit 0 接受。

---

## W4：T2 链确定性合同 + 黄金 fixture 摘要

范围仅限本轮 C4 fixture 所覆盖的离线链，不扩展到世界。

1. **进程内确定性**：同一输入连续两次构建计划 → `serde_json` 序列化字节完全一致（profile、`base_profile_rejections` 顺序、entity ID、station/site 顺序）。
2. **跨进程确定性**：同一 fixture 在两个独立测试进程中输出摘要 hash 一致（可用一个 `#[test]` 写出摘要文件 + 脚本比较两次运行结果）。记录 HashMap/HashSet 在该链上的遍历点清单，若有遍历顺序影响输出的，改为稳定排序（写集内）。
3. **黄金摘要**：为 C4 的至少 4 类场景（全 profile、部分拒绝、legacy 无 profile、base+extension 混合）生成摘要 JSON（计数、ID 列表、拒绝类别、profile 样本 hash、stations/sites 数），存为 `bevy/crates/core/tests/golden/` 下的快照（或 core 已有的测试资源目录，先查惯例），测试比对快照；快照更新必须通过显式环境变量，不得在失败时自动覆写。
4. 字段分类写入 `r2/determinism-contract.json`：哪些逐字节一致、哪些数值容差（及容差值与理由）。

---

## 5. 预算与硬边界

- cargo：本批次软计划 **30 次**（隔离副本计入），达 75%（23 次）时在 checkpoint 中汇报用量与剩余诊断；有新诊断且在写集内可继续；禁止无新诊断原样重试。
- `r2/runs.jsonl` 逐次记账（attempt、command、cwd、CARGO_TARGET_DIR、start、end、exit、pid、日志路径），日志数量与记录一一对应。上轮无法恢复的调用标 `not_instrumented`，不补造。
- 世界 0、产品 CLI 运行 0（构建允许）、浏览器 0；512m 已耗尽；1km 最后一次额度**未释放**；6×10km 禁止。
- 不改 `F:\modenmapbox`；不做 Git 写/切换/回退/stash/提交/推送；不回写历史证据。
- 不改 zoning/reserved/保护/水域/需求/退界/reachDistance/ratio/尺寸/坡度/填方上限/走廊宽度；`road_alignment.rs`、`road_profile.rs` 核心只读；`frontage.rs`、`frontage_supply.rs`、`engineering_connections.rs`、`development.rs`、`agriculture.rs`、`landcover.rs` 只允许 test-only 与机械同步。
- 新文件/目录若位于产品源码树，按 AGENTS.md 运行 `pnpm doc-tree:check`（仅针对 `src/`；bevy 下不适用则注明）。

## 6. 交付

`r2/report.json` 按 `dsh-inspector-reporting-protocol.zh-CN.md` 结构：一句话结果、证据矩阵（static/compile/unit/fixture，逐项）、≤3 条新信息、预算、阻断/下一步；逐条回应 R1 文件 I1、I3、I4、I5 与本文件 W1–W4。另交 `r2/checkpoint.json`、`r2/write-set.json`（本轮每个被改文件：角色、起点 hash、终点 hash）、源码 diff（相对本轮起点）。

状态只能是：
- `delivered_for_inspection`：W1–W4 全部完成、T3 全绿、CLI 已构建绑定、1km 准备包齐全；
- `partial`：列出每个未完成项及一个具名阻塞与最小下一测量。

不得宣称 B01 通过、城市产能增长、T4 已就绪或 1km 已授权。

## 7. 检查员下次核验清单

1. 探针已删，归因更正，hash 链连续；
2. 3 个红测的根因诊断与修法合规（无削弱断言、无放宽合同）；
3. C3 修复前红/修复后绿原始日志；
4. W1-6 五个关键点均有 mutation 敏感性证据；
5. T3 15 模块 + 全库无 0-tests 伪通过、无未解释 ignored；
6. CLI exe hash 与源码 hash 绑定可复算；
7. b8 自测对比器输出与交接数字逐项对得上或有口径解释；
8. 预注册判据在任何 1km 运行之前已冻结（文件 hash 进 checkpoint）；
9. 确定性两次输出 hash 一致，黄金快照不可自动覆写；
10. runs.jsonl 与日志一一对应；世界/CLI 运行仍为 0。
