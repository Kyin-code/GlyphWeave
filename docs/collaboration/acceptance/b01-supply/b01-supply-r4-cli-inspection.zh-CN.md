# B01-SUPPLY-R4-CLI 独立验收

## 结论

**accepted-subtask：CLI独立来源审核接线完成，本轮R1～R4供应基础修订队列关闭。**

已有A事务、B滚动承诺、C1共享生产审核、C2来源声明、C3旧parcel保护不再重开。这个结论不等于B里程碑通过，也不等于24个新需求已实现；B整体仍partial，C里程碑不推进。

主检查员没有改业务代码、启动agent、Git写操作或改F盘。本轮仅一次自给CLI测试内bake，复用该世界做全部正负例；无正式世界生成、80点时钟扫描、冻结128扫描、carve诊断、release/浏览器/最大图。

## 1. 独立复验

HEAD：`fb36dc4f9fdd38e6aa9c5a89676a65b96e5001b1`。

证据目录：`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-supply-r4-cli/`。

- receipt完整SHA256：`3ad745e81598544c6bdda93b72c3bc8b717a19c3a1837425befda3bf9c04e504`，与用户前缀一致。
- 18项path/log/hash/bytes绑定匹配（其中changedFiles包含标注unchanged的文件，不能全部算本轮修改）。
- W1登记7个文件hash未变，L1/R1/R2/R3四份旧receipt仍符合原签收hash。
- auto_roads.rs、frontage_supply.rs、landcover.rs与R3绑定一致，R6模块hash也保持。

独立命令：

1. `cargo test --manifest-path bevy/Cargo.toml -p glyphweave-cli --test frontage_supply_source -- --test-threads=1 --nocapture` → **1 passed，52.74s**。
2. `cargo test --manifest-path bevy/Cargo.toml -p glyphweave-core the_legacy_undeclared_entry_and_unknown_sources_are_refused -- --test-threads=1` → **1 passed，1.04s**。

未重复residual_source回归以免额外bake。日志cli-roundtrip.log/source-refusal.log，实际子进程JSON证据提取为cli-roundtrip-evidence.json。

## 2. CLI往返实际结果

同一次TempDir bake，world.seed=12648430、seedOffset=48879、有效seed=12599553，v3候选状态预算1。

| 步骤 | 独立结果 |
|---|---|
| validate-world | exit0 |
| quality-report | exit0，roadNetworkAudit=pass，report=warn |
| world-quality-gate | exit1，报告七项内容缺额，不是来源上下文错误 |
| 两份plan同步篡改naturalHeightfieldHash | exit1，road plan differs from natural raster reconstruction |
| 还原plan | quality-report恢复exit0 |
| 改错world.seed | exit1，scene-0 natural snapshot seed mismatch |

gate七项：frontage未完成、自动道路candidate_state_budget_exhausted、joint住宅最低需求不足、purpose面积、landcover、land-use、road-first development未完成。既有scale_audit调用仍位于validate之后、quality-report之前，没有被移除。

没有把负例伪装成副本不一致拒绝：两份plan均改同值，实际拒绝来自来源重规划。错seed在自然快照来源层被拒，也是有效源绑定保护；不宣称它已经到达道路plan比较层。

## 3. 接线审查

- `bake_world`规划和正式审核两处共用declared_natural_source；effective seed及精度不再各写一套。
- CLI quality_report_command现在调用verify_with_manifest。
- 门面从原manifest/scene推导来源，再调用verify_impl，trusted_record=false；仍做重规划、记录对照、final支撑检查，没有绕过来源重建。
- 旧verify_with_style在v3缺声明时继续拒绝，未知terrain source继续fail-closed。
- 公开门面的调用前提仍是scene、style、zones等来自已验证原manifest/其可信重建。CLI按此条件使用；不要把此多参数函数宣传为能自动净化任意互相矛盾的外部输入。

当前测试真正复验了命令行子进程，而不是cargo check或只调helper。剩余工作应回到生成供给能力，不再循环审查同一调用点。

## 4. 非阻塞记录纠正（不重开代码队列）

### bake数量

执行者说“整轮bake1次”，但同时交付了residual_source回归1 passed日志。该测试在`bevy/crates/cli/tests/residual_source.rs::residual_policy_is_bound_to_the_frozen_manifest`中明确调用一次bake_world。因此：

- frontage_supply_source往返测试本身：1次bake，属实；
- residual_source回归：额外1次bake；
- 执行者整轮已披露命令合计：**至少2次测试bake**，并非1次。

不改旧receipt覆盖事实；下一报告/新回执追加说明。这是成本/预算披露问题，不影响已复验的来源拒绝正确性。检查员本轮只运行前者，实际1次bake。

### 警告归因

首次quality-report在scale-audit之前运行，所以出现scale-audit.json missing；随后gate会执行scale-audit并写报告。它是报告时序提示，不是地形/容量缺额，也不是一状态预算的结果。

一状态预算确实可能造成搜索不足，但不能断言所有住宅/土地实现缺额都仅由这一预算引起。现有landcover/development未实现同样可能是独立业务缺口。无对照证据，不做单因归因。

### 测试断言强度

独立日志证明roadAudit=pass、gate内容fail。当前测试部分正向只断言audit对象存在、gate没有旧错误并能读报告；后续顺手可对期望状态/退出码加明确断言，无需为此再bake或派一轮修订。

## 5. 项目状态与下一步

- 供应基础路径可以生成、独立复核并拒绝篡改；这一基础修订队列关闭。
- 新CLI fixture不等于W1冻结输入，也不是16住宅+8店铺完成证书。
- 原自然困难案例未解；当前暂无24/24新需求真实见证。
- 下一步先争取一个可核验的新增地块，不要把单位测试/审核数量增长当业务完成度。

下一任务：`G:/difyllmwiki/GlyphWeave/docs/collaboration/ds-v41f-b01-g1-feasible-increment-task.zh-CN.md`，限定少量有依据的候选和工程高程选择试验，保留原35路/21义务、原始自然与需求；不改全局carve，不再盲跑128。
