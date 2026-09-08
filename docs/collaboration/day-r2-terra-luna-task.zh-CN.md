# 白班 A-D1-R2：Terra实现 / Luna独立回归

日期：2026-09-08 11:35 Asia/Shanghai。用户明确选择GPT-5.6 Terra与GPT-5.6 Luna。总控负责接口冻结、协调、验收；不代写业务实现。本轮不开展B/C，不并行跑大图。

## 基线、工作区与写入隔离

唯一仓库 G:/difyllmwiki/GlyphWeave；禁止修改F:/modenmapbox。
HEAD=6020fbe2713e8c79c1b2934b244402dc93e1df14。保留全部已有脏文件。外部v4f的R2任务暂停，由用户停止外部执行；任何非预期白名单变更先报告，不覆盖。

| 任务 | 模型 | 唯一代码写入范围 | 文档和证据写入范围 |
|---|---|---|---|
| A-D1-R2-T | gpt-5.6-terra | scripts/collaboration/check-evidence.mjs | docs/collaboration/a-d1-r2-terra-result.zh-CN.md；bevy/work/collaboration/terra-r2/ |
| A-D1-R2-L | gpt-5.6-luna | scripts/collaboration/check-evidence.test.mjs | docs/collaboration/a-d1-r2-luna-result.zh-CN.md；bevy/work/collaboration/luna-r2/ |
| 总控 | 当前主Agent | 不改worker实现 | 协调/评审文档及bevy/work/collaboration/review/ |

以上相对路径均以唯一仓库为根。禁止修改对方文件、原a-d1-result、Rust文件、原始世界证据或review目录。不新建共享模块。需要改接口先报告总控，禁止私自互相适配而削弱标准。Luna写实际测试补丁，不只是建议；Terra不调整测试期望。

## 共同必读

AGENTS.md；docs/collaboration/acceptance-contract.zh-CN.md；docs/collaboration/a-d1-review-02.zh-CN.md；当前两个脚本。
参考真实小过程文件：bevy/work/r49-maximum-generation-measurement/process.json、bevy/work/r49-maximum-gate-measurement/process.json，以及examples/maximum-local-height-frontage.manifest.json。
R1冻结副本在bevy/work/collaboration/review/day-r2-baseline/，只读；Luna可将两个副本复制到自己的work中跑红测，不能改review原件。

## 冻结的接口与最小合同

保持现有导出函数、CLI参数、结果字段、退出码语义。runReceipt仍返回{result,exitCode}；status与worldQualityStatus分离。缺证据必须非零；工具外认证主Agent收据和源码→二进制来源，禁止编造信任。

为减少两人并行的接口漂移，本轮正式完整验收采用processReceipt路线：generation和gateRun各提供独立的{path,sha256}过程JSON，log仅补充证据。旧log-only收据返回incomplete，明确记录迁移，不新增臆造日志解析器或签名框架。
过程JSON采用现有glyphweave.process-benchmark/version=1，关键字段operation、status、exitCode、stopReason、executable、executableSha256、manifest、manifestSha256、output、scenes。外层generation/gateRun保持operation、status、exitCode、stillRunning和processReceipt，不新增必填外层绑定字段。

- generation.operation=generate-world；gateRun.operation=world-quality-gate。外层operation与过程operation一致。command是展示字段，不以子串匹配判断实际调用；真正身份由结构化operation确定。
- generation正常完成：baked、exitCode=0、stopReason=null。gate完整通过：success、exitCode=0、stopReason=null；gate正常判失败：failed、exitCode=1、stopReason=null。允许现有明确成功同义词，但两层语义必须一致，未知拒绝。timeout/budget/abort/crash/running等不能用失败授权抵消；外层和过程任一方中断即拒绝。
- 过程runtime和manifest的规范路径与hash分别和收据对应绑定一致；output与被检查world是真实同一目录。hash大小写可规范化，不能只核对过程文件自身hash。
- manifest真实输入format=glyphweave-world/version=1，world为对象，scenes非空；sceneId唯一，widthM/depthM有限且正，originX/originZ有限，seedOffset按当前实际schema。两份过程scenes与manifest逐sceneId核对这些字段，顺序不是身份；world场景数量/可用scene身份与之相符。可选默认值遵循实际Rust schema，不臆造必填导致合法输入全拒。输入与发布manifest不要求字节相同。
- receipt.world.path是真实world根，不接受子目录；保留等价路径合法正向。
- gate.failures在pass/fail都必须是字符串数组，pass必须空，fail有明确失败且逐条匹配授权基线。
- 继续保持authority限制、报告hash/revision、frontage计数及几何unverified、读大小预算、scene只stat不读、输出wx拒覆盖等R1要求。

## 各自交付

Terra：定点完成review-02的F1–F5；不整文件重写、不增加依赖。自行在terra-r2 work中小fixture测试，不改Luna测试。更新脚本顶部合同注释，给出变更hash和兼容性说明。实现稳定后通知总控。
Luna：按冻结合同升级原正向fixture为真实schema形状，独立扩展回归。从合法control每次只变一个条件，并更新对应hash，避免因无关缺字段产生假阴性。不靠宽泛“非零”代替检查具体拒绝原因。对旧R1先留红测证据，再针对Terra候选跑绿测；未就绪则交测试补丁和待跑清单，不忙轮询。

最低独立用例：合法完整pass；合法授权quality fail但证据pass；demo不pass；log-only incomplete；生成/gate错误operation；过程的runtime/manifest/output/exitCode逐项冲突；两层timeout/budget/stopReason矛盾；manifest非JSON/未知版本/scene尺寸冲突；world子目录与同路径别名；pass时failures缺失/null/object/scalar/非字符串；保留frontage/只读/字节预算/CLI回归。必须有真实shape完整control，不能全部拒绝后声称通过。

## 资源、验收和停止条件

只用Node内置库和已有命令。允许轻量Node测试；禁止Rust编译/全workspace/最大图/preview/new server、新依赖、新模型。总控在两人交付后固定候选hash再串行复验，运行node --test、两个node --check、node scripts/generate-tree.mjs --check、git diff --check及独立单变量反例。真实r49继续integration_pending，绝不改demo为假授权。

本轮只授予一次有界修订。遇合同冲突先反馈；同类核心缺陷仍失败则最小复现和阻塞，不无限循环。完成仅ready_for_review，不宣布A完成、不开始D2。

## 禁止Git写操作

双方禁止git add/commit/push/merge/rebase/reset/clean/checkout/switch/stash，禁止改分支/标签/config/hooks/index。只读Git允许。不清理他人文件。主Agent本轮也不提交未验收变更。

结果包含任务ID、文件路径/hash、命令/退出码/日志、未验证事项、无后台进程及未暂存/提交/推送声明。所有路径在交付中用绝对路径。
