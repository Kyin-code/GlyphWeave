# B01-G5 junction9 对照独立审查（2026-09-18）

## 结论

**accepted-planning-progress-with-errata**：接受两态AB/BC生产append通过、共享交集196格零不匹配，以及BC测量预测增量为0的限定结果。G4接点匹配阻断确已在该参数对照中解除，是实际进展。

不签收B，不推进C；无真实新增parcel见证。完整候选final、全部旧路final支撑、development/landcover/access联合realization未运行，因此不能将“道路规划追加通过”扩大成“最终世界工程审核全部通过”。推荐状态写road_append_pass_frontage_measure_zero，full_engineering_audit=not_run。

## 1. 检查员实际验证

- receipt SHA256：c5b7370a603b3e2cd51489a49f90ec03fe924f93899da40b5679804fb5b8ab71。
- 当前changedFiles/evidence中的12项path+bytes/hash绑定匹配；11旧回执、8份G4登记证据、7冻结输入、7其它源码hash匹配。**另有preRegisteredTable.sha256算法标注错误，见下节；不能因此称整个receipt所有hash字段一致。**
- 将新增G3以来测试区域从内存副本去掉、恢复两行测试CRLF后，auto_roads.rs精确还原G1 hash，确认生产代码未改；未实际修改任何源码。
- run.json与run.log内JSON完全相同；两态baseline hash前后一致、best=null/incomplete=false、candidate realization和baseline realization均0。
- 读取代码控制流确认：measurement_zero_gain只会在AB和BC均append成功后出现；两态196 compared/0 mismatched/0 missing，B接点关键采样为3000mm。
- 测量函数会调用budget.accept更新site_ids，trial.realized_totals前后差值在这里是预测接受数。对本次0结果没有发现“计分器永远零”的缺陷；但它不是实际已实现信用。
- 测试二进制准备一次；--exact独立运行单位换算/方向平台范围两个小测试，2 passed/0 failed。
- 为核实算法，独立编译并运行一个只读文件BLAKE3工具；只读rule-table字节，不触发任何世界函数。
- 未独立重跑自然harness、生产design/append/infer/realize、bake/CLI世界/大图/浏览器。自然结果验收依据为日志+源码路径+绑定，不冒称第二次自然观测。

## 2. 明确错误：BLAKE3被写成SHA256

rule-table.json实际1465 bytes：

- **SHA256**：8f2efc8c52315765bb3230eec1584207848cf95bce2f56600a74ea5ccb5d8327
- **BLAKE3**：75900cd8564dcf897e5596790df3e14f4f02a37be3238ef065df8b9184981e89

run.tableHash等于后者，是源码实际调用blake3生成的值。receipt.evidence数组记录SHA256正确，但receipt.preRegisteredTable.sha256、结果文档§1及回复把BLAKE3当SHA256，且称二者相同，明确错误。

这不是发现文件被修改；是同一份文件的两种算法混淆。旧table/run/receipt不可覆盖，在新勘误或新回执写清sha256、blake3、bytes，删除“同值”的说法，不为修正hash重新跑自然实验。

## 3. 内容拒绝证据只到首个总类，尚不能定因

G5新候选阶段仅测量BC街段：每态两请求各8次站点/侧别尝试，总16；两态总32次尝试、4次measure调用，预测[0,0]。

harness只保存每态firstReason，并丢弃measured.trace.rejected、rejection_details、samples。生产frontage::support_check把三种不同情况都归入natural_support_or_fill_limit：
1. natural_above_design_level：自然面高于设计面，按no-cut合同拒绝；
2. fill_exceeds_roadbed_limit：所需填方超过道路基准上限；
3. fill_exceeds_zoning_capacity：超过该点zone保护距离给出的填方容量。

因此当前能说：两个街段测量均未接受候选，保存的首拒绝总类相同。不能说32次都由同一子原因拒绝，也不能凭这个词决定升/降路面、提高fill、缩小parcel或改地形。

没有测量AB新增路段的潜在地块，也没有跑全部新旧道路的frontage实现。下一步应只对已有两态BC的32次尝试补生产trace，不扩大搜索；本轮结果不代表整个新网络的所有地块可能性已穷尽。

## 4. 工程进展的准确边界

junctionLength9仍是现有输入参数，junctionRadius仍9，未改审核、求解、默认或旧路。这轮生产append证明两条新走廊的设计、自然包络、拓扑接入及支撑交集匹配通过。

由于预测0提前退出，candidate_final_terrain与realize_candidate等后续链没有执行，旧工程支撑在新carve叠加后的保持、真实yard/access等仍未测。尤其历史已有existing_support_changed等阻断，不能在未运行final链时默认为已消除。

“工程通过”可以作为道路追加阶段的简称，但报告/回执应明确roadAppend=pass，fullFinalAudit=not_run，而不是无范围的engineering pass。

## 5. 下一步

任务ds-v41f-b01-g6-frontage-rejection-diagnosis-task.zh-CN.md，仅对原G5两态做一次有界只读诊断重放，输出生产逐站首拒绝note/坐标/自然mm/设计mm/填方容量和分布。该次诊断重放需单独预注册和计成本，不冒充原G5证据，不增加新候选，不realize、不bake，不调参数。

hash勘误、状态分层与trace取证一次完成；不用再单开一轮纯文档修订。B partial/C不推进。

## 6. 检查员证据与边界

目录bevy/work/collaboration/inspector-b01-g5-20260918/：evidence.cjs/json、两项exact日志、table-hash.rs及table-blake3.txt、构建日志、review-decision.json、doc-tree/diff日志。

检查员无生产/测试源码编辑、无子agent、无Git写操作、无F盘写入、无自然实验。只新增审查/下一任务/回复/检查员证据及追加status。
