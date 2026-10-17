# B01-G3-R1 收口独立审查（2026-09-18）

## 结论

**accepted-closeout**。T1结论/预算纠正、T2完整交集诊断、T3晚截止防止见证发布修复均通过本次限定审查；G3-R1修订队列关闭，不再单独加一轮基础修复。

接受的是修订，不是自然增量。原五点无见证、B轴未穷尽、B=3000只证接入段、完整自然正路径未运行，这些边界仍保持。B partial，C不推进。

T4只是一份待执行建议，其中把“C高程自由度”与“移动C坐标/改变街段长度”混在一起，不能整体视作授权。下一任务明确只允许固定V5坐标的B/C高程变化，不准移动C、不改junction/carve。

## 实际复验

- 回执SHA256：c11bb2252e6f04db576b651f018be492fb8f03f1b044dc12e58afc74960df3b6。
- 9项当前回执文件bytes/hash匹配；9旧回执匹配；G3的11个登记证据文件匹配；7冻结W1输入及原authored manifest 930bdc13…匹配。
- 6个其余G3源码hash匹配。内存移除新增G3测试区域、恢复旧测试紧邻两行CRLF后，auto_roads.rs精确还原G1 hash e6af8cb6…；确认未变更生产部分。没有实际修改源文件。
- cargo test --lib --no-run准备当前测试二进制；逐个--exact独立执行以下六个用例：全部1 passed，合计6 passed/0 failed。没有使用宽过滤或--ignored，没有运行自然harness。
  1. joint_diagnostic_covers_the_padded_closed_intersection
  2. a_late_expiry_after_the_last_realize_refuses_the_witness
  3. a_late_expiry_after_a_design_refusal_marks_incomplete
  4. a_staged_witness_is_cleared_when_the_deadline_passes_before_output
  5. an_in_budget_witness_survives_and_a_refusal_stays_clean
  6. a_mismatched_shared_bundle_node_is_refused_by_production

## 分项判断

### T1 通过

已明确2560为下界而非唯一值；撤回B轴穷尽；保留B=3000完整路束未测；披露原baseline realization2超授权1和重复5态。旧错误回执保留、新勘误给出正确解释，符合证据保留规则。

### T2 通过

失败分支已用两侧生产profile.height_at；闭区间支撑交集由实体几何推导；缺失值计missingValues并标diagnostic_incomplete，不再由负站点as usize静默漏列。合成回归196/27通过，四个x>288遗漏不匹配点有断言。真实自然196/27仍来自上轮检查员probe，本轮不冒充重测。

此helper只是诊断，不负责决定生产是否接受；生产append仍是唯一工程拒绝/接受来源。

### T3 通过，证据范围如实限定

检查到BundleOutcome真实接入harness：循环入口检查、设计/测量完成检查、成功结果暂存前检查、stage_witness检查、最终finish门。晚到期会清空witness并标incomplete。

需要精确表述：不是每个生产叶子返回后都立刻单独check。某些拒绝分支continue后由下一循环入口或最终finish捕获；after-realize检查位于commit_decision通过的分支，失败由后续入口/finish捕获。对本轮要修复的“晚截止却发布成功/干净负结果”已有覆盖，不以措辞不完全精准再阻塞收口。

四个小测试测试的是时钟结果封装，既有共享节点守卫测试生产拒绝；它们不等于自然harness带注入时钟端到端实测。本轮未重跑自然路径是预算要求，不宣称新的自然证书或硬实时抢占保证。

### T4 仅采纳方向，不接受隐含范围扩张

文档先承诺A/B/C坐标不变，随后又提出改变C位置/街段长度。这两个范围不一致。报告中的[C高程、C坐标]必须分开命名；下一任务以本检查员任务书为准。不得把(2560,7692)当成已证可行区间。

## 下一步

执行 ds-v41f-b01-g4-fixed-v5-joint-level-task.zh-CN.md：固定A/B/C/width/junction等，最多6个高程状态；先工程两段joint检查；仅有通过者才测量、必要时baseline realize1次和候选realize最多2次。自然实验最多一次调用、全轮180s；不为补诊断重跑。无bake/大图/浏览器，无公开接口或生产策略变更。

## 检查员边界

本轮自然design/append/infer/realize均0；无bake、自然harness、128、clock80、carve、CLI世界、release/browser/max/workspace；无子agent、Git写操作、生产或测试源码修改、F盘写入。仅新增审查/任务书/检查员证据及追加status。

证据：bevy/work/collaboration/inspector-b01-g3-r1-20260918/ 的6份exact测试日志、integrity-review.json、review-decision.json及检查日志。
