# B01-T2 R2 检查裁定 + R3 批量任务（给 V41F）

日期：2026-09-29。检查员/项目经理：Claude。执行：V41F。
上位：`dsh-b01-t2c-r2-batch-task-20260929.zh-CN.md`（SHA-256 `ffb71cb4…db45`）、`dsh-b01-t2-production-chain-continuation-20260929.zh-CN.md`（`20DA5EE4…1542`）。本文件未改动的禁止项全部继续有效。

**请严格按任务书完成任务，不得因为其他任务无关问题擅自中断或停止部分步骤；上下文压缩、会话切换、摘要丢失或可修复的驱动问题不是跳过必测步骤的理由。需要中断时先写 checkpoint，恢复后继续。**

---

## 第一部分：R2 检查裁定

检查证据：`bevy/work/collaboration/inspector-b01-t2c-r2-20260929/`（`inspection-process.json`）。检查员 cargo 3 次（1 次因突变脚本未生效作废）；世界 0、CLI 0、Git 写 0。

### 结论：partial，整体质量明显提升；有 1 项必须回退

| 项 | 裁定 | 依据 |
|---|---|---|
| 归因更正（3 个红测是 T2 回归） | **接受**。诚实推翻上轮结论，隔离对照方法正确 | 隔离副本 3/3 绿 |
| 4 个语义文件 hash | 与 write-set 终点一致；road_alignment/road_profile 未变 | 检查员复算 |
| 道路 4 套件 | **独立复现通过**：44 passed / 0 failed / 6 ignored | `road-suites.log` |
| 12m 基础 tie-in 平台 | **接受**（≥ 扩展合同的 8m 下限，≤ 长度一半，属于更严不是放宽），但须与扩展合同常量绑定，见 V2 | 源码 |
| road_extensions 接缝判定放宽 | **不接受，须回退**，见 F-A | 检查员突变实验 |
| C3 身份 = 规划来源、可信基线锁 profile/端点 | **接受**。mut-b（身份回退→`supply phase needs a base road domain`）、mut-c（去掉 profile 锁→篡改未拒）可作修复前红的等价证据 | 日志 |
| F4/F5/C4 五要点 | 接受为 fixture 级；第 2 点的突变是改 fixture 不是改生产代码，V1 补 | c4-matrix |
| W3 冻结输入 | 接受：manifest hash 与 b8 一致，唯一变量为源码 | frozen-input.json |
| W3 对比器 | 部分接受：漏找了 b8 已有的审计结果，见 F-C | — |
| W3 预注册判据 | 须修订为 v2，见 F-D | — |
| W4 确定性 | 接受 | 两次摘要一致 |
| 20 个 g15–g17 失败 | 分类属实（都是 `create_new` 文件已存在）。**但这意味着这 20 个测试的计算逻辑在当前源码下完全没执行**，不能算作已观测，见 V3 | 全库日志 |
| frontage_supply notObserved | 不接受为终态。这 7 个测试读取冻结世界，不生成世界，可以跑，见 V4 | t3-summary |
| source-diff-vs-round-start | **不合格**：只含 auto_roads、road_parcel_feasibility 两个文件，且基准是 T2 前隔离副本而非本轮起点；缺 road_network、road_extensions | patch 文件头 |
| runs.jsonl | 35 条记录，logs/ 34 份，差 1 份未对应 | 计数 |

### F-A（必须回退）：接缝判定放宽不是修绿所必需

R2 把 `road_extensions::append_one` 从“接缝半径内**每个重叠格**两路面等高”改成“只在**接缝点**等高”。检查员实验：**保留 12m 基础平台，只恢复逐格等高检查**，`road_network` + `road_extensions` 28 passed / 0 failed（`mutation-cell-rule.log`）。

因此：
1. 放宽不是修复 3 个红测的必要条件，真正起作用的是 12m 平台；
2. 放宽允许路口重叠区内两个路面不等高，也就是在路口里留下台阶，属于削弱路面连续性合同，违反“不放宽工程合同”；
3. 没有任何测试能区分放宽前后，说明路口重叠区连续性目前没有负例覆盖。

