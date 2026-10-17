# B01-SUPPLY-R1 独立审查（2026-09-17）

## 结论

**partial / needs-changes。35边基线、按request ID比较数量、共享RequestBudget和真实landcover/access调用是有效进展；但超时事务、跨轮已实现身份保护、实现审核等价性尚未收口。**

不是全盘重做，也不重开已验收R2/W1/R6。本轮保持原自然/固定输入，未发现偷偷改carve；0/24如实，没有新正式bake，均认可。不得将“仍未构造强制生产回退负例”当成已验收的等价覆盖。

检查员只读业务源码/世界、执行小型复验和写文档，没有改业务代码、启动agent、Git写操作或F盘改动。

## 1. 独立证据与测试

HEAD：`fb36dc4f9fdd38e6aa9c5a89676a65b96e5001b1`。

证据目录：`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-supply-r1-20260917/`。

- 当前receipt实际SHA256：`d84a59bee0d89765db3756ac78b1f4e306f9216a47c8361afba31c7e9a0f50f5`。本轮用户未给预期hash，故只报告读取值。
- 19项path/log/hash/bytes绑定全部匹配。
- W1登记7份旧文件hash不变，上一轮receipt仍为检查员记录的`69ff2c9b…`。
- R6模块hash保持`fb5dd93504e29acd609d07643e9276081a30dd536f7a2ae4ec7fa06027baaa34`。
- 独立core `worldgen::auto_roads`：15 passed，5.55s。
- 独立core `worldgen::frontage::`：10 passed，1.09s。
- 独立只读ignored `frozen_w1_demand_led_solve_reports_real_verified_parcels`：1 passed，8.46s。
- 独立只读ignored `frozen_w1_joint_realization_baseline_is_measured_once`：1 passed，3.22s。
- 独立只读ignored `diagnosis_replays_production_and_carries_budgets_across_roads`：1 passed，2.66s。

运行前确认这些诊断不写冻结文件；stdout放检查员目录。命令均为`cargo test --manifest-path bevy/Cargo.toml -p glyphweave-core <filter> -- --test-threads=1`；ignored测试另加`--ignored --exact --nocapture`。未重跑全部5项诊断、全CLI、全worldgen或全settlement，没有新正式世界。

## 2. 已确认的进展

### 正确的冻结基线

独立append重放结果：35边、128状态、0接受、`candidate_state_budget_exhausted`、`sweepExhausted=false`，planningSeconds=8.3088637。旧两条auto道路不再被v3替代，新增命名空间区分road-supply和road-auto。

基线实现报告：35边/27节点/21site、21建筑实现、0新frontage、yardUnresolved=13m²、requiredUnconnected=0。development/landcover为incomplete，programLandcover为pass。不能将基线整体说成全质量pass，但其旧缺额可以单独保留。

### 旧恒false守卫已修、实现阶段已增加

守卫开始按request ID比较实现数量，而非`total < saturating_sub(total,baseline)`。新链确实调用了development.verify、landcover.realize/verify和AccessGraph，不再只有plan函数。已有小正例报告2个实现地块/yard0，也通过本轮15项测试。保留这些改动。

### 诊断累计预算已改善

生产与诊断共享RequestBudget，冻结场景缩小预算后等价测试通过。该测试仍在零接受的冻结场景，不能宣称已证明多路正供给的全部交互；下一轮可用已有小正例补两轮状态覆盖，不重复跑自然困难场景。

## 3. P1：超时仍可保留成功前缀并返回Ok

位置：`auto_roads.rs::append_supply_aware`。

代码在每次候选验证后立即`*plan = staged`，后续如果clock触顶，只设置report.incomplete并break；函数末仍给plan写报告且返回Ok。没有会话起始快照恢复。`realize_candidate`之后、实际写回之前也没有再检查截止，因此耗时验证跨越截止仍可能提交；若需求刚好满足，下轮先判断满足再看clock，还可能没有incomplete标记。

现有`resource_clock_expiry_never_publishes_a_success_prefix`只设置`trip_at=0`，覆盖“任何提交前即超时”，不能证明“第一条已提交之后超时”或“实现阶段中超时”。生产worldgen/road_network中没有消费`.incomplete`来阻止继续bake的路径（查找代码可见）。

这是源代码控制流结论，本轮未声称已实际构造晚超时正式bake。要求是事务安全，不是新增调度系统：本轮追加在局部副本完成，墙钟中止时完整回退且返回不允许发布的错误/独立诊断结果；已验证的legacy基线可保留。成功身份/计划不得由墙钟决定。

