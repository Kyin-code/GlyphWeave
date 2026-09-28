# V4.1 B01-G3-R1：纠正V5实验结论与诊断，收口测试harness

## 1. 先读检查员结论

读 b01-g3-v5-bundle-inspection.zh-CN.md 及 inspector-b01-g3-20260918/level-probe.json。

接受五个登记状态的negative observations；撤回“B唯一2560/整个B轴耗尽”。检查员已用公开production design/verify/append证明同几何B=3000的接入合法，不需要为认错再跑一次自然实验。

本轮是小修订，不是新搜索。无自然五态harness、无natural baseline/candidate realize、无bake、无128/clock80/carve/CLI世界/gate/release/browser/max/workspace；不改生产算法/合同，不Git写操作、不agent、不F/A。

## 2. T1：文档、回执和搜索口径一起纠正

- 原G3文档只追加勘误，旧receipt/表/两次run文件不改；新ds-b01-g3-r1-closeout回执绑定新结果及检查员来源。
- 写清：2560是生产infer_end最小平台和可达下界；3000接入通过；五个登记点耗尽不代表整数B域耗尽。没有证据说完整V5必不可能，也没有真实increment。
- 对B=3000只引用检查员已证接入，不说街段或8地块已通过。
- 累计实际：unique5/attempt10/重复5/candidate realization0/baseline realization2，baseline授权1故超额；总约15.98s未超过180s。以后所有重启计全轮，不可把全轮预算改每进程。
- 保留此前未观测项not_exercised，不能用“无增量”代替测量0地块，因为根本没进入测量。

## 3. T2：修正完整交集诊断，不变更生产拒绝规则

在auto_roads.rs的测试区，将失败分支street profile原始数组get改成production road_extensions::profile(design).height_at，与approach同样通过height_at取值。支撑交集按两实体生产几何导出，不再负站点as usize。

闭区间每格必须获得两值，任何缺失明确报diagnostic_incomplete，不能静默不计。保留生产阈值，不改plateau/自然/fill/旧road。

加小的合成profile诊断测试：含端外padding的[279,171,292,184]必须遍历196格，覆盖x>288四列；模拟原S01生产profile末端/开头序列，期望27不匹配。fixture标合成回归，真实196/27来自已交付检查员production probe，不冒充本轮自然重测。

不修改旧140/23证据，只在新结果中纠正并链接来源。

## 4. T3：补测试harness晚截止，不重开生产时钟项目

允许仅测试区抽取小的deadline/结果封装，使用可注入时钟。截止必须在baseline后、设计/测量的长阶段返回后、realize返回后、best写入前、最终输出前一致检查。

任一晚截止→incomplete=true、bestLegalCandidate=null、丢弃整个staged束，主baseline保持；不能先把best写入再返回incomplete=false。成功输出必须同时满足真信用/commit_decision/未截止。

新增少量StepClock测试，不sleep、不自然realize，不运行旧80点扫描：
1. 最后一次realize前未截止、返回后截止→拒绝输出见证。
2. 最后一态设计拒绝时刚超过期限→incomplete=true。
3. 完整成功信息暂存后、输出前截止→见证清空且baseline不变。
4. 未截止的成功/普通拒绝保持各自状态；零增量不提交。

复用现有已验收守卫，不改frontage_supply/road_alignment/road_network/worldgen/CLI生产代码。代码范围优先auto_roads.rs现有cfg(test)，不得开公共接口。

## 5. T4：只提出下一实验计划，不执行

用一页说明如何在未变的A/B/C坐标、width12/junction8等合同下选择下一批**不同**B/C状态；区分minimum returned by infer_end与其它合法值，不重复原五个状态，不根据几个点推导完整可行区间。

若建议修改junctionLength，必须作为另一个新候选参数方案明确标注，并与“现合同内剩余高程自由度”分开。不要宣称必须修改合同，不能自动转设计B/放开carve。此方案只供下一次审批，不在本轮实施/执行。

## 6. 验证、预算和交付

- 实际运行仅新诊断/截止小测试与必要编译；沿用/引用自然证据，natural design/append/infer/realize新增调用全0。
- 不调用v5_bundle_tries_for_one_real_parcel_increment；编译不得以宽过滤误跑别的自然用例。
- 新 b01-g3-r1-closeout-result.zh-CN.md、ds-b01-g3-r1-closeout/receipt.json、测试日志、源码hash、旧证据未变检查。
- 增加预算表total/unique/actual/thisRound/history，不将检查员探针计入worker本轮运行，也不把历史额外baseline消去。
- doc-tree:check/git diff --check；检查冻结输入与旧回执不变。
- 完成停止等复验；B partial、C不推进，不自行签收，不开启下一自然运行。
