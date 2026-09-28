# V4.1 B01-G4：固定V5几何的共享节点高程小实验

## 1. 目标和范围

G3-R1已accepted-closeout。不要再重开来源/yard/计分/生产clock修订。先读b01-g3-r1-closeout-inspection.zh-CN.md及G3检查员probe。

本轮只问：**在固定V5几何与现行生产求解/匹配合同下，少量有来源的B/C高程组合能否使两段同时通过，并进而得到一个真实新增地块？** 不是求24、不声称高程域穷尽。原16住宅+8店铺需求保持。

固定：A=(288,96)/8000mm，B=(288,180)，C=(176,180)，AB84m、BC112m，均12m local/urban，shoulder3、junctionLength8、maxFill6000、maxRise80。**C只允许改高程CLevelMm，不允许改坐标/街段长度。** 不变seed/自然/zone/water/既有35边27节点21义务/需求/insets/setback/预算/请求顺序。

不改carve、solver、profile、保护合同、生产默认、公开接口；不Git写操作、不agent、不F/A、不B签收/推进C。

## 2. 准备与预算：避免先做一次无用realize

- 测试专用harness复用现有生产叶子和G3-R1 BundleOutcome/joint_support_diagnostic，cfg(test)+ignored，不接生产搜索策略。
- 从原manifest与现有可信重建链准备一次35边roads-only基线，验证原登记natural/plan/identity，不用仅边数作为等价证书。此阶段不运行baseline realization。
- seed/precision由真实manifest规则取得，不硬编码成通用seed42。保留冻结输入hash；旧证据不可改。
- 全轮自然实验最多**一次harness调用**；实际高程状态≤6，不重复旧五个(B,C及参数相同)或本轮同态。失败重启也计预算，不获新额度；若运行缺证据，不自动重跑。
- runtime总≤180s，编译单列；使用已验收晚截止门。自然基线realize≤1，仅第一次需要完整验证时做；候选realize≤2，复用该基线。无任何正式/TempDir bake。

## 3. 候选表：B只取两个有来源值，C最多三种规则

先用create-new写入候选规则、输入hash和排序，再执行；登记算法派生规则可以早于其具体求解值，结果只追加。

B取值最多两个：
1. **3000mm**：检查员已证明接入design/verify/append合法，完整束未测。
2. **2640mm**：由固定根8000、AB84m、首端8m平台、B支撑交集须匹配末端9m给出的必要下降下界8000-80×(84-8-9)=2640。这里只是必要候选，不预先声称合法，也不把junctionLength改成9。

每个B，C高程候选最多三种（坐标始终176,180）：
- 生产infer_end返回的最小端点，明确记录returnedMinimum与chosenC。
- C=B：有来源的同高端点对照，不等于整段平坡，不强改profile。
- 端部平台/4m padding全支撑自然与实际zone填方上限给出的必要C上界，与从B可达约束相交后选取一个有明确来源的上界候选。该值仍必须经生产design/append检验，不把它当已证最大可行值；区间空或与前项重复就跳过并记录，不凑6态。

可先用必要边界排除明显不合法项，排除项与真正生产attempt分开计。禁止均匀扫毫米域、额外B/C、移动C、换宽度/平台来补失败。若某派生需调用production solver，次数如实计入，不藏在“只读预处理”中。

## 4. 首要检查：B的完整交集匹配，不能只盯端点

各态在baseline副本上生产append AB，再以同B高程生产design/append BC。第一段允许暂存但不产生信用。任一失败整束丢弃；旧plan/session保持。

用已修正诊断，在[279,171,292,184]完整196格上记录两侧profile/27类偏差；两侧取值必须走height_at。关键样本：AB末端station75(距B9m)、BC station9，以及端外padding。记录两端设计profile或其可重算来源/hash，不为补这些数据再运行一遍。

可以解释生产最小profile的约束来源：根端/终端平台传播、自然下界如何决定第9米样本。必须区分：
- 采样状态被拒；
- 某个已给定上下界内的数学必要矛盾；
- 确定性生产solver输出不匹配；
- 所有合法工程profile不存在。
不允许从前两三个失败点跳到全域无解，或把更高/更低的自由度自动排除。

若解析必要界已证明某个登记状态无法满足匹配，可直接记拒绝并保留坐标/不等式，不另造solver或改生产容差。

## 5. 只有两段通过才进入地块链

- 两段都经生产append通过后，再调用现有共享RequestBudget/PlacementState/frontage测量；窗口数/预测数不是信用。
- 测量无可用增量则拒绝，不跑完整realize。
- 首次需要完整realize时才计算一次真实baseline，建立全部旧parcel/owner/identity/access/yard承诺。复用SupplySession、realize_candidate、commit_decision和现行来源声明/production encoder/shared audit。
- 候选完整审核要求planned==realized-with-access、新parcel yard缺额0、旧逐parcel不恶化、旧ID/geometry/入口/access不丢失、两请求不互换数量；non-yard已知限制如实保留，不扩开发新审核。
- 整束收益至少1且所有审核通过才记录bestLegalCandidate；仍只在测试内暂存，不发布世界。首个见证证据齐全即停，不继续追24。
- 任意晚截止清空候选、incomplete=true；无部分prefix信用。候选budget耗尽且还有登记项未验证需明确停止原因/未测项，不能宣称集合全完成。

## 6. 代码/验证/交付

仅auto_roads.rs现有cfg(test)区新增独立G4入口及最小帮助；尽量不复制大段已验收链。不得修改旧G3表/证据、不用新入口自动再次执行旧G3用例。Rust定向编译+G4必要小测试后，只执行新自然入口一次。旧6小测试可按需要--exact重用，不宽过滤自然test。

不跑原五态/128/clock80/carve/CLI生成或gate/release/browser/max/workspace。不添加公开manifest开关、不改内部类型可见性、不实现新的平台/共享支路合同。

新增ds-b01-g4-fixed-v5-joint-level/：规则表/实际派生表、输入与源码hash、单次run日志/JSON、每态全部阶段/理由/坐标/profile、196格诊断、主baseline前后hash、真实操作计数、如有见证的全部parcel归属/yard/access/审核信息。

报告b01-g4-fixed-v5-joint-level-result.zh-CN.md；回执区分unique state、实际状态尝试、design/infer/append/measure/realize各类调用和内部重建；不把函数内部调用隐藏成0。完成doc-tree/diff/hash检查。

若全拒绝：best=null，只关闭本次小集合并提出准确原因，不直接更改合同或转设计B。若正例：increment_witness_ready_for_review，不是24验收或ready_to_generate。完成停止，B partial/C不推进。
