# V4.1 R2/W1 收口任务：显式轴优先 + 道路几何版本绑定

## 1. 只做这两项

仓库仅`G:/difyllmwiki/GlyphWeave`。先读`docs/collaboration/b04-c1-r2-b02-w1-inspection-20260917.zh-CN.md`。保留dirty工作区、当前R2院落正确推导和W1宽路施工，不重做已经通过的T1/T3/T4、道路矩阵、R6或原自然案例。

不启动agent，不git add/commit/push/reset/clean/checkout/switch/stash，不动F盘/A冻结材料。两项分别报告，B不签收、C不推进。

## 2. 修订一：只计算真正缺省的轴

位置`morphology_compile.rs`：当前仍无条件`effective_policy.footprint()?`同时算两轴，再决定是否写入，因此显式尺寸合法也会被默认地块扣院落的无关错误阻断。

实现真正的逐轴lazy推导：
- width存在：保留并让正常严格consumer验证，不推导默认width。
- width缺省：只算完整地块宽减2×生效side，checked、正偶数。
- depth同理；不得用另一个轴的默认失败否决显式轴。
- 全显式width/depth不做默认footprint计算。profile尺寸范围是缺省启发式；用户显式尺寸越出profile范围但符合真实consumer/asset/parcel合同，应保留并notes披露。
- 严格解析insets、unknown字段拒绝、显式非法尺寸拒绝仍保留，不为通过反例放宽任何合同。

必须回归：
1. 前轮inset三例维持预期。
2. low-density/storefront：width12 depth16 height10、insets8/16/26、setback12。完整请求通过，完整parcel44×50、notes披露profile宽度偏离，显式字段不动。
3. 上例仅缺depth：width12保持，depth派生16，通过。
4. 上例仅缺width：真实缺省26−32无效，拒绝。
5. 对称深度反例：显式depth16与front8/rear50/side2，合法显式宽度或缺省宽度应独立处理；缺省depth才拒绝。验证真实consumer/asset边界，不依赖随意预期。
6. 来源表、原始需求、发布effective独立重编译仍成立。必要时递增effective语义revision，旧文档/世界只读。

禁止仅修改测试预期来接受错误拒绝。

## 3. 修订二：真正绑定W1道路几何版本

`GRADED_ROAD_GEOMETRY_REVISION`目前只有定义，没有使用。不能继续声称新增常量就完成版本区分。

### 需要的设计和实现

先写清适用范围与兼容策略（可在本轮结果文档前置）：
- 会使用新16m整平规则的输入必须在权威身份中绑定道路几何revision，生成与独立审核使用一致规则。
- 必须覆盖**无morphologyProfile的显式16m**，以及可能生成16m的automatic hierarchy/profile有效输入；不能仅依赖本次effective版本变化恰巧改变hash。
- 可以保守地给“可能使用该规则”的明确配置绑定版本，不要求为计算revision先生成一次世界；规则应从可信原始/有效程序确定。
- 保留A被冻结输入的既有revision语义和旧证据；不要直接全局替换WORLD_GENERATOR_REVISION导致所有无关legacy输入变动。
- 发布可核对的道路几何版本，并从原manifest/程序重建核对；不能信任产物自己声称的tag。旧字段缺失如何处理必须明确：未受影响legacy可以沿原合同；应绑定新规则的产物缺失/错误版本须拒绝或明确unsupported，不静默当最新。
- 不要求兼容重建所有历史坏的16m世界；可以诚实拒绝，但不得覆盖旧世界或将其改成新版本。

### 验收

1. 检查员version-probe所用的无profile显式16m输入：新身份必须与旧legacy公式不同；payload可追溯到道路几何revision，而不是靠更改name/seed/rulesDir。
2. automatic hierarchy中16m与有profile路径也绑定；保持确定性，相同输入同版本同revision。
3. A冻结manifest的revision保持；给其真实路径、文件hash和只读计算结果。至少补无profile/无受影响宽路的legacy对照。
4. 单独模拟道路几何版本v1/v2时身份不同（生产版本payload辅助函数可在单测传参），不能只改morphology revision证明道路绑定。
5. 发布字段缺失、篡改、与世界身份不一致的负例由独立审核拒绝。自给TempDir小集成即可，不依赖旧work存在时才执行。
6. 不改本轮max(7,半路面+1)规则，不重写道路搜索，不开放AlignmentContext/ZoningMask。已有35/35生产几何证据引用，不要求重跑正式控制世界。

## 4. 白名单

- `bevy/crates/core/src/settlement/morphology_compile.rs`及就地测试：轴选择/来源/有效语义revision。
- `bevy/crates/core/src/worldgen.rs`：道路几何版本的适用判断、身份payload/发布元数据及必要测试；不得顺手重构大文件。
- `bevy/crates/cli/src/main.rs`：仅独立版本重建/核对接线。
- `bevy/crates/cli/tests/morphology_compile.rs`；可新增`bevy/crates/cli/tests/graded_geometry_revision.rs`自给小测试。
- 确需调用共享版本判定时，`bevy/crates/core/src/worldgen/road_network.rs`仅版本接线/测试；不改工程算法或搜索。
- 新结果`docs/collaboration/r2-w1-closeout-result.zh-CN.md`，前轮结果只追加勘误。
- 新证据`bevy/work/collaboration/ds-r2-w1-closeout/`，create-new，不覆盖旧世界/旧receipt/检查员输出。

其他文件只读。需要白名单外改动先记录最小必要理由与阻断，不自行展开。

## 5. 验证与交付

定向测试单项120s，首次必要编译300s。只跑相关morphology、版本单元、CLI自给小集成；如道路算法未动，不重跑所有历史工程测试。最多一次自给TempDir bake供发布/审核版本正负例复用。**不再生成正式512m控制、不跑release/最大图/浏览器/全workspace、自然四档或ignored搜索。**

如小集成遇阻，先交付源码/定向结果与确切原因，不通过新增大图重跑替代诊断。不能将“测试通过”和“全部验收通过”混淆。

交付两项状态、source hash、命令/exit/log、所有轴反例、版本适用表/A保持证据、发布/审核正负例、旧证据完整性、实际bake次数。修正文档六组/八组、ignored是否保留、18m实际blend口径，不扩展任务。

跑doc-tree:check和git diff --check。不自行签收B，不提交Git，不启动agent，完成停止等复验。
