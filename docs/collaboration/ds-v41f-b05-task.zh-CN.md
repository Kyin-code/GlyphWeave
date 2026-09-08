# DS v4.1f B-05：生态尾差、组合回归与最终候选集成

本任务受 G:/difyllmwiki/GlyphWeave/docs/collaboration/ds-v41f-b01-b05-runbook.zh-CN.md 预授权。它不是无条件的大图运行许可；必须先满足下列前置条件。最终只能交ready_for_review/partial，不能签收B。

## 1. 进入条件与先停规则

- B-01～B-04接口可编译，新增几何/来源/保护/入口/面积正负例通过，未解决的工程错误为0。
- 至少一个≤1km可行组合的新增必需需求实际实现；若连可行小场景都因同一业务缺陷失败，不跑最大图放大失败。
- 内容容量在最大规模未知、但小场景与安全合同正确时，可运行第一次最大图做最终诊断；若失败保持candidate partial，不自签。
- 先写final-freeze.json：源码hash/WORK_BASE、B manifest、请求数量/用途目标、道路政策预算、world生成与gate预算、预计输出、当前adapter源码。之后只允许一次有明确原因的候选修正，保留旧freeze与旧失败。

## 2. 生态1株尾差的有界修复

A原需求342380株、实现342379。只有在新B实际输入仍存在类似缺额时定位真实原因；不是硬编码为某个A ID补一棵树。

- 在合法候选/拒绝逻辑内做确定性、有界的补采或错误修正，保留索引后的完整几何谓词、植物尺寸/密度/来源ID。
- 不删除缺少的1株、不四舍五入需求、不把水域/门口/保护区放宽、不让已接受植物全量重新编号。
- 若不存在合法位置，保留unfilled和约束证据，不能承诺所有输入都补齐。
- 对新增尝试上限/候选策略明确版本和上界；旧需求和语义不暗改。用小fixture与线性参考验证空间查询/碰撞等价，不再跑7分钟最大linear基准。

允许settlement/ecology.rs、ecology_spatial.rs的相关逻辑及测试，不重写生态系统或把自然源改平。

## 3. 最终代码/读取适配范围

本阶段以测试、修复已复现的集成问题、记录证据为主，不新增大型功能。允许前述阶段白名单内最小修复；可修改：
- bevy/crates/cli/src/main.rs、tests/baked_audit.rs、tests/preview_parity.rs，及新增tests/b_content_integration.rs；
- bevy/crates/core/src/preview_stream_audit.rs、worldgen.rs：仅B真实新增schema的独立重建/完整字段绑定和版本；不能绕开权威源；
- adapters/html/app.js、core/render-stream.js、render/terrain.js、render/road-profile.js、render/buildings.js、render/ecology.js：仅读取/展示B权威几何和来源、兼容新字段，不能浏览器发明新对象或变更视觉风格；
- scripts/check-frontage.mjs、check-maximum-world.mjs、check-render-stream.mjs及对应测试：仅读取B新字段/版本的兼容；
- scripts/collaboration/audit-input.mjs及.test.mjs、check-preview-world.mjs及.test.mjs：只有新输出schema实际需要时最小兼容，原边界/hash/高度/实例/复访负例全部保留；
- docs/collaboration/b05-result.zh-CN.md、b-run-handoff.zh-CN.md和本轮work。

**check-final-acceptance.mjs及其测试、check-evidence.mjs及其测试默认只读。** 复用generic closure时通过新候选内容与本轮源码绑定适配，不再重写验收框架。若其固定A假设无法适用B，保存已有独立Rust/浏览器证据与明确格式缺口，留待主Agent判断，不为得到pass私自删校验。

任何验收兼容改动必须证明：旧正例继续通过、旧负例仍以原原因拒绝、新字段篡改能被拒绝。无法做到则退出大图准入。

## 4. 集成顺序（严格串行）

1. 完成纯函数/模块/CLI小fixture回归和受影响A工程负例；落实生态尾差修复或真实不可行报告。
2. 跑全workspace一次：cargo test --manifest-path bevy/Cargo.toml --workspace -- --test-threads=1。保留ignored基准，不用--ignored启动重性能用例。
3. 集中跑Node collaboration、pnpm lint/typecheck/test、doc-tree、diff；新HTML读取变更先做≤2块实际控制（新parcel/地表/实例来源都有覆盖）。未改的前端不重复跑多轮。
4. 生成最终sourceSnapshot；构建当前B release CLI到新的 G:/difyllmwiki/GlyphWeave/bevy/target-r-b-<runId>/，不覆盖A runtime。单次build最多1800s，最多2个候选构建。可设置本轮CARGO_BUILD_JOBS=2控制编译内存，不改全局Cargo/Git配置。
5. 冻结exe的实际路径/bytes/hash和输入。生成与独立audit可以使用同一份当前B exe，但必须不同CLI操作/进程，不能用生成内的自报代替独立gate；不使用旧A exe验证新B逻辑。
6. 用B最大manifest生成新world，独立gate、audit-preview-stream、frontage/maximum/render-stream检查；不覆盖A世界。
7. 在第一份最终世界上完成来源与几何检查后才启动240块浏览器观察；优先完成有界确定性对照再选定最终world/parity，避免观察后又更换产物。
8. 输出final-receipt、汇总和未解问题；停止自己的preview及临时子进程，保留所有失败产物。

