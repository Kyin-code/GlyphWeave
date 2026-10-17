# V4.1 B01-G3：只验证 V5 两段路束是否产生一个真实增量

## 1. 目标与禁止事项

先读 b01-g2-r1-design-inspection.zh-CN.md。本轮只实施测试专用实验，复用已验收的来源、审核、事务与旧parcel保护链；不重新开发生产供给算法。

唯一问题：冻结W1真实自然/需求/35边基线上，V5两段路束能否带来至少1个生产审核通过、有access、有完整yard的新parcel？0增量也是有效实验结果，但必须定位真实首拒绝和统计范围。原16住宅+8店铺需求保持，8只是该束几何窗口，不改成目标8、不宣称24。

不改carve规则、fill/坡度/支撑容差、保护合同、seed/地形/zone、固定根、既有路/节点/21旧义务及需求尺寸/insets/setback/请求顺序/预算。不开内部类型public。B不签收，C不推进，不Git写操作，不启动agent，不动F/A及旧证据。

## 2. 唯一候选几何与基线

- A=(288,96)，原固定入口8000mm；B=(288,180)新增共享节点；C=(176,180)新终点。
- A→B长84m，B→C长112m；两段12m、已授权local/urban、shoulder3m、junctionLength8m、maxFill6000mm、maxRise80mm/m。
- V4/V6和新枝/其它宽度/坐标均不进入本轮。不得因V5失败自行换案。
- 冻结输入取 ds-b04-c1-r2-b02-w1/gen-w1-collector-local，使用现有可信重建链，不把带frontage的实体计划冒充roads-only基线。先确认35边/27节点/21义务与已归档输入一致。
- NaturalSource用真实原manifest声明；seed/precision从统一门面导出。不能恢复Undeclared/seed0/采样猜来源。
- actual effective请求由生产编译链读取，确认与冻结显式需求和院落几何相同；若重建出现超出已知身份版本的业务差异，停止报告，不能随意重绑成新的成功基线。
- 完整baseline realize最多1次，建立SupplySession全部旧parcel/owner/identity/access/yard承诺，并记录真实基线；旧yard历史9+4只作交叉核对，不硬编码为豁免额度。

## 3. 先登记至多12个状态，再执行

1. 状态唯一键至少含几何、两段方向/顺序、A/B/C毫米高程、宽度/肩宽/junction/填方/坡度参数。A不可变；两段B必须是同一个值，C自由候选但必须有出处。
2. 高程只由生产infer_end或已披露的自然包络/固定端可达必要界提出。保留出处和排序；不凭结果临时扩大范围，不复制/改写production solver，不用单站[1920,7692]当联合可行解。
3. 如C依赖B的production infer_end，先冻结B候选及C派生规则、异常分支、状态上限；实际派生值逐项追加结果，不把失败派生从attempt计数剔除。无需为填满12个状态做无意义尝试。
4. 表写新文件wx，运行前记录sha256/输入源码hash/命令；日志先声明表hash再调用生产链。改表另起revision并披露，所有尝试累计预算不重置。
5. G2-R1小问题在新证据/文档中纠正：effective insets、production stations、三带共同交集、padding范围、闭区间vs面积、9 unique/18历史评估、hash一致不能证明历史预注册。纯区间负例[0,10]/[0,4]/[6,10]应false。不要改旧JSON/回执。

## 4. 整束暂存，不提交零收益前缀

测试harness在计划副本中按A→B、B→C调用现有corridor design/road_extensions::append_one。第一段没有parcel收益允许暂存，但不加入正式session、不作成功提交、不产生信用。

共享B不仅端点同高程：必须让生产append逐格审核兄弟路支撑交集[279,171,292,184]。9m支撑与8m平台差异尤其要记录：不扩大junctionRadius、不改profile第9米、不跳过相交格。完整padding、自然fill/no-cut、water/zone均交生产审核，不凭粗筛通行。

任意阶段失败/超时/无真实收益，整个副本丢弃，主baseline序列化bytes/hash不变；第二段失败不能留下第一段。只有两段共同通过且真正新增需求实现、commit_decision通过，才作为测试内whole-bundle候选原子采纳；不写入冻结世界。

## 5. 信用与验收链（不新造计分器）

复用已有RequestBudget/PlacementState/frontage::append、realize_candidate、commit_decision、SupplySession::adopt。候选最终源必须遵循现生产encoder和source声明，复用共享road audit及extension来源重建。

完整realize包括development/landcover计划及verify、真实surface/yard、access、全部既有承诺；按request的realized_ids_by_request计真实增量，planned必须与realized-with-access一致。不能只拿测量/预测窗/规划数作信用。

要求：旧实体/parcel/owner/几何/固定入口/access不丢失不转移；旧yard逐parcel不恶化；新增parcel的yard缺额0；新请求不能互相交易数量；基线无关non-yard缺额仍按现批准标准如实披露，不在本轮扩成全新审核项目。

## 6. 最小测试与预算

允许代码范围：优先在 auto_roads.rs 已有cfg(test)模块新增1个ignored V5实验；确需辅助可在 frontage_supply.rs 增测试专用帮助函数/最小pub(super)可见性。不改生产默认、manifest schema、世界revision、规划/求解/审核逻辑。不要为了以后量产先接一个新公开开关。

小测试证明：
- 共享B值不一致被生产拒绝；若已有对应测试，可引用/定向运行，不重造体系。
- 第二段失败/零收益/实验超时后整束baseline保持，不遗漏临时第一段。
- 真实增量才可提交；若本轮全失败，不伪造一条自然正例，记录正路径not_exercised。

整轮自然实验累计≤12个attempted bundle states，候选完整realize≤3次，基线完整realize≤1次。所有自然实验草稿/失败/重启都计入，按唯一状态与实际调用分别报告；不偷偷重试同态。总实验runtime≤180s（编译单列），超限立即退出且不提交。必要的小合成守卫测试单列，不用大量fixture隐藏bake。

生产design/append失败、测量0、不满足必要保护的状态先拒绝，不能为了凑3次realize而执行。找到首个真实增量且证据齐全即可停，不追求扩到24。

正式或TempDir bake均0；不跑CLI世界生成/gate/release/浏览器/最大图/workspace；不重跑128/80clock/carve诊断。定向编译/小测试与本轮唯一ignored实验即可，所有命令、退出码和耗时入回执。

## 7. 交付与停止点

新增 ds-b01-g3-v5-bundle/：预注册表及hash、append-only run记录、每态首拒绝/坐标/层级、B/C高程来源、两段profile/父节点绑定、actual design/append/measure/realize计数、主baseline前后hash、通过候选的完整审核摘要和新parcel ID/归属/yard/access。

新文档 b01-g3-v5-bundle-result.zh-CN.md；仅向旧设计文档追加勘误；旧receipt/表/世界不覆盖。回执绑定实际源码/输入/日志/结果hash，budget严格区分本轮、历史、检查员。

若成功：bestLegalCandidate为完整两段+真实新增ID和审核证据，状态increment_witness_ready_for_review，不是ready_to_generate。本轮仍不bake、不接生产策略。
若失败：bestLegalCandidate=null，明确实际尝试集合/是否耗尽，记录设计拒绝、匹配拒绝、零地块或实现拒绝；不得声称全局无解，也不自动转工程政策B。
若超时：incomplete=true，无部分提交。

最后doc-tree:check与git diff --check；检查冻结输入、旧回执及本轮非白名单源码hash。停止等主检查员复验。
