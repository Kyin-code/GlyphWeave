# B01-T2 R5 检查裁定 + R6 任务（给 V41F）：证据修复 + 路边站点改走临街精确站点路径 + 129-130 诊断

日期：2026-09-30。检查员/项目经理：Claude。执行：V41F。
上位：`dsh-b01-t2c-r5-1km-run-and-b1-task-20260930.zh-CN.md`（`a5345ca0…5629`）。未改动的禁止项全部有效。

**请严格按任务书完成任务，不得因为其他任务无关问题擅自中断或停止部分步骤；上下文压缩、会话切换、摘要丢失或可修复的驱动问题不是跳过必测步骤的理由。需要中断时先写 checkpoint，恢复后继续。**

---

## 第一部分：R5 裁定

### 业务结果（接受，证据层级 production-world）

同一 manifest、同一 seed，唯一变量是源码：
- 26 条基础路带权威 profile（b8：0）；
- **frontage 实现 1 → 11（商铺 1/24 → 11/24）**，10 条临街道路中 9 条是基础路；
- 按预注册 v2 判为 **回归**：已实现建筑 21 → 17，realizedM2 334,257 → 330,069（−4,188 m²），scale-audit 新增一类失败 `invalid road longitudinal contract generated.road-network.scene-0.129-130`。

判定严格按预注册执行，没有改判，**接受**。这是 T2 以来第一次在真实世界里证明“基础路 → 临街 → 建筑”链条起效。

### 过程问题（4 项，须在 R6 修复或登记）

| 编号 | 问题 | 裁定 |
|---|---|---|
| P-1 | 第一次尝试 exe 已启动（pid 31356，861 ms）后又进行了第二次生成。R3 授权附件 §2-5 明确“exe 已启动即消耗，不得第二次生成” | **违反规则**。鉴于第一次在读取 rules 目录时即失败、未产生任何世界产物，且第二次输入字节一致，检查员**接受第二次的结果作为证据**，但登记为规则违反。正确做法是停下报告。另：W3 声称“冻结输入可直接使用”不成立——冻结副本的相对 `rulesDir` 在副本位置失效，须更正 |
| P-2 | run-accounting 中所有 `exit` 为 `null`（PowerShell `Start-Process -PassThru` 未取句柄导致） | 违反“exit 与 JSON status 双记录”。本次 exit 标 `notObserved`，不得从产物推断为 0；修复脚本 |
| P-3 | **b8 历史世界** `runs/b8-current-a01/world/preview/core/render-stream.js` 被改写（原 `53fc5125…` → `3227ce46…`，修改时间 2026-09-30 01:12）；新 1km 世界 `r3/1km-run/world/preview/core/render-stream.js` 也在生成后被改写 | **回写历史证据，违反硬边界**。检查员已核对：b8 世界中仅此一个文件变化；原版可从 `inspector-b-mainline-20260929/b8-readonly-source-copy/preview/core/render-stream.js`（`53fc5125…`）恢复 |
| P-4 | `adapters/html/core/render-stream.js` 在写集之外被修改 | 修的是真实缺陷（并发上限 2 写成直接抛错，4 个 chunk 的世界必然失败），**追认授权**，但须登记与测试，见 R6-0 |

A-4 的 4 张截图未做（具名缺口），接受；项目负责人可直接看运行中的预览。

### B-1 诊断：接受；B-2 修法：不接受原设计

B-1 把冲突讲清楚了：站点垫层按平的 `ground_height_m()` 砍削，相邻带坡道路按 profile 砍削，`apply_terrain_carves` 只取两者较大值（fill-only），垫层无法把格子压回自己的声明高度。

但报告中“让垫层**逐格跟随道路 profile**”的设计不接受：建筑垫层应当是平的，带坡度的垫层不符合建筑落地常识，也会让建筑与入口的合同变复杂。仓库里已有经过真实世界验证的做法——`frontage.rs:288-313` 的临街精确站点：**垫层是平的，高度取道路 profile 在临街站点处的高程**，并写 `support_level_mm: Some(..)` 与 `frontage_road_point: Some(..)`。本次 1km 的 11 个商铺就是走这条路落成的。路边站点应改走同一条路径，而不是再发明一套规则。

