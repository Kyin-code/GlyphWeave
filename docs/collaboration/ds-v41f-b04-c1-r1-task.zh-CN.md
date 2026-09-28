# V4.1 B04-C1-R1 任务书：修正编译语义，定位 CLI 道路审核失败

## 1. 目标与约束

只在 `G:/difyllmwiki/GlyphWeave` 工作。先读同目录 `b04-c1-inspection-20260916.zh-CN.md` 和原 `ds-v41f-b04-c1-task.zh-CN.md`，不重读全部历史。保留 dirty 工作区与 C1 旧证据，记录 HEAD/输入源码 hash。

你负责实现，主检查员验收。不启动任何 agent；不 git add/commit/push/reset/clean/checkout/switch/stash；不修改 F 盘、A 冻结输入/世界/runtime/证据。B 不签收、C 不推进。

本轮不是追求自然案例 24/24，也不是新建审核框架。保留已有 effective 编译/原 manifest/独立重编译接线。R6 `road_parcel_feasibility.rs` 只读，不重开加固任务。

## 2. 顺序任务与验收

### T1：严格解析，不能吞未知输入

复现检查员 `compiler-probe.json` 的 frontageProgram.unexpectedField，再修复。补默认前检查容器和键，或保留未知键给严格消费者；不能把合法的未填默认字段误判为缺必填。

验收：未知 program 顶层键、未知 request 键、错误类型/null/版本均按合同拒绝；合法省略默认可编译；显式 count/id/尺寸/setback/预算/完整程序保持不变。检查其他重建 effective 子程序的相同风险，只修真实同类问题，不扩建通用 schema 平台。

### T2：拆清完整地块、建筑尺寸和院落

修复 parcelWidth/Depth 直接赋给 building width/depth 的语义错误。优先沿用现有完整地块字段语义，根据确定性选定的 parcel + insets 推导缺省 footprint；推导必须 checked arithmetic、正偶数、consumer/asset 合同验证。若当前字段组合无法表达，明确错误/unsupported，禁止 clamp/缩小显式尺寸/放宽 asset 范围。

显式建筑尺寸优先于 profile 的启发式范围；若因此偏离默认范围，报告 original-input/偏离原因，不偷偷调整。部分显式、部分缺省也需来源清楚。

验收：六 profile 列出可用/拒绝/unsupported 矩阵；默认 footprint 与真实 parcel_geometry.boundary 一致；完整缺省地块不越出声明范围；覆盖四朝向、显式覆盖、部分缺省、院落扣减后无效等测试。不能仅用字符串或编译结果尺寸表证明真实地块正确。

### T3：比例语义

停止从 publicSpaceRatio 直接派生 ruralPastureRatio。显式牧草地程序不变；缺少独立牧草地依据时不发明数值，保留已定义 legacy 默认并标记该 profile 映射 unsupported，或使用确有独立来源且语义清晰的现有字段。无需联网引入新研究或扩大 profile 资料任务。

验收：公共空间 profile 改变不会无依据改变牧草地比例；明确来源、分母/作用域；原始 manifest 保留，重编译审核正负例通过。更改有效语义时更新 effective revision 并保留旧产物，不覆盖旧版本报告。

### T4：修复可移植测试和旧断言

- morphology CLI 常规测试必须在没有 ds-b04-c1 work 目录的条件下有真实正负断言，使用独立 tempdir/自给小 fixture。不要删/移动现有 work 来模拟干净环境；通过路径隔离验证。
- 手工大产物检查可独立为显式 ignored 测试，但不能把普通测试全部变 ignored，也不能缺目录 return-as-pass。
- 仅最小更新 baked_audit 的旧错误文本断言，保留 forged ownership 的实际构造、非零退出与正确拒绝来源。不要修改审核器迎合错误消息。

### T5：现有两世界的道路诊断与最小修复

先只读 `ds-b04-c1/gen-compact-mixed-block`、`gen-village-field`，禁止先生成新图。确认 3 条 roadProfile 及 append_one 接线，纠正报告。

