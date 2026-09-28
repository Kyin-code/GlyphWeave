# 可直接转交DS v4.1f：A-FINAL-R2 修订与有界补采

日期：2026-09-08。你是实现/自测执行者，主Agent是独立检查员。上一轮R1的F1/F2和复访分支修复已复验，不要重写这些已通过部分。当前不是里程碑A完成。

## 0. 必读与职责

读取：
- G:/difyllmwiki/GlyphWeave/docs/collaboration/a-final-inspection-r1.zh-CN.md
- G:/difyllmwiki/GlyphWeave/docs/collaboration/acceptance-contract.zh-CN.md
- G:/difyllmwiki/GlyphWeave/docs/collaboration/a-final-signoff-r1.zh-CN.md
- G:/difyllmwiki/GlyphWeave/AGENTS.md

只操作G:/difyllmwiki/GlyphWeave，不改F:/modenmapbox。禁止git add/commit/push/merge/rebase/reset/clean/checkout/switch/stash、Git配置/索引/历史改动；不启动其他agent/模型。保留已有dirty和旧证据。交ready_for_review，不自行写milestone-A-accepted。

检查员反例仅供只读：
G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-r1-20260908/actual-collector-counterexamples.json
G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-r1-20260908/browser-checker-adversarial.json
G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-r1-20260908/closure-parity-world-conflict.json

## 1. 写入白名单与冻结对象

代码只允许：
- adapters/html/app.js（观测和ID到实际对象/实例关联，不改视觉规则）
- 如确有需要，adapters/html/render/vegetation.js 仅给真实创建的实例补ID/index元数据；不得改几何/分布/密度/筛选规则
- scripts/collaboration/check-preview-world.mjs及.test.mjs
- scripts/collaboration/check-final-acceptance.mjs及.test.mjs
- 可新增scripts/collaboration/preview-instance-observation.test.mjs用于实际Three对象测试
- 新结果docs/collaboration/a-final-signoff-r2.zh-CN.md
- 新证据、测量脚本副本、输入控制样例均在bevy/work/collaboration/ds-final-r2/（所有输出必须新路径）

只读冻结：Rust生产、Cargo/package/lock、输入manifest、A-D1 checker、R1独立audit/消费者实现、旧world/旧runtime/旧parity/旧观测/旧source manifests。禁止改worldgen/settlement和生成语义；禁止新依赖；不跑完整workspace或重新编译release。本轮既有audit exe足够处理parity；如确需Rust修改，停止并给最小理由，不能顺带扩大。

原生成exe：G:/difyllmwiki/GlyphWeave/bevy/target-r-final/release/glyphweave.exe
SHA256：99374af17e471e46e5c09185e28775fcd6661e71e025d6a45aa49f2e521b3bc8
审计exe：G:/difyllmwiki/GlyphWeave/bevy/target-r-audit-r1/release/glyphweave.exe
SHA256：e414d4ae08fb7b66a05671d3d32db463f2870df5b149b73ff4ebd9648520c4da
输入：G:/difyllmwiki/GlyphWeave/examples/maximum-local-height-frontage.manifest.json
SHA256：d4fe4d8c72783067f6996621f2836fd5bbc8a154f3fb81c6d2e220fba9154749
原冻结world：G:/difyllmwiki/GlyphWeave/bevy/work/milestone-a-final-r51-20260908-v2-abs2

## 2. 阶段S：两个小修，通过才可进入重运行

### S1：真正的实体/实例存在性

目前__previewStats对任意Group或object的userData.entityInstanceIds直接记账，空Group/count=0 InstancedMesh也被算成实体。修复为：
- 普通实体：逻辑根必须与实际有可绘制几何的子节点关联；空根不能获得存在性信用。
- 树/岩石批处理：把逻辑ID映射到实际创建的batch UUID、instanceIndex和有效count；索引必须在有效范围。树由多个部件批次构成时记录统一逻辑实例及其部件，不将树干/树冠当重复实体；也不能用Set隐藏真正重复逻辑根。
- checker必须验证renderedEntityInstanceSources完整性、ID一一对应与上述结构；缺来源不得pass。一个普通Group可以合法包含多个mesh，不能简单“type必须Mesh”误杀。
- 不做像素/遮挡/相机视锥可见性证明；只证明图中实际几何与有效实例承载。合法LOD按原合同处理，不能临时减少自然实体来过关。

