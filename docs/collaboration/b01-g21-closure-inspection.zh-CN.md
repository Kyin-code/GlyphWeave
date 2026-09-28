# B01-G21收尾独立复核：求证不确定被ground当作无需结构承托（2026-09-20）

## 裁定

**partial / ordinary-ground-negative-now-rejected / indeterminate-ground-bypass-reproduced**。

本轮三项修复均有真实进展：ground已接入新求证器；原NaN/零轴helper反例返回indeterminate；原侧条缺口见证移到真正未覆盖点。独立25/25核心回归及合法楼梯/terrace观察通过。

但不是“只差其余CLI矩阵”。新增centreBorne布尔把“未知”与“确定无结构承托”混为一谈；事件预算耗尽时生产ground与独立审核均跳过支持门。检查员已复现完整生产CLI差分：相同本地连接从拒绝变为built/found/verified，仅增加远处、不参与目标路径的构件。

保留partial停点，不批准整体闭合，不追认历史超支，不自动新增CLI额度。

## 1. 来源与核验

证据根目录：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g21-closure-20260920/。

- worker清单104/104 bytes+SHA256匹配；closure-receipt绑定2554b31dfc31aff99e3c678ffa70ca079f816574bdf763c3b4ec3295fbb67fec /16267B匹配。
- core361839B /39b3260e26da75280fa93c4ed78df8ed088503bdf6caed7be5709b26798ce5bb。
- support_coverage.rs19954B /200c09bbfd786439dfcabd50e31ac06dae85df5beb5e62b7ce912d13b9c38068。
- CLI源码99c90e46…；access/auto_roads保持前次绑定。
- 清除测试导出环境变量，独立重跑25项，25/25通过；重新构建本版CLI，未使用旧二进制代替。
- 独立CLI11次（含一个未到达目标阶段的中间人工构造），全部日志保留。不是替worker抹去“本轮仅3CLI”的事实；检查员观测另列。
- 检查员未编辑生产源码/资源/worker证据，未运行自然/ST18/默认世界。

## 2. 已接受的本版进展

1. 普通ground窄承托反例在本版确实返回ground_edge_unsupported_by_structure，不再仅有旧版helper参考。
2. positive-terrace、minimal-straight-stair、minimal-switchback各自built/found/verified均由检查员在本版复现。
3. 新路径--inject-failure final-write-sync实际到达真实原语，stderr为“injected failure before the real sync (production primitive)”，bundle/views无残留。
4. 原侧条helper差分中，新结构求证返回拒绝；witness现在为x=4.06,z=0.625，生产MemberFrame.contains(...,0.0)确认不在任何支持面内。该指定见证修复成立，不延伸成所有见证输入的全称证明。
5. 原centre.x=NaN和零along/across的两个helper反例现返回band=0，coverageVerdict=indeterminate，不再certified。

## 3. P1：indeterminate -> centreBorne=false -> 跳过ground门

### 源码路径

G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/worldgen/engineering_connections.rs：

- structural_support_detail约3659–3675行：求证器certificate.slices为空时，插入一个indeterminate区间，但把centre_borne写死false。
- build_ground_edge约3159行只拒绝interval.centre_borne && !interval.carried。
- verifier ground分支约7271行使用相同条件。

因此事件预算耗尽、无效输入或其它无法求证的早退，可能被解释成“这是无需结构承托的裸地”。这是状态丢失，不是正确的裸地/结构分段。

### 完整CLI负例与差分

共同本地条件：

- 64×64人工输入，natural/baseline均1000mm；本包不调用自然生成。
- 平台deck中心(8,1,8)、6×1.6m，z范围[7.2,8.8]；没有修改安全阈值。
- ground连接walk：(6,1,8.6)→(10,1,8.6)，目标BC，长度4m。
- 正侧余量0.2m，对称结构承托带仅**0.4m < 1.5m**。

**控制组** G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g21-closure-20260920/ground-narrow-control.json：

    build -> rejected / ground_edge_unsupported_by_structure

**差分组** G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g21-closure-20260920/ground-narrow-with-remote-parts-v2.json：

仅新增3组远处独立直跑构件及其端口，起点分别(20,1,20)/(20,1,30)/(20,1,40)，每组18踏、步高1mm、步长250mm；这些是现有模块允许的人工健壮性输入，不是声称现实楼梯推荐如此设计。

本地平台、原端口、地形、ground连接、障碍、水、目标和预算均逐字段保持不变。新增结构不连接a/b，最终路线也完全不经过它们。

当前同层支持part总数109，事件上界预估为96576 >65536。预估发生在空间相关性裁剪之前，远处构件也被计入。随后：

    certificate -> indeterminate
    interval.centreBorne -> false
    ground gate -> 找不到 centreBorne && !carried，放行

实际生产结果：

- build=built，route=found；
- pathNodeIds=[a,b]，pathEdgeIds=[walk]，4m；
- 独立verify=verified，findingCount=0；
- stored_capabilities_resampled=true。

证据包括完整输入、bundle、报告、生产视图、ground-differential-facts.json与closure-probe.log。

这次**不是仅helper风险，而是完整build/route/verify差分**。它证明当前模块的既定窄承托拒绝被无关构件数触发的预算失败绕过；不外推为真实地形发生了改变或现实必然发生坠落。

### 中间尝试披露

第一个额外构件版本使用2组30踏。该版本在远处结构自己的结构边上先触发求证预算拒绝，未到达目标ground阶段，不能作为ground绕过证据。此失败输入/日志仍保留，并计入检查员CLI调用；最终3组18踏版本才是上述完整复现。

## 4. 唯一必要修复方向

不要重做楼梯、事件扫描主干、碰撞或阈值。将承托身份与覆盖结果分开：

- **确定由结构承托**：必须有certified覆盖；gap/indeterminate均不能放行。
- **确定由权威地形承托且无需结构**：允许保留现有地形门；这个“无需结构”必须有独立依据，不是从求证失败推出来。
- **无法确定承托身份或求证失败**：明确拒绝/unknown，不能伪造centreBorne=false作为免检凭据。

可使用三态承托分类或带原因的证书；保留NoCandidateParts与BudgetExhausted/InvalidFrame/NumericalUncertain的区别。生产和verifier同时消费。

先在安全的空间/有效层筛选之后估算求证规模，避免无关远处部件无谓耗尽预算；但空间筛选是优化，**不能取代不确定时拒绝**，也不能只提高事件上限来隐藏问题。

必须钉住不变性：向场景加入不接触目标带、不参与路线的构件，不能把原应拒的本地连接变为可通行。新增负例至少覆盖真实有结构但预算耗尽、无效输入、正常裸地和正常结构接驳；不能粗暴地把无结构裸地全部拒绝。

## 5. 后续额度与停点

G21自报cargo8/12，CLI24/24，CLI余额0。历史G20超支1仍保留。检查员运行不计入worker，也不产生可供worker继续使用的额度。

本报告不自动发布新执行包或新增预算。保持当前partial；在新的明确授权前，不要求另交确认/勘误包，不改冻结证据。后续授权应集中在此状态传播修复与成对验收，而非以“补跑其余11项”作为唯一任务。

检查员成本：cargo2、rustc1、生产库probe进程1、CLI11（含一个中间失败版本）；自然/ST18/bake/realize/默认世界/生产源码修改0。其他矩阵项未在本检查重复，未声称完整最终矩阵验收。

A冻结、B partial、C不推进。
