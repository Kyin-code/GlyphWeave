# B01-T2 续行中期检查裁定 + R1 续行指令（给 V41F）

> **2026-09-29 追记：** 项目负责人不同意本文件 §2-I2 的“既有红项豁免”规则。§2-I2 与 §3 执行指令已由 `dsh-b01-t2c-r2-batch-task-20260929.zh-CN.md` 取代（T3 必须全绿，3 个红测无论是否既有均须修复）。§1 裁定与 I1/I3/I4/I5 发现仍有效。

日期：2026-09-29。检查员/项目经理：Claude。性质：**对续行轮中途交付的独立只读检查 + 下一段执行指令**。
上位任务书（仍然有效，本文件只补充、收窄）：
`docs/collaboration/dsh-b01-t2-production-chain-continuation-20260929.zh-CN.md`（SHA-256 `20DA5EE4…1542`，检查员已复算一致）。

检查员本次：只读源码与证据目录，复算 hash；**cargo 0 次、世界 0、CLI 0、浏览器 0、Git 写 0**。

---

## 1. 中期裁定

状态维持 **partial**。C0/C1 接受；C2 部分接受；**“road_network 3 个失败为既有失败”的归因不接受，撤回为 unattributed**。

| claim | 检查结果 | 证据层级 |
|---|---|---|
| C0 旧“28/8”更正为编译器实测 5/4 | 接受。c0-baseline.json 的 5 个 E0063 与检查员前次独立编译一致 | compile |
| C1 测试编译恢复（5 RoadEdge + 5 RoadNetworkPlan 字面量） | 接受为声明；write-set.json 中 7 个文件 hash 与当前源码逐一复算一致 | static |
| C2-F2 拒绝持久化到 `RoadNetworkPlan.base_profile_rejections` | 静态确认：road_network.rs:505–509 已把返回 Vec 写入计划，节点在 `?` 之前归还。**尚无 save→load 读回 fixture**，不算 fixture 级 | static |
| C2-F5 checked 算术/高程域/NaN | 声明，未见对应测试 | static（未验证） |
| C2-F4 实际 entities() 调 validate_entity | 执行者自认未做 | — |
| C3 base 身份/trusted contract | 未开始，未离线复现 | — |
| “3 个失败为既有失败” | **不成立**，见 I1 | — |
| T3 套件状态 | 覆盖不全，见 I3 | — |

## 2. 检查发现

### I1（严重）：归因探针不等价于“改动前行为”，结论撤回

`worldgen/road_network.rs:494` 的 `B01T2_DISABLE_BASE_PROFILES` 分支在 `if let Some(level)` 内直接 `return Ok(result)`，**跳过了其后整段 roadside_sites 生成及后续所有逻辑**（:512 起）。因此：

- “disabled 10/4 vs enabled 11/3”比较的是“完整新代码”与“截断的新代码”，不是“T2 前”与“T2 后”；
- “attach 修好一个测试、不破坏任何测试”无依据；
- 3 个失败信息 `no legal 12m corridor was found at rotation 0` 来自 `road_extensions::append` 的走廊探测——base 实体带 profile 后占用/冲突判定变化**恰好是可能诱因之一**，不能排除。

另外，生产函数内读取环境变量改变行为本身属于越界的临时脚手架，必须删除且在交付中登记删除前后 hash。

### I2：T3 红项的处理规则（项目经理裁定）

用隔离对照（见 R0）判定：

- **若 T2 前隔离副本同名测试以相同 panic 信息失败** → 登记为 `pre-existing-red`，附两边原始日志与源码 hash；不计为本轮回归，本轮不修（根因不在写集内），但 T3 汇总中必须单列，不得写成 pass。
- **若 T2 前隔离副本通过** → 这是本轮回归，必须在已授权写集内修复；禁止改测试期望、放宽走廊/工程合同或把 base profile 清空来“修”。若修复需要改 road_alignment/road_profile 核心或走廊算法，报告具体越界点，停在该项。

### I3：T3 覆盖不完整、存在“0 tests”伪通过

- `road_profile` 报“0 tests in this filter”——过滤器没有命中，**等于 notObserved，不是 pass**。
- 未报告：`road_alignment`、`road_extensions`、`auto_roads`、`frontage_supply`、`road_parcel_feasibility`、`agriculture`。
- 15 个模块必须全部有原始日志，每个日志中测试数 > 0（若某模块确实无单测，给出 `grep -c '#\[test\]'` 静态证据并标 N/A-申请，由检查员批）。

### I4：进程账目不可核

报告称“每次调用记录在 logs/”，但 logs/ 只有 3 个文件，对应不了 13 次 cargo。要求补 `runs.jsonl`（每行：attempt、command、start、end、exit、pid、日志路径）。已无法恢复的历史调用标 `not_instrumented`，**不得补造时间/PID**。软计划 12 已超出 1 次，接受（有新诊断）。

### I5：结构错误与单边拒绝的分界需要测试证明

`attached?` 把 helper 的 Err 向上传播。须用 fixture 证明：单边不可设计 → 进入 `base_profile_rejections`、道路保留、无 station；结构损坏（节点缺失、ID 重复、实体不一致）→ 返回 Err 且公开 plan 不半更新。

---

## 3. R1 执行指令（按顺序，整包执行，不逐步请示）