### F-B：12m 是魔数

注释写的是“覆盖扩展的 9m graded band 与 11m footprint”，但数值和扩展合同（允许宽度 8/12/16、路肩 3）没有代码层面的绑定。将来扩展允许 20m 路时会静默失效。

### F-C：b8 审计结果其实存在

`bevy/work/collaboration/inspector-b-mainline-20260929/b8-readonly-source-copy/scale-audit.json` 与 `quality-report.json`（两者 `status = fail`，附带 stdout/stderr 与 `b8-audit-results.json`）。对比器应读取它们，并核对其 `worldRevision` 与 b8 世界一致；这样“出现 b8 没有的失败类别”这一回归判据才可执行。

### F-D：预注册判据偏松

- 把 `roadsideSites` 列入“正增量”不对：roadside sites 由基础网络的旧逻辑生成，与 profile→frontage 链无关，单独增加不能说明 T2 起效。
- b8 的真实缺口是 urban homes 0/120、shops 1/24，原因 `no_feasible_frontage`，唯一临街道路还是自动扩展路 `generated.road-auto.scene-0.4`。T2 的业务目标就是让基础路也能提供临街站点，判据应直接对准这一点。

---

## 第二部分：R3 批量任务

证据目录：`bevy/work/collaboration/ds-b01-t2-production-chain-continuation-20260929/r3/`。不覆盖 r2/ 及更早内容。

### V1 恢复路口重叠区逐格连续性 + 负例

1. 在 `append_one` 恢复逐格等高检查（接缝半径内每个重叠格：新路面 == 旧路面，容差保持 0.00001）。R2 新增的“空接缝集直接拒绝”和“接缝点等高”可以保留（等价或更严）。
2. 新增负例 fixture：扩展路与带 profile 的基础路重叠，重叠区内有格子落在基础路平台以外、路面已下降 → 必须拒绝。另加正例：16m 扩展接到 12m 平台上 → 接受。
3. 生产代码突变：删掉逐格检查 → 负例必须变红，日志存档后还原。
4. 如果你认为存在某个**合法**路口，12m 平台加逐格规则会误拒：给出具体坐标、宽度、各格高度差作为反例 fixture，提交检查员裁定；**不得先行放宽**。
5. 补 C4 第 2 点的生产代码突变：去掉 F4 的 `validate_entity` 调用块 → 对应测试变红。

### V2 平台宽度与扩展合同绑定

1. 在 `road_extensions.rs` 用具名常量或函数导出扩展路口的最大占用半径（由允许的最大路宽与路肩推出，包括 graded band 与 transport footprint 两者的最大值），`road_network.rs` 的基础平台从它推出（可以取推出值与 12 中的较大者），不再单独硬编码 12。
2. 测试：对扩展合同允许的每一种宽度，断言基础平台 ≥ 该宽度的占用半径。
3. 短边：当 `length/2` 小于所需平台时，该边端点无法安全接宽扩展。必须具名记录（例如 `base_platform_short_for_extension_width`），扩展在该处以具名理由拒绝，**不允许静默接受**。给出 fixture。

### V3 g15–g17 证据写出器：让计算真正执行

1. 仅改测试代码：默认输出到一次性目录（`$CARGO_TARGET_DIR/test-evidence/<test-name>/<run-id>/` 或系统临时目录），历史目录只有在显式设置环境变量（如 `GLYPHWEAVE_WRITE_HISTORICAL_EVIDENCE=1`）时才写，且仍然拒绝覆盖。**不删除、不改动任何历史证据文件，不 `#[ignore]`，不削弱断言。**
2. 全库 `cargo test -p glyphweave-core --lib` 必须 0 failed。
3. 把这 20 个测试本次新算出的 JSON 与 2026-09-20 的历史文件逐个做语义对比（忽略时间戳、源码 hash 等元数据字段，要列出忽略了哪些字段），输出 `r3/g15-g17-drift.json`：每个文件 identical / drift，drift 须逐项归因（基础路带 profile 导致 / 其他）。这是信息性结果，不是修改历史的依据。发现与 T2 无关的漂移，写入阻塞。

