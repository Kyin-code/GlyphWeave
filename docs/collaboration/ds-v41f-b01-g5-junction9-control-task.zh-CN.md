# V4.1 B01-G5：9m端平台的最小参数对照

## 1. 先修G4口径，不重跑G4

先读b01-g4-fixed-v5-joint-level-inspection.zh-CN.md。G4六态negative observations已限定接受，旧表/run/receipt不改。

本轮开头一起完成：
- 向G4报告追加勘误：撤回“8m必然不可能/唯一出路改合同”，保留六态失败与未穷尽。
- 后续Mm字段必须乘1000并转整数，补小转换测试；旧2.92等值在新勘误说明，不覆盖旧JSON。
- 完整端平台+padding采样按行进方向及实际junctionLength推导，不再端点左右各4m冒充全平台；G4完整范围x172..184给更紧必要界5314，引用检查员证据，不补跑。
- 准备阶段调用与candidate-stage计数分列，内部调用未插桩明确写未知，不能都叫全轮worker calls。

## 2. 明确本轮唯一参数变更

只对两条拟新增段，将**junctionLengthM从8改为9**。现生产append已支持该参数，无需修改解析器/solver/共享支路语义/版本标签/道路审核，也不能改生产默认。

固定：A=(288,96)@8000mm，B=(288,180)@3000mm，C坐标=(176,180)，AB84/BC112，两段12m local/urban，shoulder3，fill6000，rise80。旧35边/27节点/21义务及旧profile一律不变。seed/natural/zone/water、原16住宅+8店铺及尺寸/insets/setback/预算/顺序不变。

12m的junctionRadius=max(9,12/2+3)=9，与G4相同，不扩大豁免；只是两条新段端平台各增加1m。该变化是输入参数对照，不宣称能通过地块，也不需要先证明旧8m全域无解。

## 3. 预注册：最多两个状态，不增加轴

候选规则先create-new登记并hash，随后执行：
- J9-R1：B=3000，C高程由**junctionLength9条件下**生产infer_end派生，记录returnedMinimum与chosenC；不得复用旧8m的-382当作新最小值。
- J9-R2：B=3000，C=3000。若与R1重复则只运行一个。

若AB自身拒绝，记录新参数真实原因即可停止，不换B、不回到8/10m、不平移C、不重跑旧G4六态。推导失败计入执行/派生计数，保留生产原因。禁止加第三个状态。

## 4. 运行链与诊断

一次可信重建冻结基线，核对登记natural/plan/旧身份，不只比35边数量。使用原manifest来源声明，不做采样猜来源；未涉及的源码保持hash。

每态在副本上生产append AB、BC，固定B同值，父节点/完整自然fill/no-cut/zone/water/padding/逐格匹配审核全部不跳过。第一段零收益允许暂存，但不能形成提交信用。

记录完整196格诊断：two profiles height_at、missing=0、正确毫米字段、AB station75和BC station9、对应profile/request/hash。预计9m平台应覆盖交集只是待验证假设，真实production拒绝高于任何预期。

端平台范围必须使用junctionLength9：C向西段端平台x176..185、端外padding至172，完整范围x172..185，横向z171..189。其它朝向用统一方向推导。只做本轮所需边界，不另枚举新候选。

## 5. 先工程后内容，严守惰性实现

两段通过才运行现有frontage候选测量。测量无增量则拒绝，不跑realize；预测窗口不算信用。

第一次确需完整验证时才baseline realization1次，建立全部旧parcel/owner/ID/geometry/access/yard承诺；其后复用。候选按现realize_candidate/commit_decision链审核，新增yard0、旧逐parcel不恶化、planned==realized-with-access、需求不互相交易，真实增量至少1才生成见证。

首个有效见证且证据齐全立即停止；完整失败保留真实阶段，不追求本轮24。测试内暂存，不发布世界；任意失败/超时丢弃整束，不留第一段prefix。

复用G3-R1结果截止门，baseline/candidate长阶段返回及输出前复查；晚到期incomplete=true、best=null。不要扩改生产时钟项目。

## 6. 上限与白名单

- 整轮**1次自然harness调用、≤2唯一状态、0重复**；总运行≤180s，编译另计。
- baseline realize≤1、candidate realize≤2；均按实际全轮累计。
- 0 bake（含TempDir）、0 G3/G4重跑、0 128/clock80/carve、0 CLI世界/gate/release/browser/max/workspace。
- 只改auto_roads.rs的cfg(test)入口和最小诊断帮助函数；小单位转换/端平台范围测试先跑，预先检查过滤不会触发自然用例；不开内部public、不改生产策略/默认/审核/求解/地形。
- 运行前一次准备齐诊断字段，不因补日志再执行自然实验。编译/测试/自然调用与预算分项记录。

## 7. 交付和停止

新增ds-b01-g5-junction9-control/：规则表、单次日志/JSON、输入源码hash、各态请求/profile/196格诊断、调用计数、基线前后bytes/hash、如有见证的全部parcel/owner/yard/access和审核摘要。

新b01-g5-junction9-control-result.zh-CN.md及receipt；G4只追加勘误，原证据保持。hash/diff/doc-tree检查，未改白名单外源文件。

若联合工程通过但地块失败，清楚报告工程进展和内容首拒绝，不能算整体成功；若全部失败，只关闭本次两态，不全球归因；若真实增量通过，increment_witness_ready_for_review但不bake、不签收B、不推进C。停止等待主检查员复验，无Git写操作/agent/F/A变更。