证据目录不变：`bevy/work/collaboration/ds-b01-t2-production-chain-continuation-20260929/`，新产物放 `r1/` 子目录，**不覆盖**已有 report/checkpoint（新写 `r1/report.json`、`r1/checkpoint.json`）。

### R0 删除探针 + 隔离归因

1. 删除 `B01T2_DISABLE_BASE_PROFILES` 分支；记录删除前后 road_network.rs hash。
2. 建隔离副本：`r1/isolated-pre-t2/`，复制构建所需的 bevy 工作区最小集合（workspace `Cargo.toml`/`Cargo.lock`、`crates/core`）；用 `ds-b01-r1-t2-opt-b-profile-20260929/source-backup/*.pre-t2optb` 替换三文件；对四个机械同步文件用可复现脚本去掉本轮 `profile: None` / `base_profile_rejections` 字面量行（脚本存档，列出每处行号）。使用独立 `CARGO_TARGET_DIR`（放在 r1/ 下或系统临时目录并登记），**不得触碰当前工作树、不得 git checkout/stash**。
3. 在隔离副本只跑那 3 个失败测试（`--exact`），保存原始日志与副本关键文件 hash。按 I2 分支处理。

### C2 收尾

4. 生产 `entities()` 输出中每个带 profile 的基础路实体调用 `validate_entity`（在进入 frontage 消费前，生产路径内），失败归入具名拒绝类别 `profile_entity_mismatch` 或结构错误，按 I5 分界。
5. F5 数值边界测试：i32::MIN/MAX、-32000/256000 边界及±1、NaN/±Infinity 旋转、合法 0/90。无 panic、无饱和截断。

### C3 base 身份与 trusted contract

6. **先复现**：真实生产入口离线小场景，所有基础边带 profile → 证明 `base_only_plan` 清空 base、`trusted_base_contract` 拒绝。保存失败输出（修复前红）。
7. 先 `grep` 全部按“有无 profile”分类道路的读取点，把清单写入 `r1/base-identity-readers.json`，再修改。
8. 按上位任务书 §5 修复：身份 = 规划来源 + extensions 成员关系；TrustedBaseContract 从真实已校验基础计划建立并锁定节点高程、几何、权威 profile、自然 hash。禁止清空/flatten profile、禁止只删拒绝条件。
9. 非平坦毫米地形下验证既有 roadside_sites、共享节点、入口平台与 profile 相容（上位 §5 配套检查）。若站点因 profile 退化，按上位任务书报告越界点，不放宽支撑。

### C4 fixture

10. 按上位任务书 §6 全清单逐条实现。每条在 `r1/c4-matrix.json` 登记：测试名、文件:行、覆盖的命题、helper 还是下层 solver、mutation 是否使其变红（至少对 trusted 篡改、拒绝读回、station 非空三条做 mutation 敏感性验证并存日志）。
11. 关键验收点（检查员会重点看）：**profile → entities() → validate_entity → frontage::stations() 非空 → 至少一个实际 candidate/site**；拒绝边 station 为空；save→load 后拒绝仍在；base+extension 无重复实体；同输入两次运行 profile/拒绝顺序/ID 字节一致。

### C5 T3 与 CLI 绑定

12. 15 模块定向测试全部跑，原始日志逐模块保存，`r1/t3-summary.json` 机器可读（模块、passed、failed、ignored、测试数、日志路径、源码 hash）。0 tests = notObserved。
13. T3 仅剩 I2 判定的 `pre-existing-red` 时视为可进入 CLI 构建；否则停在具名红项。
14. 构建 CLI（仅构建，**不运行生成/审计**），记录 exe 路径、hash、构建时源码 hash 集。

### 交付

15. `r1/report.json` 按 `dsh-inspector-reporting-protocol.zh-CN.md` 结构：一句话结果、证据矩阵（每项标 static/compile/unit/fixture）、≤3 条新信息、预算、阻断/下一步。逐条回应本文件 I1–I5。
16. 状态只能是 `delivered_for_inspection` 或 `partial + 一个具名阻塞`。不得宣称 B01 通过、城市产能增长或 T4 就绪。

## 4. 预算与硬边界

- cargo：本段软计划 **15 次**（隔离对照的编译/测试计入），达到时汇报并在有新诊断时继续；禁止无新诊断原样重试。
- 世界 0、产品 CLI 运行 0、浏览器 0；512m 已耗尽；1km 最后一次额度**未释放**；6×10km 禁止。
- 不改 F:\modenmapbox；不做任何 Git 写/切换/回退/stash；不改 zoning/reserved/保护/水域/需求/退界/reachDistance/ratio/尺寸/坡度/填方上限；road_alignment.rs、road_profile.rs 核心只读。
- 中断前必须写 `r1/checkpoint.json`（源码 hash、各项状态、cargo 账目、最后命令与 exit、恢复第一条命令）。上下文耗尽不是跳过必测项的理由。

## 5. 检查员下一次裁定会核验的内容

1. 探针已删、hash 链连续；2. 隔离对照日志真实可复算（副本文件 hash）；3. C3 修复前红/修复后绿的原始日志；4. §3-11 五个关键验收点逐一有测试且 mutation 敏感；5. T3 15 模块无 0-tests 伪通过；6. runs.jsonl 与日志数量一一对应；7. 世界/CLI 运行额度仍为 0。
