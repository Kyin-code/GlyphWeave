# A-D1 主Agent验收：needs_changes（第一轮）

日期：2026-09-08，Asia/Shanghai。用户确认夜间外部DeepSeek v4f主力，白天Terra；主Agent只评审/验收，不代写工具实现。本轮不提交未验收候选、不开展A-D2或新功能。

## 1. 已复验事实

- 当前HEAD仍为 `6020fbe2713e8c79c1b2934b244402dc93e1df14`。
- `node --test scripts/collaboration/check-evidence.test.mjs`：41/41，退出0。
- `node --check scripts/collaboration/check-evidence.mjs`：退出0。
- 工具将evidence status与worldQualityStatus分开、有JSON体积检查和流式hash，方向正确。
- 但现有正向fixture使用虚构commit/hash、无manifest、无可信完整gate运行记录仍整体pass，覆盖标准不足。worker声明限制是诚实的，但不能以声明替代本任务“缺证据不通过”的要求。

## 2. 独立反例（已实际运行，不只是代码推测）

总控只在自己的review目录新建合成fixture与检验脚本，未改worker实现或原world证据：

- `G:\difyllmwiki\GlyphWeave\bevy\work\collaboration\review\a-d1-adversarial.mjs`
- `G:\difyllmwiki\GlyphWeave\bevy\work\collaboration\review\a-d1-adversarial.log`
- `G:\difyllmwiki\GlyphWeave\bevy\work\collaboration\review\a-d1-adversarial-1788801484443\summary.json`
- 原41项测试复验日志 `G:\difyllmwiki\GlyphWeave\bevy\work\collaboration\review\a-d1-existing-tests.log`

运行：`node bevy/work/collaboration/review/a-d1-adversarial.mjs`。每次生成新测试目录，旧失败原件不覆盖。

| 反例 | 实际结果 | 应有结果 |
|---|---|---|
| authority改为demo | pass / 0 | incomplete / 非0，不能签署正式证据验收 |
| sourceSnapshot={} | pass / 0 | incomplete / 非0 |
| manifest指向不存在文件、fake hash | pass / 0 | fail或incomplete / 非0 |
| reports只hash无关runtime，不覆盖三个必需报告 | pass / 0 | incomplete / 非0 |
| world.scenes=[]，仍声称sceneCount=1 | pass / 0 | fail / 非0 |
| world/gen/gate/receipt version=999 | pass / 0 | unsupported / 非0 |
| gate.status=banana | pass / 0，quality=unknown | incomplete / 非0 |
| gate.status=pass但有failure且在允许列表 | pass / 0，quality=pass | 矛盾必须fail，不能授权抵消 |
| 删除format及gen/gate的revision | pass / 0 | incomplete或fail / 非0 |
| scene路径逃到world目录之外 | pass / 0 | fail / 非0 |
| receipt生成状态banana，gen报告无status | pass / 0 | incomplete / 非0 |

以上11个缺失/无效/矛盾条件均误放行。fixture本身不是可信收据，不能据此宣称主Agent已批准任何真实world。

## 3. 阻塞接受的问题（P1，必须本轮修正）

### R1：来源与运行证据没有真正绑定

- manifest在注释schema中声明，但实现不读取/校验。
- sourceSnapshot仅检查null，{}的undefined会被放行；“禁止Git操作”不妨碍Node只读hash，且任务从未禁止只读Git。
- runtime存在且hash正确不证明本次命令使用了它；完整gate文件存在不等于完整gate实际运行。
- reports可只有任意一个文件，不能保证真正读取的world/gen/gate被hash约束。

修正：按统一验收合同支持文件级sourceSnapshot.files[{path,sha256}]、manifest、runtime以及生成/完整gate两个独立运行回执（command/operation、exitCode、status、log或过程JSON与hash、输入/输出/runtime身份）。逐项验证，只覆盖已声明证据范围，不伪造源码→二进制编译证明。无法从现存旧产物获得可信源码快照，真实r49检查就保持integration_pending。原manifest与published manifest可能规范化不同，按过程回执绑定各自身份，不无脑要求两者字节相同。

### R2：授权不是一个可自行填写的pass开关

authority为demo/worker-draft/none/unknown必须整体incomplete，不因精确匹配allowedFailures就正式pass。authority字符串本身不构成密码学身份验证；文档说明receipt来源由主Agent/用户在工具外确认，工具仅做严格完整性验证。不要把demo改个标签来通过真实验收。

### R3：必需报告的形状、版本和矛盾状态

formatCheck目前只比较存在的format，根本没验证version。缺失的必需字段、非object根、unknown status、空scene列表和声明数量差异必须拒绝；每份报告必须独立有适用的revision，不能剩下一份就说“各报告一致”。实际版本以当前仓库产物为准，不能把文件名/格式字符串猜错后再改产物适配工具。

gate pass + failures非空、fail +缺失或非法failures内容都不能通过；失败列表必须是字符串数组，不能悄悄filter掉非字符串。生成running/timeout/非0/未知状态、完整gate未运行不完整。schema不支持的版本明确报告unsupported，不能当pass。

### R4：frontage字段与项目真实合同不一致

当前project的审计字段是 quality-report.scenes[i].frontageAudit 中 `requested/planned/realizedWithAccess/unfilled`，工具并不读取realizedWithAccess，planned也未同audit比较。

规划缺额与实现缺额分开：requested = planned + unfilled（本项目的unfilled为规划缺额）；0 <= realizedWithAccess <= planned；planned-realizedWithAccess为实现/入口缺额，不能用realized+unfilled=requested错误覆盖。总控允许的整体未完成范围必须明确，不凭计数认定几何正确。按sceneId定位，不能固定读第一个场景；所用紧凑审计须与当前world revision/报告hash绑定。无几何回执维持unverified，不能暗示已独立几何通过。

## 4. 安全/鲁棒性（P2，同一修订顺带补齐）

- scene路径必须在world真实目录内，拒绝绝对路径逃逸、..逃逸和可检测的symlink/junction逃逸；禁止依赖仅转小写字符串比较实现跨平台路径身份。
- out必须在world只读输入树之外，且不得等于receipt/runtime/任一输入；使用exclusive create（wx）解决先stat后写入的竞争覆盖。当前writeFile普通写入不是真正原子“不覆盖”。
- hash/read检查普通文件并捕获目录、权限及读错误，返回结构化结果而不是未捕获崩溃。
- 字节预算为有限正整数，公开runReceipt缺省时仍使用安全默认；stat体积限制与真实读取边界说明清楚，不扩展到无预算大JSON。

## 5. 此轮交付/验收

保留原有合法功能，修正错误的正向fixture，不以新增expected-pass来绕过反例。先加每类失败测试再改实现。更新完整schema与a-d1-result，指出旧41测试的不足，真实demo输出始终不是正式验收。

新增测试应覆盖R1-R4和P2；合成完整可信格式样例可测pass逻辑，但明确它是合成fixture、不等于真实运行证据。至少一次r49紧凑检查仍可能integration_pending；不得为了过关修改receipt为假主Agent授权。

只编辑原A-D1白名单；不安装依赖、不改Rust/HTML/gate、不跑大图、不提交。主Agent下一轮只复验新测试、上述反例和真实紧凑结果。未经接受不开始A-D2。