最少反例：空带标签Group；所有batch count=0；某实例index=count；删去某ID的实际部件而元数据仍在；重复逻辑根；来源数组缺失/ID错配；warm/revisit/retry/late中的漏实体和错height。合法多部件树、合法复合建筑、2块正常control必须通过。

### S2：引用闭包的跨文件绑定

当前checkAcceptance接受parity.worldFile.sha256=全0，只要刷新candidate.parity自身bytes/hash。必须核对parity.worldFile规范身份/hash与candidate.world/实际文件一致。补wrong-world-path、wrong-world-hash、同revision但不同world bytes的独立负测。

补已声明字段之间的直接身份对应：parity header/source/auditInput、选中的原始observation、其新checker报告及候选引用；文件hash索引不等于运行来源证明，外部认证边界继续声明。不能只把某个requirement名字对应任意pass文件就说A完整。

将checker从只接受taskId=A-FINAL-R1与measurement_pending改成支持本轮R2和明确测量状态：
- closureStatus只表示引用/内容一致；
- pending情况下readiness必须incomplete或列出具体missing，不等于里程碑通过；
- measured必须有绑定的process原始报告/log（以及本轮sample JSONL），核对operation/exe/hash/input/hash/output、exitCode/stopReason、时间和预算；只改字符串measured不算证据。
- gate正常质量fail/exit1与中断区别，完整质量fail不等于测量失败；总quality status仍fail。
- 不强制worker获得main-agent authority，不编造编译证明或签名系统。最终接受仍由检查员执行。

阶段S验证：运行Node协作测试及新的真实Three对象测试；然后一次2块实际浏览器控制（同时含合法复合实体和自然批实例），不拿单块incomplete代替。记录具体测试/小观察hash。S失败则停止重运行，交最小复现；不得先跑完最大图再来修checker。

## 3. 阶段M：补一次真实进程测量（本任务书显式授权）

前提：阶段S正负测试与2块浏览器控制全部符合预期；两个冻结exe、manifest hash匹配；磁盘空间足够，当前无本任务或其他重cargo/世界生成并发。

本阶段仅允许：一次原exe的完整6×10km生成 + 一次原exe的world-quality-gate，均使用同一个全新measurement-world目录。不是换新世界算法，不提高预算。

推荐路径（存在就选新后缀，不覆盖/删除）：
G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-final-r2/measurement-world/
G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-final-r2/generation-measurement/
G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-final-r2/gate-measurement/

已有scripts/measure-world-bake.ps1可只读复用。为了保留原始采样，可在自己的work目录新增副本：只补逐样本JSONL（UTC/elapsed/PID/privateBytes/workingSet/cpu），保留原500ms采样、原600s/8192MiB判定、原日志和stopReason；副本本身记录hash，不修改原测量脚本和生产exe。

采样及预算语义：
- 生成和gate各自TimeoutSeconds=600、MaxPrivateMiB=8192，绝不能提高。
- 预算约束是原private bytes口径；同时记录OS peak working set，不能把private或working set偷换成同一个RSS名字。
- PID/executable/hash/manifest/hash/绝对output、开始/结束时间、interval、sampleCount、elapsed、sampledPeakPrivateBytes、peakWorkingSetBytes、exitCode、stopReason均明确。
- 连续样本只写自己的新JSONL；null/NaN/无样本不得输出测量pass。测量脚本异常和子进程正常gate exit1区分。
- gate子进程正常完成、exit1、六条原批准内容失败且无新增工程错误，可作为完整预算测量通过；测量包装脚本可能返回1，必须分别记录包装退出码和子进程退出码，不修改成0欺骗。
- Start-Process隐藏窗口，停止只针对自己保存的PID/Process对象；历史8099 preview不动。不按名称kill。

参考命令（使用实际已校验脚本或上述只加采样的副本）：
& 'G:/difyllmwiki/GlyphWeave/scripts/measure-world-bake.ps1' -Executable GENERATION_EXE -Manifest INPUT_MANIFEST -OutputDirectory NEW_MEASUREMENT_WORLD -ReportDirectory NEW_GENERATION_MEASUREMENT -Operation generate-world -TimeoutSeconds 600 -MaxPrivateMiB 8192
& 'G:/difyllmwiki/GlyphWeave/scripts/measure-world-bake.ps1' -Executable GENERATION_EXE -Manifest INPUT_MANIFEST -OutputDirectory SAME_NEW_WORLD -ReportDirectory NEW_GATE_MEASUREMENT -Operation world-quality-gate -TimeoutSeconds 600 -MaxPrivateMiB 8192

