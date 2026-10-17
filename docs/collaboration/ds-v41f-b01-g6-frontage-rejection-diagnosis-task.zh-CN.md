# V4.1 B01-G6：只诊断G5两态的frontage拒绝，不扩搜

## 1. 已接受边界与本轮目标

先读b01-g5-junction9-control-inspection.zh-CN.md。G5两态道路生产append/接点匹配通过已作为规划阶段进展保留；full final工程/yard/access链未运行。BC两态各16次尝试预测0属实，但只保存首个总类，尚不足以指导改参数。

本轮唯一目标：用**同样两态**的生产measure trace给出这32次尝试的具体首拒绝。不是新搜索、不是实现新策略，不尝试消除拒绝。

## 2. 先做不需重跑的纠正

G5文档只追加勘误，旧table/run/receipt保留：
- rule-table1465bytes，sha256=8f2efc8c52315765bb3230eec1584207848cf95bce2f56600a74ea5ccb5d8327；blake3=75900cd8564dcf897e5596790df3e14f4f02a37be3238ef065df8b9184981e89。字段写完整算法名，不称同值；新回执分别绑定两种算法。
- 状态分层：roadAppend=pass、junctionMatch=pass、frontageMeasurement=zero_predicted_gain、candidateFinal/development/landcover/accessJointAudit=not_run。不能称最终工程全部pass。
- firstReason只是保存的首拒绝总类，不是全部候选的原因分布。既有请求16+8不变。

## 3. 唯一允许的自然动作：一次诊断重放

本轮明确允许同两态**一次diagnostic replay**，与G5历史运行分开计数。不得自动再跑原G5入口写入旧目录，另建测试专用G6入口和新证据目录。

固定参数不变：A(288,96)/8000、B(288,180)/3000、C坐标(176,180)，两新段junctionLength9/width12/shoulder3/fill6000/rise80；C高程只取G5已登记的-382、3000。以旧规则作为来源，不新增infer搜索；若选择重新infer交叉核对，最多1次并如实计数，不据返回值另建状态。

固定原manifest、natural、zone、水、旧35边27节点21义务、请求尺寸/insets/setback/顺序/预算。BC是本轮唯一测量道路；不顺便测AB/其它道路/新坐标/新高程。

一次基线重建，每态在独立副本生产append AB/BC并调用现有measure_road_for_request。复用已验收RequestBudget/PlacementState，不复制审核决策，不修改frontage生产代码。trace sample_cap仅在这个测试调用提高到至少8以保存每请求全部8次尝试，不改变maxCandidateAttempts/站点/选择结果。

## 4. 每态、每请求、每站点证据

保存原生产trace（rejected计数、accepted IDs、rejection_details、samples），不再只保留firstReason。应有2态×2请求×4站×2侧=32条尝试；这代表16个状态内几何窗口×2请求，不代表32个可同时地块。

每行至少：
- state、requestId/kind、roadId、station/side、候选中心、yaw、building footprint、完整parcel、生产support_check使用的building+padding支撑范围（不同几何不要混称）。
- 真实production reason和note；若natural_support_or_fill_limit，原样区分natural_above_design_level / fill_exceeds_roadbed_limit / fill_exceeds_zoning_capacity。
- 首个违规点at、naturalMm、designMm；requiredFillMm=design-natural允许负值，负值解释为需要cut，不能写成填方超限。
- 该点道路填方上限、实际zone填方容量及剩余/超限量，给出来源。样本单位用整数mm，未知/缺失写null并解释，不用0伪造。
- 样本中的natural min/max若来自完整parcel，明确标记，不能冒充building padded support的min/max。可按需读这几个有限矩形的natural作辅助，但不重写验收器。

确保生产trace中每条拒绝与对应attempt正确关联；不能将只有失败项的数组直接按包含accepted项的数组下标拼接。即使预期接受0，也应断言/处理计数不符。

按state/request分别汇总attempted/accepted/rejected reason+note，再给总计。若重放与G5两态16attempt/[0,0]/firstReason不一致，报告差异与停止，不偷偷改参数“恢复预期”。

## 5. 保留足以复查的计划，不引入实现

在新证据目录保存同两态的新增AB/BC CorridorRequest及生产design/profile（或完整仅内存计划序列化副本）及hash，natural来源与baseline hash一起绑定。不得修改冻结world或旧文件；这些是诊断候选数据，不是发布世界。

main baseline前后bytes/hash必须一致。新的输出create-new，任何失败/超时丢弃副本；仍使用已验收deadline门。

全部baseline realization=0、candidate realization=0；不调用candidate_final_terrain来诊断一个当前纯natural首拒绝，不bake/validate/gate、不改carve、不重跑full final审核。

## 6. 最小成本与白名单

- 自然harness调用≤1；同G5两态恰好一次诊断重放，候选状态≤2，不新取值、不重复重启；总runtime≤120s，编译单列。
- 候选measure_calls≤4、station attempts≤32；append/infer/design/准备调用分别据实记录，内部调用not_instrumented，不混入G5旧预算。
- 只改auto_roads.rs cfg(test)及本轮文档/新证据。优先重用已有函数，不改frontage/frontage_supply/road solver/生产审核或公开接口。
- 0 baseline/candidate realize、0 bake、0 G3/G4旧实验、0 128/clock80/carve、0 CLI世界/gate/release/browser/max/workspace。无Git写操作/agent/F/A变更。

## 7. 交付与下一步判断

新增ds-b01-g6-frontage-rejection-diagnosis/：规则/输入源码hash、唯一run、32条完整生产trace、reason/note统计、请求/profile包、基线前后hash、预算；新b01-g6-frontage-rejection-diagnosis-result.zh-CN.md与receipt，G5只追加勘误。

最后可以根据实际证据提出**一个**下一步最小候选调整建议（例如如果确为no-cut需要什么方向的路面变化），必须写清待验证假设及影响，不在本轮执行。禁止根据总类就提高fill、改平地形、缩小建筑/院落或放松保护；其它尚未触及的后续拒绝不推断成已pass。

doc-tree/diff/hash检查后停止。B仍partial，C不推进，不签收、不生成世界。