## 4. P1：跨轮身份保护和请求归属仍错误

### 4.1 只保护会话开始时的身份，没有保护后来已经提交的身份

baseline_sites与baseline_realization只在函数开始定义一次。每次commit后更新session数量/ID，却不更新已承诺几何快照。下一轮守卫对session只查按请求数量，因此可以丢失第一轮新增ID、换同请求的另外两个ID，数量有增仍通过。

此外`*plan=staged`只保存道路，未保存验证时的新增frontage site；下一轮`committed_sites(&session, plan)`从道路版plan取实体，通常取不到上一轮新增地块，测量阶段所谓“已实现地块作为占用”也就失效。

### 4.2 已有结构化归属，却从字符串猜request

`apply_realization`没有使用`realized_ids_by_request`，而用`id.rsplit`和`id.contains(".{request}.")`归属。

独立抽取当前**原样的守卫/归属函数**，用最小接口桩运行（不是完整bake），得到：

- 合法request IDs：`homes`、`scene-0`，生产frontage parser确认都可接受。
- 一栋实际属于homes的ID：`generated.frontage.scene-0.homes.generated.road-supply.scene-0.0.24.positive`。
- 因scene部分也包含`.scene-0.`，当前函数把这同一个ID同时写入homes和scene-0两个需求。
- 另一探针模拟初始baseline_sites为空、第一轮已实现first-round-home；第二轮候选换成replacement+extra，同请求数量1→2，当前守卫返回Ok。

详见guard-probe.rs/json。为调用私有函数，探针复制当前函数体并以最小接口桩代替SupplySession/Plan，不修改生产文件；这是守卫级反例，不冒充自然世界端到端缺陷实跑。最初尝试直接链接私有site.entity接口编译失败，最终探针通过接口桩执行；结论只覆盖所示逻辑。

修复应原子维护当前已实现的实体/地块/入口与request精确映射，每轮成功更新；不改全局ID命名合同，也不限制合法用户ID掩盖错误。

## 5. P1：实现信用仍不等价于完整生产审核

### 5.1 新road_support_status跳过基础道路

`frontage_supply.rs:449`之后的私有loop对没有road_profile的实体直接continue。它是新增独立审核实现，而非调用已有完整道路叶子审核；只检graded band/fill/cut，未覆盖所有固定base道路支撑、extension设计源重建等已验收合同。

`audit_node_heights`校验节点与设计高程，不等于每个base road支撑格的final正确。测试中“base域与重新生产plan一致”也不证明候选final没有改变base支撑。不能将其称为完整继承了R6审核。

### 5.2 候选final与正式bake存在已披露但未解决的基面/量化差异

candidate_final_terrain从`natural.height`(f32)重建；seed参数被忽略。执行者报告实际路径存在`actual=7.998, expected=7.9975`分歧，并因此没有使用原joint审核。这是实现信用可信性的待决问题，而非可以长期用另一套较窄检查绕过的问题。

本次冻结0接受，所以没有错误的24证书；有正供给时仍需证明预检与正式生成对同一候选支持/院落/入口采样一致。不要提高容差，不要求重开R6全部工作；可从现有生产生成/审核抽出一个不递归的共享叶子接口，供新供给协调调用。

### 5.3 总缺额不增加，不能证明每个新增院落实现

当前yard仅判断候选总unresolved≤初始13m²。若旧地块缺额减少、新地块出现等量缺额，总数仍可通过。road/development/landcover部分失败对比也仅比较条数，可能用修一个旧失败抵消一个新失败。

需至少按既有/新增parcel或surface来源分离缺额：旧13m²不是新地块的可花费额度；新增地块yard/入口必须独立满足，新失败身份不能被同长度列表抵消。源码中已有surface parcel_id，可复用，不建新的面积审计平台。

## 6. 状态与后续

- 35边冻结追加上下文与本轮0收益结果认可（有限128预算，不是全局无解）。
- RequestBudget、按request比较计数、真实地表调用认可为进展。
- 超时不提交、跨轮完整承诺、生产实现信用尚未通过；不签收B01-SUPPLY。
- 不修改全局carve语义；其真实冲突证据保留，但先让新供给程序能可靠提交/拒绝。

下一任务：`G:/difyllmwiki/GlyphWeave/docs/collaboration/ds-v41f-b01-supply-r2-task.zh-CN.md`。只收口上述三类，不再重跑128自然扫描，不跑大图。