若生成或gate超预算/中断/新增不允许的失败，保留全部证据并停止后续完整巡检，返回实际失败，不第二次自动重跑。

## 4. 阶段E：新旧内容等价与审计复用

用流式hash对比measurement-world与原冻结world：所有scene.json、全部自然/最终height、surface/terrain payload、重要entity/road/landcover计划等权威文件。world/generation报告中仅输出绝对路径或测量时间的非语义字段须先明确列例外，不能事后为了相等忽略未知字段。

已知原scene SHA256：a2cc8ae185d6fbfc30b97766b092dcda41e0d89a4389e974dfe216948693d340。
worldRevision：bcc8328c0618ac78756a37bd7eaf013db8098a4f9a73aec19e583822242e52c0。

不只比较worldRevision，它取决于输入。若权威内容不同，先报告差异停止，不能强行复用旧A-05等价或自然保护结论。

若一致，原7分钟linear/indexed测试可以凭相同生态源/高度/实体输入和相关代码未变的证据复用，**本轮不授权再跑linear大测**。

## 5. 阶段B：一次最终240块新观测（本任务书显式授权）

对新的measurement-world，仅更新其preview/下本次观测用HTML/JS派生适配器；旧冻结world不动。不改变source scene/height等。可以复制仓库完整adapter模块集合到新world，避免旧core/new app混用；保留vendor/license。记录实际被服务的每个adapter文件path/hash/bytes及source对应关系，不将原exe的旧嵌入HTML冒称新版。

用冻结审计exe对新world执行一次audit-preview-stream，输出全新parity目录；它只审计source/projection，不重生成自然/人工内容。校验source/header/auditInput身份后，使用本轮修复checker采集新的原始observations。

只启动一个本任务localhost preview server，选择确认空闲端口（不要占历史8099），记录PID、完整命令、serve root与exe/hash；采集结束关闭该PID。测试可使用Playwright headless msedge，不调用额外模型。

必须同一页面240 coverage、3个关键点warm、3轮共9个revisit、networkRetry/lateResponse，所有ready阶段实体/height及实际实例来源核对。冻结预算保持：
- active高细节块1、payload cache1、residentBytes=该descriptor.bytes、pending峰值<=2；
- 几何增长<=8、纹理<=4且不能每轮持续增加；
- post-GC heap相对warm增长<=64MiB，后两轮增长<=8MiB；
- 单步<=120s，不支持heap则unverified不能造0。

额外成本上限：完整巡检本次仅一次，运行40分钟仍未完成则保存部分结果并停止，不能利用240×120s的最大故障截止无限运行。40分钟是本次执行停止条件，不降低既有每步/内存验收标准。失败不要重复全图，只记录最小失败位置和建议针对性补查，等待检查员。

原旧240块观测保留为历史，不编辑补字段、不把旧实例来源移植到新观察。原最终r51身份仍是同内容基线；新补采世界拥有新绝对路径，用新parity/HTTP root完整绑定，不能只改JSON中的地址冒充重新采集。

## 6. 最终提交（不要自签A）

taskId=A-FINAL-R2，status=ready_for_review或具体失败/measurement_pending。
提交：
1. 代码diff白名单、最终source清单及hash；原生成exe、审计exe与当前adapter三者分开。
2. 阶段S的所有控制/反例及2块browser log/observation。
3. 一次generation和一次gate原始sample/process/stdout/stderr、脚本hash、预算判定；六条质量失败原文保留。
4. 新旧权威文件流式hash对照、允许非语义差异清单；A-05旧证据复用理由。
5. 新parity及Node几何消费者检查；一次240块新原始观察、checker结果、实际served adapter清单、server启动/停止记录。
6. 更新候选引用闭包：所有path/bytes/hash、requirement映射、measured/pending状态、observer/checker版本等。旧证据引用可保留但标historical/reused，不能换hash伪称重新运行。
7. A-01~A-10逐项候选状态，建筑精确许可来源及workerd元数据未核实继续列出；不要为了本轮A-09盘点擅自升级依赖或下载新资产。
8. 声明未暂存/提交/推送，无本任务残留进程，未修改原世界、原runtime、F盘，无额外模型。

如阶段S失败，立即交修复候选和最小复现，后续运行未执行明确写not_run；无需为了交付表格齐全制造pass。整个计划不授权B/C开发、全库重构或第三次自动大图重跑。
