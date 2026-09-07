# r38：面积驱动生态密度、分层实例与缺额闭环

日期：2026-09-07。权威仓库：G:\difyllmwiki\GlyphWeave。

## 1. 本轮必须解决的缺口

r36/r37 输出 densityPerHa，但每个 patch 仍固定生成3或2个实例；参数没有控制实际生成。本轮将密度转成数量预算，让生成、拒绝、独立审计使用同一份预算。不能通过修改参数的显示值、扩大全图绿色面积、缩小植物或删除实例来伪装完成。

## 2. Rust 模块拆分

新增 bevy/crates/core/src/settlement/ecology.rs，承接：

- EcologyClassSummary / EcologyCommunity / EcologyPlant / EcologyRealization；
- EcologyLayerBudget（每层目标、已生成、缺额）；
- EcologyRejection（未实现实例的稳定ID、来源patch、层级、原因）；
- 面积累计取整、整数配额分配、实例生成、结构校验与独立重建比较。

landcover.rs 保留地表mesh生成、地形采样、共用矩形净空、物理面积审计，调用 ecology::realize。既有 landcover::Ecology* 类型路径通过 pub use 保留，但旧序列化文件不保证兼容新合同：必须 fresh bake；本轮明确提升生成器revision到 worldgen-2026-09-07-r38-area-driven-ecology，生态model为 area-budgeted-stratified-vegetation-v3。

## 3. 密度如何真正影响实例数量

当前仍沿用程序化设计参数（不是现实植被调查统计）：

| 类型 | 所有层合计密度（实例/ha） | 分层权重 |
|---|---:|---|
| forest | 420 | canopy 40%、understory 30%、ground 30% |
| wetland | 280 | emergent 70%、understory 30% |
| shrubland | 190 | shrub 60%、ground 40% |

注意：密度单位是每公顷植物代理实例，不是冠幅覆盖率、芦苇茎数或生物量。canopyCover 等旧字段仍是规划先验，不能当成测得的实际覆盖。

### 3.1 同类面积累计取整

按 patch ID 确定性排序，每类独立累计真实三角形的平面面积：

    累计目标 = round(累计面积_m² × densityPerHa / 10000)
    本patch目标 = 当前累计目标 - 前一累计目标

不逐patch盲目向上取整。例如100个256m²森林patch：总目标1075，而不是1100。任意输入patch排列得到相同计划；tiny/partial patch允许目标为0，不以“每层至少一个”虚增密度。

这是**单scene内按类预算**，不是独立chunk随机数预算；全世界多scene/分块重划后的舍入一致性仍需要后续设计和验证。

### 3.2 每层整数配额

对本patch总量按权重分配整数底数，剩余额度按小数余量从大到小分配，相同余量按固定层顺序。每层即使目标为0也保留预算记录。

严格等式：

    patch目标 = sum(各层目标)
    各层目标 = 该层已生成数 + 该层缺额数
    patch目标 = patch已生成数 + patch缺额数

不存在“层数=植物数”的旧限制，一个canopy层可以有多个独立实例。

## 4. 生成准入与失败处理

实例ID由 sourcePatchId + layer + ordinal 组成。使用既有blake3依赖，对scene.seedOffset与实例ID做确定性散列，获得候选位置；最终Y仍从Rust BakedTerrain采样。

先试候选，再按原patch内1m网格有限搜索（至多289个备用格点）。尺寸来自固定层profile，不自动缩小。每个候选必须：

1. 完整矩形占地留在来源patch；
2. 不与结构化水体相交；
3. 不与道路、建筑、围墙、岩石、既有树等硬实体的旋转矩形相交；
4. 不与本patch已经生成的植物占地重叠。

各植物完整占地包含于互不重叠的来源patch，因而不以只检查中心点来规避跨patch重叠。新增跨patch配对测试覆盖了普通相邻patch；大型跨scene拼接仍非本轮证明。

找不到位置时输出 EcologyRejection，目标数量不变，记录缺额。不再因一棵植物放不下而丢弃整个世界，也不把“有拒绝记录”当成已经实现。1m有限搜索不能保证找到连续空间中所有狭小可行位置；当前缺额表示本算法未找到位置，而不是数学上证明无解。

## 5. 审计与质量门禁