如果某步超过预算/不满足条件，后续依赖项not_run，不用伪造旧结果补齐。

## 5. 最大图输入与测量合同

新增 examples/b05-maximum-settlement.manifest.json，从A maximum-local-height-frontage派生。保留6000×10000、原seed/自然输入/用途目标、144新增frontage、原最低设施/住宅与生态需求；允许新增B能力和B-02已冻结的道路政策，不改A原文件。

B program若与原精确需求冲突，必须拒绝/列冲突；不能只换成更容易的seed和用途比例后称“解决A缺口”。生成语义改动后新worldRevision必须来自当前实际输入/代码，不手填成A revision。

生成最多2次、独立gate最多2次，均各600s/8192MiB private；第二次生成用于确定性或一次有根因的纠正，共享配额。禁止两次失败后继续第三次；保留manifest调用参数导致身份变化等失败原件。

复用 G:/difyllmwiki/GlyphWeave/scripts/measure-world-bake.ps1 或已验证的R2采样实现的只读来源，在本轮work中做必要参数化副本，不改原采样脚本。**保留实际传给CLI的相对manifest参数和工作目录，不能先转绝对路径导致rulesDir/identity变化。** 对新exe、输入、process、samples写实际hash/bytes/elapsed/PID/exit/stopReason；UTC相邻非倒退、summary/process/samples一致。

- generation exit0才是完整生成；gate exit1若为内容缺额，测量可以完整但worldQualityStatus必须fail。
- 对照以权威scene/height/实体/计划/地表为准；只比较revision不能证明确定性。时间/输出路径是明确非语义差异，不在比较失败后临时扩大排除名单。
- 若第二次额度用于修正而未做确定性对照，determinism=not_run并说明原因；不能宣称全部集成通过。

## 6. 浏览器与实际几何观察

新输出正确通过Rust审核后，先冻结served adapter文件清单/原始hash；启动自己的localhost预览端口，不碰8099或用户当前tab。复用当前check-preview-world工具，其本轮运行使用新B parity和新B world，不可重放旧A观察证明新B内容。

全域最多1次40分钟：240 coverage、3 warm、9 revisit、2 retry、2 late，实际实体/height/实例来源均验证，errors=0，沿用既有驻留/缓存/资源增长阈值，不调大阈值。

若工具本身无法控制浏览器，或大观察超时/故障，保存原始observation与局部复现，browserStatus=incomplete。可以给截图辅助判断，但不得拼接多个page/session或把自填report当实际观察。完整巡检失败不第二次整套重跑。

如果新geometry不能由当前HTML展示，先在小控制中修复真实读取；无法修复就不要花40分钟大巡检。用户当前优先不是渲染视觉，但“所见对象来自Rust且不丢失”仍是工程合同。

## 7. 最终业务判定

逐条对比A的六个内容失败：道路目标、purpose-area、landcover、ecology缺额、land-use、road-first development。报告原指标、B指标、关闭/保持/新增、证据路径；不能只说failureCount下降。

B整体ready候选需要：B合同中可行控制与最大图的必需需求达到，工程错误0，比例/覆盖账本真实闭合，完整gate对应状态如实给出，B-04研究/未支持类型没有假完成。任何仍未完成项列入overall partial，而不是继承A豁免后自行签收B。

A两项许可未解析必须原样携带；本轮无新外部复制代码/资产时声明“无新增依赖/外部材料”，不能宣称历史来源已解决。C自然侵蚀/工程后水文仍未因本任务获得验收。

## 8. 最终回执和交接

创建 G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b-run/final-receipt.json 与 docs/collaboration/b-run-handoff.zh-CN.md。

必须包含：
- 实际WORK_BASE、完整增量源码清单/版本、最终输入和runtime；
- B01～B05各状态、几何/面积/数量的实际增量及回归；
- 编译/生成/gate/浏览器实际运行次数、失败次数、elapsed/内存/日志与停止原因；
- 最终world/parity/观察/source/adapter的绑定，不必复制A整套回执；
- 所有未解决缺额、unsupported/source_pending、未执行项、待主Agent决策；
- 改动文件与可供审查的小patch/checkpoint索引、活跃PID（应只剩用户原有进程，本轮句柄应停止）；
- 未Git提交/推送、未动A冻结证据、未启动其他agent的真实声明。

最后停止，等用户集中核对。即便全部自测通过，也只写ready_for_review，不能在status板或README自行标B完成。
