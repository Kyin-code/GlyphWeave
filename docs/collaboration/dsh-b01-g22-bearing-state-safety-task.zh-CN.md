# B01-G22：承托身份与覆盖结果分离——DSH单一执行任务

日期：2026-09-20。用户已要求发布下一阶段任务，本文件为执行授权，不是确认稿。无需回复收到，按本任务完成一个连续、有界的实现与验证包；预算不足或必需改动越范围时准确停止。

## 0. 唯一目标与不变项

**消除“求证失败被当作无需结构承托”的生产及独立审核放行路径。**

用两个独立事实表达承托身份和覆盖结果；对真实裸地继续使用权威地形判据，对结构承托要求覆盖证书，对无法判断的情况拒绝。保留G21已通过的楼梯几何、有限事件扫描及原拒绝语义，不再重做楼梯、挪动端口、提高事件上限或放宽阈值。

本阶段只处理现有engineering_connections模块，不新增设施，不接ST18/默认世界。A冻结、B partial、C不推进；G19R3C/G20/G21旧状态不倒写。

## 1. 冻结起点与证据来源

起点已经检查员只读核实：

- G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/worldgen/engineering_connections.rs，SHA256 **39b3260e26da75280fa93c4ed78df8ed088503bdf6caed7be5709b26798ce5bb**。
- G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/worldgen/engineering_connections/support_coverage.rs，SHA256 **200c09bbfd786439dfcabd50e31ac06dae85df5beb5e62b7ce912d13b9c38068**。
- CLI源码SHA256 **99c90e46f1a926e61d654e74e35372e45eaddfaebacba2b16898be700205ecd6**。
- 检查报告：G:/difyllmwiki/GlyphWeave/docs/collaboration/b01-g21-closure-inspection.zh-CN.md。
- 已证完整CLI差分输入：
  - G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g21-closure-20260920/ground-narrow-control.json
  - G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g21-closure-20260920/ground-narrow-with-remote-parts-v2.json
- 对应ground-differential-facts.json、closure-probe.log和build/verify日志均在上述检查员目录，只读引用。
- 原偏移端点：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g19r3c-final-20260920/endpoint-near-edge.json。

两个ground差分输入必须原样验收：共同本地连接不变，仅加远处不连通构件。不得用“100m外新例”替代现有已复现反例。它们是人工健壮性输入，不是现实楼梯设计推荐。

新证据目录：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g22-bearing-state-safety/。创建时目录不得已存在；新产物create_new，不覆盖历史目录。执行前绑定源码/资源，发现基线不同须记录并核对来源，不自动回滚。

## 2. 正确状态规格：两个轴，不是一个含混布尔

### 2.1 承托身份（独立于宽度是否合格）

建议命名，可等价实现但必须有明确类型：

- `structure`：已确认有效层的真实结构部件承托该区间中心线；**不意味着够宽或已认证**。
- `no_structural_support`：在有效几何、有效层及完整评估的前提下，已证明该区间无结构承托中心线。
- `unknown`：身份未能确定，包括无效输入、层关系不明确、承托分类预算耗尽或数值不确定。

纯几何层不要自行返回“terrain可走”。只有ground消费者结合权威地形与已有走行判据，才能解释为地形承托。缺结构与地形可通行是两件事。

### 2.2 完整带覆盖结果

- `certified`：已证明所需带连续承托；
- `gap`：有不足覆盖的真实见证；
- `indeterminate`：覆盖评估未能完成；
- `not_applicable`仅用于已证明无需结构证书的ground区间，不能由异常转入。

身份和覆盖的原因分别记录，例如`no_candidate_parts_proven`、`coverage_budget_exhausted`、`invalid_frame`、`numerical_uncertain`，不能统一压成false。

### 2.3 消费规则

| 承托身份 | 覆盖 | ground处置 | structure边处置 |
|---|---|---|---|
| structure | certified | 此支持门通过，其余原判据仍须通过 | 此支持门通过 |
| structure | gap | 拒绝 | 拒绝 |
| structure | indeterminate | 拒绝 | 拒绝 |
| no_structural_support | not_applicable | 继续/保留权威地形、坡度、水、障碍、采样等全部判据 | 不允许降级成ground，拒绝 |
| unknown | 任意 | 拒绝/明确unknown，不放行 | 拒绝 |

禁止继续用`centreBorne && !carried`决定是否免检。旧centreBorne如保留诊断，不能替代上述状态。窄结构必须是structure+gap；失败不能让它失去结构身份。

若已独立证明structure，后续覆盖预算耗尽应保留structure+indeterminate；若连身份也未知，应unknown。两者都拒绝，不强迫统一为某一个状态。

## 3. 带外部件筛选：安全优化，不是正确性的唯一防线

生成昂贵事件前保守筛选支持候选：