### V4 frontage_supply 真实运行

1. 用 `--ignored` 运行 frontage_supply 的 7 个测试（以及 road_parcel_feasibility 的 2 个 ignored）。它们读取冻结世界，属于离线读取，**不是世界生成**。
2. 运行前后对它们读取的冻结目录做 hash 清单，证明没有写入；若某个测试会写入冻结目录，停止该测试，按 V3 的方式改为写到一次性目录后再跑。
3. 失败处理：生产回归 → 在写集内修复；断言固化了 T2 前的世界事实（例如“精确复现已发布计划”），而差异完全来自基础路带 profile → 可改为 T2 感知、强度不低于原断言的断言（例如：逐项列出预期差异并断言其余部分完全一致），在 `r3/frontage-supply-diagnosis.json` 逐条说明原意图与新断言。禁止删测试、禁止继续 ignore 了事。
4. 报告 frontage_supply 的 passed/failed，状态从 notObserved 变为已观测。

### V5 1km 冻结输入的离线阶段重放（最后一次 1km 运行前的预测）

目的：不消耗额度，先看当前源码在 b8 同一输入上，基础路 profile → stations → sites 这条链能走多远。如果大部分基础路被拒，要在用掉最后一次 1km 之前修好。

1. 新增 `#[ignore]` 测试（或 core 下的测试工具），读取 `r2/1km-candidate/frozen-input/b01-1km-probe.manifest.json`，在进程内调用**生产函数**完成：地形/zoning → 道路网络计划（含基础 profile 挂接与拒绝）→ 扩展/自动路 → frontage stations → candidate sites；如果开发计划阶段在进程内成本可控，一并算出计划的 homes/shops。
2. **不写世界产物、不调用产品 CLI、不生成 gemap/sidecar**；输出只有 `r3/1km-stage-replay.json`，字段与 compare.mjs 对齐：基础路总数、带 profile 数、拒绝按类别计数（逐条列出 edge ID 与原因）、每条道路的 stations 数、frontage 候选数、site 数、homes/shops 计划数。
3. 证据层级标为 `offline-stage-replay`，**不是 production-world**，不能用于宣称城市产能增长。
4. 同一输入跑两次，输出字节一致。
5. 结果分析 `r3/1km-stage-replay-analysis.json`：
   - 如果基础路拒绝占多数，按类别给出根因。属于本轮写集且不违反合同的，修复后重跑重放；属于合同/zoning/地形本身的，如实记录，**不改需求、zoning、尺寸、比例**；
   - 如果 profile 路很多，但 stations 或 sites 仍接近 b8，找出链条断在哪一环（stations 空？candidate 被 zoning/reserved/退界拒？），给出具名断点。

### V6 对比器与预注册 v2

1. compare.mjs 读取 F-C 中的 b8 审计文件，核对 worldRevision 与 b8 世界一致后使用；输出 b8 的失败类别清单。
2. 新写 `r3/1km-candidate/preregistration-v2.json`（v1 保留不动，v2 注明取代 v1 的原因），判据改为：
   - **正增量（主判据）：** 基础路带 profile > 0，且 **frontage 实现数（urban homes + shops 已实现）严格增加**，且临街道路 ID 中至少一条是基础路；ledger 无重复计量或负面积；
   - **辅助观测（不单独构成正增量）：** frontage stations、候选数、roadside sites、realized buildings、realizedM2；
   - **空增量：** 基础路带 profile > 0，但 frontage 实现数不增 → partial，交断点诊断，不扫参、不重跑；
   - **回归：** frontage 实现数、realized buildings、realizedM2 任一下降，或 scale-audit/quality-report 出现 b8 清单之外的失败类别；
   - exit code 与 JSON status 双记录，status=fail 永不按 exit 0 接受。