对每个失败 road 定位至少一个最大误差样本：road ID、坐标、实体/计划 profile、是否在路面/支撑/边坡/端点范围、natural/final/设计目标毫米高程、取样来源、相应 carve 覆盖和执行阶段。特别核查独立 audit 的范围及 profile.height_at 回退是否符合生产工程合同。

区分：生成实体/高程不一致、后续阶段覆盖、或审核器范围/目标错用。必须用小确定性测试复现，再在现有生产流程做最小修复。若为审核器错报，证明原合法工程为什么应通过，并保留实际路面/支撑扰动负例；若为生成缺陷，修阶段写入/来源，不提高 fill/误差容差、不跳过 profile 道路。

验收：原失败机制被定向回归覆盖，合法小控制 road audit pass，真实支持区域的未授权扰动拒绝；内容缺额和工程失败分别报告。frontage 仍缺额可以 partial，但必须按真实拒绝计数解释，不能再说 auto-road 缺 roadProfile。不要自行重建道路架构；白名单外确有必要则记录阻断与最小变更提议。

## 3. 验证预算和停止条件

先完成 T1–T4 定向编译/CLI 测试以及 T5 只读诊断/小回归。单项测试 120s，首次必要编译可单独 300s。不跑全 core/workspace，不跑 release/最大图/240 块浏览器，不执行会覆盖 R6 旧 work 的 ignored 测试。

仅修复和小回归完成后，必要时允许最多 **2 次新的 512m 生成**，串行、单次 120s/4096MiB，输出到本轮新目录；对应独立 quality/gate 记录退出码和失败项。优先复用 C1 两份原始 manifest，仅语义修正由新编译器生效；不要换 seed、改自然地形、降低 count 或预算、增加 fill 来获取通过。可以继续缺额，不能混淆通过范围。无需为了证明多次运行而耗尽次数。

若两个同输入世界因新严格语义应拒绝，记录具体输入冲突，不强行改输入冒称同输入成功；真实几何效果由自给可行小集成另证。

## 4. 写入白名单

- `bevy/crates/core/src/settlement/morphology_compile.rs`：语义/严格解析/来源/revision及定向测试。
- `bevy/crates/core/src/settlement/morphology.rs`：仅必要字段语义注释或明确独立参数，不虚构新统计。
- `bevy/crates/cli/tests/morphology_compile.rs`：自给小测试。
- `bevy/crates/cli/tests/baked_audit.rs`：仅已确认的旧断言最小修复。
- 道路根因确证后可最小修改 `bevy/crates/core/src/worldgen/road_network.rs`、`auto_roads.rs`、`road_extensions.rs`、`road_profile.rs` 及就地定向测试；`worldgen.rs` 仅必要阶段接线/版本修正，禁止顺手重构大文件。
- 如生产/审核输入传递确需，`bevy/crates/cli/src/main.rs` 仅保持独立重新编译的兼容接线。
- 新结果 `docs/collaboration/b04-c1-r1-result.zh-CN.md`；原 C1 结果可追加醒目勘误，不重写其历史证据。
- 新证据 `bevy/work/collaboration/ds-b04-c1-r1/`，create-new、不覆盖旧包。

其他源码（尤其 R6 joint、自然算法、资产、依赖/lock、渲染）只读。测试必须新建源码 fixture 时限定新增在 cli/tests 的 morphology 专属 fixture 子目录，并在回执列出。

## 5. 交付要求

逐项 T1–T5 状态、修改路径/hash、命令/exit/log、检查员反例前后结果、六 profile 映射表、真实几何证据、道路误差定位表与根因、小图是否运行/原因/质量缺额。标注旧证据复用而非本轮重跑。

纠正文档：validate-world 不等于 road audit pass；JSON 内容相等不等于原文件逐字节；共享 effective 输入不等于生命周期只调用 compile 一次。

跑 doc-tree:check 和 git diff --check。交付 ready_for_review 或 partial，不自签收。完成停止等检查，不提交/推送、不再派 agent。
