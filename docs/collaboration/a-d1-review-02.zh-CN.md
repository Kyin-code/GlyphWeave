# A-D1-R1 主Agent复验：needs_changes（第二轮）

日期：2026-09-08，Asia/Shanghai。总控仅做验收和协调，不代写worker实现。

## 1. 结论与已完成部分

A-D1-R1 暂不接受，A-D1仍为needs_changes；不是里程碑A完成。
本轮确认已有改进：必需报告hash覆盖、authority分级、format/version/revision检查、frontage计数含义、路径逃逸/输出拒覆盖等已有新增实现和测试。42项自测全部通过，不否定这些进展。

基线HEAD仍为6020fbe2713e8c79c1b2934b244402dc93e1df14。
实测候选hash：
- check-evidence.mjs：d09a27c416ac0394b5d6637ad35a3c21b4a99ac310966af953dc9848e20fff0b
- check-evidence.test.mjs：8ad14d5677660198996ecdb97c7bd148a702cba085e8dc1e57daddd6ae174fb3

## 2. 独立复验方法及结果

目录前缀均为 G:/difyllmwiki/GlyphWeave/。

- 42项复跑日志：bevy/work/collaboration/review/a-d1-r1-existing-tests.log（退出0）。
- 旧11类反例复跑日志：bevy/work/collaboration/review/a-d1-r1-old-adversarial.log。旧基础样例本身已经incomplete，因此不能靠全部非零证明各分支修复。
- 新独立脚本：bevy/work/collaboration/review/a-d1-r1-independent.mjs。
- 新日志：bevy/work/collaboration/review/a-d1-r1-independent.log。
- 本次逐项JSON：bevy/work/collaboration/review/r1-independent-q9Rao5/summary.json 及各case.result.json。

运行命令：node bevy/work/collaboration/review/a-d1-r1-independent.mjs。
脚本使用worker R1的基础fixture构造，先确认候选当前认为它完整，再逐例改一个条件并更新应变的hash，不使用hash不匹配掩盖语义漏洞。所有材料均为合成测试，authority标签不是真实主Agent验收授权。基础manifest本来也是简化/错误格式，这进一步说明正向fixture仍需按实际合同补齐。

| 用例 | checker实际结果 | 正确验收含义 |
|---|---|---|
| control | pass / 0 | 仅证明当前候选基础分支能通过，不证明真实运行 |
| wrong-gate-operation | pass / 0 | scale-audit不能代替world-quality-gate |
| wrong-generation-operation | pass / 0 | preview不能代替generate-world |
| world-subdirectory | pass / 0 | world/scenes不是被检查的world根 |
| manifest-not-json | pass / 0 | 无关纯文本不能作为有效manifest |
| pass-without-failures | pass / 0 | 缺failures不得默认空数组 |
| pass-invalid-failures | pass / 0 | failures对象不得默认空数组 |
| authorized-gate-timeout | pass / 0，quality=fail | 超时不等于完整gate正常判失败 |
| authorized-gate-budget-stopped | pass / 0，quality=fail | 预算中断不能被allowedGateFailures授权抵消 |
| contradictory-process-receipt | pass / 0 | 过程JSON明确不同operation/hash/output/退出码，不能只验文件hash |

9个负例全部误放行。新脚本有断言式总退出码：本次退出1，表示验收预期失败；不是程序崩溃。旧脚本退出0不代表所有验收断言通过。

## 3. 收敛为5个必须修复项（均为P1）

### F1 运行身份及过程文件语义（候选262–344行）

validateRunReceipt只要求operation/command非空，对processReceipt只hash，不解析。必须区分generate-world与完整world-quality-gate；operation与结构化调用身份冲突必须拒绝。不要做脆弱的命令子串匹配。

优先适配当前已有glyphweave.process-benchmark/version=1：operation、status、exitCode、stopReason、executable/executableSha256、manifest/manifestSha256、output、scenes。校验过程文件与外层receipt、当前输入/runtime/world的身份一致。

可只读参考：
- bevy/work/r49-maximum-generation-measurement/process.json
- bevy/work/r49-maximum-gate-measurement/process.json

普通日志hash只证明字节没变，不能独立证明运行身份。若保留log路径，必须同时有明确、可核对的结构化运行声明；只有任意文本日志而没有上述运行绑定则incomplete。现有过程schema没有worldRevision，不能假称已有；报告hash/revision继续单独校验，过程与报告的真实采集来源由总控在工具外确认。不得新增密码学签名系统或声称证明源码编译来源。

### F2 gate失败与执行中断必须分离（候选294–306、855附近）

正常完成的gate质量失败（当前r49为status=failed、exitCode=1、stopReason=null）可以按明确失败基线做证据通过，worldQualityStatus仍fail。
超时、预算停止、kill、abort、panic/crash、stillRunning不得当作上述正常失败；即使旧gate文件齐全且失败列表获授权也必须整体非零。读取并核对外层及过程文件状态，不能仅正则把所有失败词归一。退出码/状态/stopReason互相矛盾也拒绝。缺关键执行信息为incomplete，不推定完成。

### F3 manifest必须是有效输入（候选778–789行）

受限读取并解析manifest，校验实际支持格式/版本与必要字段；真实输入是glyphweave-world/version=1，而非当前fixture的glyphweave.manifest。
校验scene身份、数量、尺寸与两份过程scenes一致，world身份按可用紧凑字段核对；缺必要证据则incomplete。保留输入manifest与规范化发布manifest的区别，不要求二者字节相同。不读取443MB scene.json来补此项。

### F4 world根身份不是包含关系（候选829–830行）

receipt.world.path与检查world必须是规范化后的同一真实目录，不能仅判断子目录在world里面。路径逃逸检查继续使用containment，身份校验用exact identity；保留有效同路径拼写/符号链接别名的明确策略，不强制字符串原样相等。

### F5 gate.failures形状独立于pass/fail（候选548–605行）

在任何gate状态下都必须校验failures为字符串数组；缺失、null、object、scalar、含非字符串项全部拒绝。pass只允许空数组；fail必须有明确失败项并与授权基线比较。禁止先filter/default后检验导致错误字段变成空列表。

## 4. 下一轮验收冻结

只做上述5项及相应测试/文档，不扩成通用schema框架或大重写。
必须同时存在：真实schema形状的完整合成正向；完整正常质量失败且批准基线的正向；逐项负向。从合法正向每次只变一个条件，不能让全部样例都因新字段缺失被拒后宣称已修好。

本轮新独立脚本在R2收紧合同后可能连control也拒绝：这是fixture迁移信号而非R2已通过。worker应在自己的测试中提供补齐后的对应正负样例，总控保留本轮脚本原件并独立构造R2复验。

真实r49没有当前受信源码/运行全链回执，继续integration_pending；r48继续拒绝。无需生成大图、跑完整Rust、启动服务、调用其他模型。工具只能验证材料内部一致性，不能自行认证authority或证明运行真实发生。

下一次修订若同类核心问题仍失败，先交最小复现与具体阻塞供总控裁剪方案，不自行第三轮大改/无限重跑。未接受前不开始A-D2、不提交。