---

## 第二部分：R6 任务

证据目录：`bevy/work/collaboration/ds-b01-t2-production-chain-continuation-20260929/r6/`。`r3/1km-run/` 除 R6-0 规定的恢复外保持冻结。

### R6-0 证据修复（先做）

1. 将 b8 世界的 `preview/core/render-stream.js` 恢复为 `53fc5125…`（从检查员只读副本复制），记录恢复前后 hash；再对 b8 世界全目录与只读副本做一次 hash 对比，证明除审计 JSON 外完全一致。
2. `r3/1km-run/world/preview/core/render-stream.js` 恢复为生成时的原版（从 exe 生成时复制的 adapter 版本；若无法确定原版 hash，如实登记）。
3. 项目负责人仍需查看预览：把两个世界**复制**到 `r6/preview-copies/b8/`、`r6/preview-copies/candidate/`，只在副本里放入修复后的 `render-stream.js`，预览服务改为指向副本。
4. `adapters/html/core/render-stream.js` 的修复登记入写集（修改前后 hash、diff），为“并发超过 2 时排队而非抛错”补一个自动化测试（优先放在 `adapters/html` 已有的测试体系中，如 `*.test.mjs`），并运行现有 HTML 适配器测试确认无回归。
5. run.ps1：改为能可靠取得退出码（例如 `$p.Handle` 预取后 `WaitForExit()` 再读 `ExitCode`，或改用 `System.Diagnostics.Process`），用一个无害命令（如 `glyphweave --help` 或故意错误参数）证明 exit 0/非 0 都能被记录。**不得重跑世界。**
6. 冻结输入：更正 W3 的说法；给出可在副本位置直接使用的冻结输入方案（例如同时冻结 `rulesDir` 指向的资源并改为绝对路径，或记录必须从原位置调用），并用 `glyphweave` 的只读校验类命令（如 `validate-manifest` 一类，若存在）验证，不生成世界。
7. 在 `r6/process-violations.json` 登记 P-1～P-4：事实、影响、修复、以后如何避免。

### R6-1 B-2：路边站点改走临街精确站点路径

1. `road_network.rs` 生成 RoadsideSite 时，对所邻道路带 profile 的情况：取该站点临街点对应的 profile 站号高程，写 `support_level_m`、`support_level_mm: Some(..)`、`frontage_road_point: Some(..)`，与 `frontage.rs:288-313` 一致。**尽量复用**该处的函数或抽出共享函数，不要复制一份。
2. 垫层保持**平面**，目标高度即该声明高度；垫层范围必须退出道路路面 + 路肩 + graded band（沿用临街站点现有的退让与入口 apron 做法）。若临街站点路径中已有这套退让逻辑，直接复用。
3. 所邻道路无 profile（legacy）时保持原行为不变。
4. 四个下游读者：`frontage_supply.rs:819`、`:2615`、`auto_roads.rs:5051`、`:8101`，`None` 不得再静默读成 0：回退到 `support_level_m * 1000` 或具名报错，并各有测试。
5. 站点选择门槛：原先用 `level_m` 决定站点能否放置。改走精确路径后，若某站点因临街点高程与自然地形差距超出支撑/填方合同而**不能再放置**，不得放宽合同，也不得悄悄删掉：具名记录（站点 ID、所需填方、合同上限），并在报告中统计数量。若此类站点数量明显（例如超过 3 个），停在该项交检查员裁定是否需要挪位规则。
6. 禁止：压平道路 profile；逐格起伏的垫层；删除、移动、缩小站点来规避冲突；放宽支撑、坡度、填方、院落面积、入口、审计精度。

### R6-2 诊断 `road-network.scene-0.129-130` 纵向合同失败