3. v2 与 V5 的阶段重放结果**分开**冻结：v2 在看到 V5 结果之前冻结（记录冻结时间与 hash，写入 checkpoint）；V5 的结果只作为预测附在后面，不回改判据。
4. run.ps1 更新为引用 v2，授权闸门保持不变。

### V7 账目与差异补正

1. 重做相对本轮起点的源码差异，包含全部被修改的文件。若 R2 起点副本没有保存，用 R1 起点已知 hash 加 T2 前备份如实说明基准，不得把 T2 前副本称为“本轮起点”。
2. 核对 r2 的 35 条记录与 34 份日志，找出缺的那一份，如实标注（例如 `log_missing`），不补造。
3. r3 的 runs.jsonl 从一开始就保持记录与日志一一对应。

### V8 收口回归

所有修改完成后：15 模块定向 + 全库 `cargo test -p glyphweave-core --lib`（0 failed）、确定性两次运行、黄金快照比对（快照若因 V1/V2 合法变化需要更新，只能用 `GLYPHWEAVE_UPDATE_GOLDEN=1`，并在报告中列出新旧差异与原因）、重新构建 CLI 并更新 exe↔源码 hash 绑定（只构建不运行）。

---

## 预算与硬边界

- cargo：软计划 **60 次**（V1–V8 合计，含突变、重放、隔离）。达 45 次时在 checkpoint 汇报；有新诊断且在写集内可继续；禁止无新诊断原样重试。单次长任务设合理超时并保留进度。
- 冻结世界只读测试：允许（V4、V5），必须有前后 hash 清单。
- 世界生成 0、产品 CLI 运行 0（构建允许）、浏览器 0；512m 已耗尽；**1km 最后一次额度仍未授权**；6×10km 禁止。
- 写集：R2 的 4 个语义文件；`auto_roads.rs` 本轮额外允许 V3 的测试代码修改；`frontage_supply.rs`、`road_parcel_feasibility.rs` 允许 V4 的测试代码修改；V5 的新测试放在 core 测试模块或 `crates/core/tests/`。其余文件只允许 test-only 与机械同步。`road_alignment.rs`、`road_profile.rs` 只读。
- 不改 zoning/reserved/保护/水域/需求/退界/reachDistance/ratio/尺寸/坡度/填方上限/走廊宽度；不做 Git 写；不改 `F:\modenmapbox`；不回写历史证据。

## 交付

`r3/report.json`（按 `dsh-inspector-reporting-protocol.zh-CN.md` 结构，逐条回应 F-A～F-D 与 V1～V8）、`r3/checkpoint.json`、`r3/write-set.json`、`r3/runs.jsonl`、`r3/t3-summary.json`、`r3/cli-build-binding.json`、`r3/1km-stage-replay.json`、`r3/1km-candidate/preregistration-v2.json`、完整源码差异。

状态：
- `delivered_for_inspection`：V1–V8 全部完成，全库 0 failed，15 模块无 notObserved（road_profile 无单测的静态证据可接受），预注册 v2 已冻结，阶段重放已完成；
- 否则 `partial`，逐项列出具名阻塞与下一步最小测量。

不得宣称 B01 通过、城市产能增长或 1km 已授权。阶段重放的数字不能写成生产世界结果。

## 检查员下次核验

1. 逐格连续性已恢复，负例经生产代码突变验证；
2. 平台宽度由扩展合同推出，短边有具名拒绝；
3. 全库 0 failed，历史证据文件 hash 未变；g15–g17 漂移已逐项归因；
4. frontage_supply 已实际运行，冻结目录前后 hash 一致；
5. 预注册 v2 的冻结时间早于阶段重放结果；
6. 阶段重放两次字节一致，拒绝逐条具名，断点有诊断；
7. 账目与日志一一对应，差异文件完整；
8. 世界/CLI 运行仍为 0。