1. 校验用于筛选的部件/层/段几何。无效frame不能因算不出边界被归为带外。
2. 只有能证明与闭目标带(t∈[0,1], |s|≤r)不相交、或依据可靠层身份明确无关的部件才能排除；接触/浮点歧义保留或返回unknown。
3. 先筛选再估算事件成本，但**筛选后仍超65536也必须安全拒绝**。不得仅抬高硬帽使固定反例恰好通过检查。
4. 考虑全部完整带，包括不交中心线的横向补宽件；不得重现G20漏筛侧条。
5. `partsConsidered`/`partsFiltered`明确是候选、入求证集合还是剔除集合；记录稳定ID与筛选原因。贡献parts单独记录，不能与考虑过的部件混称。
6. 加入带外无关构件不能把本地应拒转为通过；零/少量无关构件也不能改变合法本地证书的安全结论。

不要求固定点/任意精度库。按原浮点容差合同保守处理歧义，不用epsilon抹除缺口。

## 4. 生产与指定审核必须同源传播

实际审计并列调用矩阵：

- build_ground_edge；
- refresh_walkable_capabilities；
- verifier的ground与structure分支及stored_capabilities_resampled；
- structural_support_detail / structural_support / structural_support_from_frames包装；
- band_coverage_detail / band_coverage / band_coverage_vertices旧入口及全部调用者。

新状态、原因与覆盖事实由同一生产支持评估/决策代码产生；消费者不得各自再做bool兜底。诊断采样函数不再给生产或审核提供“已认证”的依据。原access地形/障碍/水判据保持不变；裸地地形是否安全仍由权威路径判断。

保存包指定支持重查从保存几何、有效层、部件身份、有向折线及权威能力规则重算；ground证据也必须有明确的保存/审核方式，不能只有结构边保存新状态。

严格比对所有被定义为被审事实的字段：承托身份、覆盖结果、失败原因、所需带宽、区间、层、贡献part、筛选证据、witness；缺失、非法枚举、错类型、部件ID不符都失败。禁止filter_map丢失非法元素后视为合法。

若partsFiltered引用整场景的带外ID，不应要求它们出现在贡献parts；审核要复算筛选依据。事件计数可以作为成本事实，但不能被包内伪造值用来绕过预算。

证据语义变更须有可识别schema版本或明确迁移；不改旧包字节，不把缺新字段的旧包静默当作裸地。当前默认世界schema不动。

## 5. 必须钉住的回归（先核心、再CLI）

### N1：已复现的无关构件反转

原ground-narrow-control与ground-narrow-with-remote-parts-v2均拒绝。后者不应再因远处构件事件预算溢出变成terrain。正常筛选完成时应回到本地窄承托拒绝类别，并能说明无关构件被排除的依据。

保存两个输入原样，验证本地几何/地形/目标不变；生产build负例须进入预期支持阶段，不用另一处早退冒充修复。

### N2：裸地不是免检

- 已证无结构、地形齐平且其余条件清晰的ground正例通过；
- 同类裸地的坡度、水、缺采样等原负例仍拒绝；
- 结构边不能借no_structural_support自动降级成地面边。

### N3：仍然会遇到的不确定必须拒绝

至少覆盖：

- 带内相关部件或较小显式求证预算使覆盖预算耗尽；
- 无效frame；
- 数值/层身份不能确定。

证明**生产消费和指定审核**都拒绝，不只assert求证器返回unknown。可在核心测试中通过真实支持评估入口传入较小预算，避免为造测试扩大世界；必须使用生产同源预算参数/逻辑，禁止一套仅测试的模拟闸门。不新增用户可见CLI放宽开关。

### N4：身份与覆盖的单字段自洽篡改

分别改structure→no_structural_support、structure→unknown；改gap/indeterminate→certified；删除原因或身份字段，非法枚举；伪造筛选/贡献part。每个样本只变该字段并重算hash，指定支持重查独立拒绝。整包diff检出不能代替指定检查结果。

若真实gap包不能由正常发布产生，可在合法保存包中构造需要的审核反例，并明确原始/变异状态；不要声称生产发布过拒绝包。

### 既有正负例

保留现有25项具名测试语义与最小直跑/折返/宽平台、原偏移端点、墙穿、侧向补宽、仿射包络、实际见证、无效输入回归。不能删除正例或挪坐标；新增测试名称独立列出，按实际日志计数。

## 6. 最终CLI清单：17次基准调用，另有有限重试余额

所有调用绑定最终源码构建的二进制；build内部route不另计，但独立verify是另一CLI调用。

