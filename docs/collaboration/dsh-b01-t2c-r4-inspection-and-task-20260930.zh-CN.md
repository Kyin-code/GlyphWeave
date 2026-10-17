# B01-T2 R3 检查裁定 + R4 任务（给 V41F）：放行 1km 前先关掉路边院落退化

日期：2026-09-30。检查员/项目经理：Claude。执行：V41F。
上位：`dsh-b01-t2c-r3-inspection-and-task-20260929.zh-CN.md`（`7406e946…914d`）、`dsh-b01-t2c-r3-addendum-1km-authorization-20260930.zh-CN.md`（`a81bf972…e15b`）、`dsh-b01-t2-production-chain-continuation-20260929.zh-CN.md`（`20DA5EE4…1542`）。未改动的禁止项全部有效。1km 授权仍有效，**G3 本次暂不放行**，原因见 R-1。

**请严格按任务书完成任务，不得因为其他任务无关问题擅自中断或停止部分步骤；上下文压缩、会话切换、摘要丢失或可修复的驱动问题不是跳过必测步骤的理由。需要中断时先写 checkpoint，恢复后继续。**

---

## 第一部分：R3 检查裁定

检查证据：`bevy/work/collaboration/inspector-b01-t2c-r3-20260930/`。检查员 cargo 1 次（全库）；世界 0、CLI 0、Git 写 0。

| 项 | 裁定 |
|---|---|
| 全库 | **独立复现**：624 passed / 0 failed / 27 ignored |
| exe 与 7 个绑定源码 hash | 与 cli-build-binding 一致 |
| V1 逐格连续性 | 已恢复，源码确认；负例与突变日志接受 |
| V2 平台由合同推出、短边具名拒绝 | 接受。`road_extensions.rs:325` 传入的 `old.width_m` 对道路实体就是长度（`entities()` 中 `width_m = e.length_m`），语义正确但命名易误读，见 R-5 |
| V3 证据写出器改道、g15–g17 漂移归因 | 接受 |
| V4 frontage_supply 实跑、冻结目录零写入 | 接受；两处断言改写的强度说明合理 |
| V5 阶段重放 | 接受为 plan/frontage 阶段预测：26 条基础路全部带 profile、0 拒绝，商铺 11/24，10 条临街道路里 9 条是基础路。**但重放止于 frontage，没有覆盖开发计划审计、院落/地表实现和 ledger** |
| 预注册 v2 冻结顺序 | 文件写入 16:19:32Z 早于首次重放 16:21:03Z，**顺序成立**；但文件内 `frozenAtUtc` 写成 17:20Z，晚于实际写入，是记录错误，见 R-5 |
| 住宅 0/120 断点归为 zoning 事实 | 接受。93% 拒绝为 `wrong_purpose_or_protected`，本轮不改 zoning/需求 |

### 为什么暂不放行 G3

V4 的归因揭示了一个**T2 造成的既有站点退化**，而执行者只把它记成了“风险”：

- 在 W1 冻结基线上，T2 前发展审计不是 fail，T2 后变成 fail；
- 院落未实现面积从 13 m² 涨到 4419 m²，其中 **21 个既有路边地块每块 208 m²（共 4368 m²）** 从已实现变成未实现；
- 隔离 T2 前副本 7/7 通过，证明这是 T2 引起的。

这正是上位任务书 §5“配套地形/站点检查”要求处理的情况：“新增 profile 若使既有正确站点退化，不能放宽支撑或把站点删掉……不试世界掩盖它”。它不是 zoning 事实，而是本轮改动的后果，应在跑世界之前修好。

如果现在就跑最后一次 1km，大概率出现：临街商铺增加，但路边院落大面积退化，realizedM2 下降 → 按预注册 v2 判为**回归**；而且 supply 提交门拒绝 fail 候选，可能连商铺都落不成建筑。用掉最后一次额度去测一个已知问题，不划算。

---

## 第二部分：R4 任务

证据目录：`bevy/work/collaboration/ds-b01-t2-production-chain-continuation-20260929/r4/`，不覆盖 r3/ 及更早内容。

### R-1 路边院落退化：定位根因

1. 在 W1 冻结基线上逐个列出这 21 个路边地块：地块 ID、所邻道路 ID、院落被拒的具体规则与数值（例如支撑高程、路面高程、入口高程、坡度、填方）。T2 前隔离副本上同一地块的同一组数值并排列出，写入 `r4/roadside-yard-regression.json`。
2. 回答：退化是因为哪一步仍假设“基础路在 `level_m` 处处等高”？候选位置包括 `road_network.rs` 中按 `level_m` 生成 roadside_sites 的循环（约 :512 起）、院落/地表实现读取的道路高程、入口平台的对接高程。给出文件:行。

### R-2 修复

目标：带 profile 的基础路旁，既有路边地块的院落实现**不低于 T2 前**，且全部工程合同强度不变。

允许的修法（按优先顺序）：
1. 调整调用顺序或基线衔接，使路边站点、入口、院落读取道路的**权威 profile 高程**，而不是 `level_m` 常量；
2. 路边站点放置在道路 tie-in 平台或 profile 允许的位置时，正确计算入口对接高程；
3. 若以上都需要修改 `settlement/` 下院落/地表实现或站点工程算法，本任务**授权**修改这些生产逻辑，限于“从读取 `level_m` 改为读取权威 profile 高程”这一类改动，须在写集中逐处登记。

禁止：把基础路 profile 压平或局部压平来迁就站点；删除或挪走路边站点；放宽支撑、坡度、填方、院落面积或入口规则；把院落未实现改名成“有意留白”；改 zoning/需求/尺寸/比例。