1. 用 1km 世界（只读）与离线重放定位：这条基础路的哪些站号在最终地形上违反纵向合同，违反值多少，那些格子被哪个 carve 抬高或改变（路边垫层？其他道路？扩展路交叉？）。写 `r6/road-129-130-diagnosis.json`。
2. 若原因是路边站点垫层 fill-only 抬高了路基格子（即 B1 的另一面），R6-1 应同时修复它，并用 fixture 证明。
3. 若是其他原因且在写集内，修复并加 fixture；若需改 `worldgen.rs` 的 `apply_terrain_carves` 优先级/取大规则，本任务**授权**修改，但必须满足：道路 profile 覆盖范围内道路优先，站点垫层不得改变路面；所有既有 carve 测试不改断言；逐处登记。

### R6-3 验证

- W1 冻结基线：院落未实现回到约 51 m²（R-2 实验值），发展审计不再因路边院落失败；
- 分级套件全绿，不改断言；
- 新 fixture：非平坦地形 + 带 profile 基础路 + 路边站点 → 声明、垫层、院落、入口一致，道路纵向合同在最终地形上仍成立；生产代码突变（声明改回平 level）使其变红；
- 1km 冻结输入全链离线重放（R-3 方式）：以真实 1km 世界为基准逐项对比，重点看已实现建筑、realizedM2、路边院落、129-130 纵向合同；
- 全库 0 failed、15 模块、冻结 ignored 套件（前后 hash 一致）、确定性两次一致、黄金快照（合法变化只能用 `GLYPHWEAVE_UPDATE_GOLDEN=1` 并列差异）；
- CLI 重建，全集绑定写到 `r6/`（不覆盖 r4）。

### R6-4 下一次 1km 的准备（不运行）

1. 预注册 v3（新文件，不改 v2），在看到 R6-3 重放结果**之前**冻结，并把冻结时间与 hash 写进 checkpoint。基准改为本次 R5 的真实 1km 世界（`r3/1km-run/world`），同时保留与 b8 的对照：
   - 正增量：路边院落实现恢复（B1 暴露的 21 个站点中至少大部分实现），已实现建筑与 realizedM2 **均不低于 b8**，frontage 实现数不低于 R5 的 11，129-130 纵向失败消失且无新失败类别；
   - 其余情形按 v2 的方式定义空增量与回归。
2. run.ps1 更新为 R6 的 binding 与全集，放行文件路径改为 `r6/1km-go.json`。
3. **不得运行世界**。是否授予新的 1km 额度由项目负责人决定，检查员复核后写放行文件。

---

## 预算与硬边界

- cargo：软计划 **100 次**，达 75 次在 checkpoint 汇报；有新诊断且在写集内可继续；禁止无新诊断原样重试。
- 世界生成 0；产品 CLI：仅允许 R6-0 第 5、6 条的无害/只读命令（逐次记账）；浏览器：仅用于确认预览副本可打开，≤ 4 次。
- 写集：R5 写集 + R6-1 所列文件 + `worldgen.rs`（仅 R6-2 第 3 条授权范围）+ `adapters/html/core/render-stream.js`（R6-0）+ 相应测试。`road_alignment.rs`、`road_profile.rs` 只读。
- 不改 zoning/reserved/保护/水域/需求/退界/reachDistance/ratio/尺寸/坡度/填方上限/走廊宽度；不做 Git 写；不改 `F:\modenmapbox`；除 R6-0 规定的恢复外，不写任何历史证据目录。

## 交付

`r6/report.json`（按报告协议，逐条回应 P-1～P-4、R6-0～R6-4）、`r6/checkpoint.json`、`r6/write-set.json`、`r6/runs.jsonl`、`r6/process-violations.json`、`r6/road-129-130-diagnosis.json`、`r6/cli-build-binding.json`、`r6/full-source-hash-set.json`、`r6/1km-candidate/preregistration-v3.json`、离线重放结果、完整源码差异。

状态：`delivered_for_inspection` 或 `partial + 具名阻塞`。不得宣称 B01 通过。