- verify_contract 从mesh面积独立重建目标与层配额，不相信记录中的 densityPerHa 或 plannedCount。
- 每个目标ID必须恰好有一个结果：真实植物或拒绝记录；缺失、重复、两边同时出现均拒绝。
- 各层/各patch摘要必须和真实结果计数一致，计数采用checked arithmetic避免伪造溢出。
- 再次用Rust生成器独立重建，逐字段比较所有接受和拒绝结果；伪造“放不下”的原因也不能通过。
- SurfaceAudit新增 ecologyPlannedPlants / ecologyRealizedPlants / ecologyUnfilledPlants。
- 即便自然mesh面积100%实现，只要植物仍有缺额，SurfaceAudit.status保持incomplete，并提供ecology density budget remains unrealized失败原因；已有world-quality-gate据此拒绝，不改变面积分母或既有门槛。

## 6. 测试要求与本轮新增覆盖

1. 100个patch累计目标1075；大小patch预算差异；小patch零配额；输入顺序变化不改计划。
2. 32×32m森林fixture为43个实例；原件和JSON roundtrip验证通过，同seed复现，seedOffset变化保留预算但改变位置。
3. 已生成实例两两不重叠（包括相邻patch）。
4. 全部被岩石阻塞：43个目标全部记录缺额，数量不降低；删除/复制拒绝、伪造摘要、计数溢出均拒绝。
5. 128×128m自然地表已全部实现，但459个植物全被阻塞时，SurfaceAudit必须incomplete。
6. 扩展CLI双份伪造集成测试：场景和发布文件同时伪造仍拒绝；校验平原样例的面积目标1915，不能继续用544个固定实例作为完成证据。

## 7. 本轮边界与来源

未引入任何新的第三方依赖、算法源码或素材，保留项目原有许可证关系；散列使用已有blake3依赖。实例数量分配与领域实现是项目代码。本轮没有接入Mapgen4或symbios-ground侵蚀实现。

仍未完成：物种级生态模型/演替/湿地水位适宜性；将用户气候意图映射为经校验的可配置密度profile；林冠真实覆盖计算；生态资产合同细化；chunk-local实例化与LOD；农村与城市剩余土地；6km×10km时间/峰值内存证明。因此本轮不代表整体路线图完成。

## 8. 实测结果

本轮新输出：G:\difyllmwiki\GlyphWeave\bevy\work\ecology-r38-area-budget-verified。

| 类别 | 面积m² | 面积目标实例 | 实际生成 | 缺额 |
|---|---:|---:|---:|---:|
| forest | 40,960 | 1,720 | 1,720 | 0 |
| wetland | 4,352 | 122 | 122 | 0 |
| shrubland | 3,840 | 73 | 73 | 0 |
| 总计 | 49,152 | **1,915** | **1,915** | **0** |

不是把544改成更大的固定常数：1915来自三类面积乘密度后分别累计取整。Core测试同时证明其它面积的目标变化和被阻塞时真实缺额。

- 与r37逐项比较：55个原实体未改变，SurfacePatch数组未改变；自然高度与最终高度二进制逐字节相同。
- 设施4/4，住宅18/18，29道路专项均保持通过。
- 全Rust workspace最终日志：Core318、App44、CLI单元6、CLI集成18，共386通过（r38-workspace-final-tests.log）。
- Vitest170通过、Node合同27通过、typecheck通过、lint退出0但有既有警告、doc-tree检查通过、git diff --check通过。
- 新CLI的scale-audit为pass，对象规则55/55为pass；quality-report为warn，world-quality-gate实际执行退出1、status=fail。
- 完整gate仍因农村157,504m²、城市9,048m²未实现及相应规划/用途面积项失败，没有靠生态实例数扩大土地面积信用。
- 浏览器逐项核对192个自然patch与1915个实例的ID、来源、坐标和模型尺寸，mismatches=[]、页面errors=[]。
- 预览服务端口8858，来自独立r38-preview-runtime可执行副本，不锁定编译产物。
- 原始证据：输出目录r38-evidence.json、ecology-preview.png.json；外层r38-generation.log、r38-quality.log、r38-gate.log、r38-browser.log。

已实际查看生态近景截图ecology-focused.png。该截图仅在页面初始化前设置相机关注点(448,96,4)，未修改任何生成数据。依旧存在代理植物简陋、旧草渲染过密等视觉问题；不能将合同通过当作最终视觉完成。