1. 原ground-narrow-control build拒绝；
2. 原ground-narrow-with-remote-parts-v2 build拒绝；
3–4. 裸地合法control build/found + verify；
5–6. 正常结构接驳positive-terrace build/found + verify；
7–8. minimal-straight-stair build/found + verify；
9–10. minimal-switchback build/found + verify；
11. 正确来源endpoint-near-edge build拒绝；
12. 墙穿楼梯build拒绝；
13. 保存身份字段自洽篡改verify，指定支持检查失败；
14. 保存覆盖结果自洽篡改verify，指定支持检查失败；
15. 保存几何缩窄自洽篡改verify，指定支持检查失败；
16. payloadHash篡改verify，payloadHashMatches=false；
17. 新路径final-write-sync实际钩子失败，无bundle/views残留。

窄/宽平台及地形、水等对照须在核心矩阵保留；CLI余量允许补跑但不超过总额。预存冲突不能代替第17项，台账必须依据实际类别/日志。

core定向测试可兼做显式导出，但测试调用与cargo build分别计数；普通测试不能写冻结目录，导出只写本包唯一新目的地并create_new。

## 7. 权限白名单与预算

### 白名单

允许：

- G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/worldgen/engineering_connections.rs；
- G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/worldgen/engineering_connections/support_coverage.rs及必要局部状态/几何辅助模块；
- 必要的G:/difyllmwiki/GlyphWeave/bevy/crates/cli/src/engineering_connections.rs；
- G:/difyllmwiki/GlyphWeave/bevy/crates/core/tests/resources/g19r2/中的新增反例资源/测试辅助，必要证据schema和文档。

禁止：改access/auto_roads、默认世界、净宽/步高/碰撞/水阈值、合法楼梯坐标、既有拒绝预期；新增设施/大依赖；Git add/commit/stash/reset/rebase；自然/ST18/bake/realize/世界发布。

### 单独记账，避免重复结转

- 历史G21已用cargo8/12、CLI24/24；历史G20超支1保留。
- **G21未用cargo4次一次性转入G22，另新增cargo8次，G22总上限12次**。转入后G21不得再次动用该余额。
- **G22新增CLI24次，总上限24次**；G21 CLI无余额，不结转。
- 此前检查员消耗永远另列，不算DSH剩余额度。
- 每条cargo check/build/test及失败调用都计数；一个shell中两条cargo就是两次。预期拒绝、导出测试、CLI冲突和重试均计数。
- 预留最终核心回归和CLI构建额度；实际执行版变更后只能复用未受影响且源码关系明确的证据，不能拿旧CLI替当前受影响分支背书。
- 人工栅格≤64×64、结构≤64、节点≤512、边≤1024；支持事件硬帽65536不提高。N3规模用同源可注入求证预算测试，不靠超大场景。
- CLI单次≤30s、累计≤600s；独立人工probe/脚本进程≤24次，失败计数，单元测试内部参数组单列而非每组当进程；归档≤64MiB。Rust编译和测试失败时间如实记录，不伪装自然运行。
- 自然/ST18/默认集成/bake/realize/世界发布总额度0。

原17项基准CLI加余量已在24内；不因某项属于任务就自动突破额度。预算不够则partial停止，不先超支再解释。

## 8. 实施顺序与一次交付

1. 建立本包基线与一页状态真值表，列出承托身份来源、覆盖失败传播、相关性筛选依据及调用者。此为包内工作，不单发确认稿。
2. 先写N1–N4和裸地/结构正例，局部修改类型与消费者；不重做事件扫描/楼梯规划。
3. 运行核心全回归并核对新状态每种分支；显式导出本轮审核篡改材料，保存全部失败日志。
4. 构建最终CLI，完成17项基准观察并查看当前生产生成的平面/立面/mesh，确认路径仍来自实际几何。
5. 核对无关构件差分不变性与指定审核身份/覆盖事实；普通裸地仍可通过，不允许“全部拒绝”修复。
6. 最终源码/资源/产物/progress定稿，清单→回执→只读核验。新日志引用旧证据，不覆盖旧目录或回执；旧包内容按hash验证，不能用时间戳代替。

汇报：一句话状态→命题/适用域/证据类型/文件+JSON路径/verdict→新信息≤3→实际调用和累计预算→剩余项→一次交付与核验。区分worker观察和检查员既有观察。

## 9. 正负分支

正分支：同一窄承托输入及远处构件差分均在正确支持阶段拒绝；裸地与合法楼梯通过；预算/无效/不确定不再绕过；指定身份/覆盖/几何审核反例均拒绝；最终版核心与CLI满足预算。提交delivered_for_inspection，不自动宣布整个G19/G20/G21验收或集成真实城市。

负分支：任一身份来源、失败传播、裸地正例、最终版本验证或预算门未闭合，准确partial并停止。不得把问题描述为“只剩补跑矩阵”而省略尚存在的实现缺陷；不得自动追加执行。

A冻结、B partial、C不推进。