若确认必须改变工程合同本身才能恢复，停在该项，写出具体合同、数值与反例，交检查员裁定；其余任务继续。

验证：
- W1 冻结基线上院落未实现面积回到 T2 前水平（13 m² 量级；若不能完全回到，逐块说明剩余原因），发展审计不再因院落失败；
- 路边地块新增 fixture：非平坦地形 + 带 profile 的基础路 + 路边地块，院落实现成功；生产代码突变（改回读取 `level_m`）使其变红；
- frontage_supply 中本轮 pin 住的 T2 基线数值（4419 m²、23 个缺口等）会随修复改变：按 V4 的方式更新为新值，同时保留 T2 前参照，并说明变化来自 R-2。

### R-3 把 1km 阶段重放延伸到开发审计和 ledger

1. 扩展 V5 重放：在进程内继续执行开发计划 → 开发审计 → 院落/地表实现 → 用途 ledger（仍然**不写世界产物、不调 CLI**），输出 `r4/1km-stage-replay-full.json`：
   - 在 V5 已有字段之外，增加：development plan records、预计 realized buildings、发展审计 status 与失败原因清单、院落 planned/realized/unresolved m²、ledger realized/unresolved m²；
   - 与 b8 对应字段并排。b8 发展审计本身就是 fail（原因为 `remaining_land_has_no_development_or_conservation_plan`、`yard_reserved_but_landcover_not_realized`），比较重点是**有没有新的失败原因、院落和 realized 面积有没有下降**。
2. 若某一阶段无法在进程内重放（依赖只在 CLI 生成流程中存在），写明断在哪个函数，用最接近的生产函数组合替代，并标注覆盖缺口。
3. 两次运行字节一致。
4. 预注册 v2 **不改**。重放结果只作预测，附在后面。

### R-4 放行条件自检

在 `r4/g3-readiness.json` 中逐条自检，全部满足才请求放行：
1. R-2 完成，W1 冻结基线院落不再因 T2 退化；
2. R-3 重放显示：相对 b8，realizedM2 不下降、院落 realized 不下降、发展审计没有新的失败原因、临街实现数增加；
3. 全库 0 failed，15 模块无 notObserved（road_profile 静态说明除外）；
4. CLI 重新构建并绑定。

若第 2 条不满足且原因不在可修范围，交诊断，由检查员决定是否仍然运行（运行结果可能是“回归”，但也可能有信息价值）。

### R-5 记录与绑定补正

1. preregistration-v2 不改内容；在 `r4/prereg-v2-timestamp-correction.json` 说明 `frozenAtUtc` 与实际写入时间不一致，给出文件系统写入时间与首次重放时间。
2. `road_extensions.rs:325` 附近加一行注释说明道路实体 `width_m` 即沿线长度，或改用更清楚的取值方式（行为不变）。
3. **源码绑定扩大到全集**：参照 b8 的 `generation-provenance.json`（记录了 90 个源码文件的 hash），CLI 构建绑定改为记录 `bevy/crates/**/src/**/*.rs`、各 `Cargo.toml`、`Cargo.lock` 的全部 hash，并记录构建 profile（debug/release）。run.ps1 与 `1km-go.json` 校验全集，而不是 7 个文件。
4. 记录 b8 所用 exe 的构建 profile（若可从 b8 记录查到）；若与本轮不同，说明对结果可比性的影响（功能结果应一致，耗时不可比）。

### R-6 收口回归

全部修改后：15 模块定向 + 全库 0 failed、frontage_supply 与 road_parcel_feasibility 的 ignored 测试再跑一遍（冻结目录前后 hash 一致）、确定性两次一致、黄金快照（若 R-2 使分级链输出合法变化，只能用 `GLYPHWEAVE_UPDATE_GOLDEN=1` 更新并列出差异）、CLI 重建并按 R-5 全集绑定。

---

## 预算与硬边界

- cargo：软计划 **80 次**，达 60 次在 checkpoint 汇报；有新诊断且在写集内可继续；禁止无新诊断原样重试。
- 冻结世界只读测试允许，必须有前后 hash 清单。
- 世界生成 0、产品 CLI 运行 0（构建允许）、浏览器 0，直到检查员写出 `r4/1km-go.json`。**不得自行写放行文件。**
- 写集：R3 的全部写集；加上 R-2 第 3 条授权的 `settlement/` 院落/地表实现与路边站点相关生产代码（逐处登记）。`road_alignment.rs`、`road_profile.rs` 只读。
- 不改 zoning/reserved/保护/水域/需求/退界/reachDistance/ratio/尺寸/坡度/填方上限/走廊宽度；不做 Git 写；不改 `F:\modenmapbox`；不回写历史证据。

## 交付

`r4/report.json`（按报告协议，逐条回应 R-1～R-6）、`r4/checkpoint.json`、`r4/write-set.json`、`r4/runs.jsonl`（与日志一一对应）、`r4/roadside-yard-regression.json`、`r4/1km-stage-replay-full.json`、`r4/g3-readiness.json`、`r4/cli-build-binding.json`（全集）、完整源码差异。

状态：`delivered_for_inspection` 或 `partial + 具名阻塞`。

## 放行后

检查员复核通过后写 `r4/1km-go.json`，然后按 R3 授权附件 §2～§5 执行唯一一次 1km 运行、两次审计、判定与成果展示（路径由 `r3/1km-run/` 改为 `r4/1km-run/`）。成果展示由 V41F 直接交给项目负责人查看。
