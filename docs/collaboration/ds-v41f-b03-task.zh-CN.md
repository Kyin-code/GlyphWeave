# DS v4.1f B-03：用途分区、地表执行与全域缺额闭合

遵循 G:/difyllmwiki/GlyphWeave/docs/collaboration/ds-v41f-b01-b05-runbook.zh-CN.md。需要B-01地块接口和B-02安全路网接口；若依赖不稳定，只做独立模块小测试并标blocked_dependency，不跑坏世界。

## 1. 目标与口径

A的land-use分配通过，但50,230,050m²尚未实现。developmentAudit的59,578,744m² unplanned是另一层所有权口径，包含其他执行器已经实现的自然地表；必须解释/连接两者，不相加、不把改名当清零。

交付完整的“土地归谁/做什么→所需接入→真实表面/实体→独立面积账本”执行链。优先利用已有development、agriculture、farm_program、farm_access、landcover、ecology，不重建平行账本。

## 2. 残余土地分配合同

在原用途边界内部处理剩余土地，选择明确的子用途与来源：
- 城市：已建地块、实际道路/步行面、公共空间/绿地、明确有需求的后续建筑地块；不能将全部未实现城市刷成草地后说城市需求完成。
- 农村：依据原farm/pasture等需求分出耕地、牧草地、田埂、农路、村落/公共空间；农村不等于每格都放小住宅。
- 自然：已有保护/森林地表继续保留；不从urban/rural偷面积给nature。
- unassigned：保留用户未指定用途的身份；可以明确记录实际原生地表，但不转换为已满足的城市/农业需求，也不扩大unassigned比例。

可新增显式residualLandProgram，必须标版本、scene/source-zone、子用途比例/最低需求和不可行策略。缺省不自动开启新分配。各子用途是对原purpose内部的划分，不改变原全场用途目标；占地竞争顺序要确定性，禁止重叠预算。

每一片残余区域必须可追溯：reserved/allocated/realized/rejected/unresolved。只有核验过的原生地表/SurfacePatch/实体按对应语义获得physical信用；“计划为草地”“候选农田”“未建设储备地”不自动变成人类用途已完成。

## 3. 农业与村落执行

- 复用farm_program的明确fieldTemplate，保留16..128m、4m倍数等现有界限；不要为了填满碎片自动缩小用户指定田块。
- 可以对明确允许多模板的新增B program使用预先列出的离散田块模板，必须记录选择来源；旧单模板请求仍不偷偷替换。
- 多田块共享真实、连通、避障的农路/田埂网络，连接B-02道路或已验证农路；不能给每田块伪造一条穿水/穿楼直线。
- 保留现有FIELD_GRADE/TRACK_GRADE、土方和保护条件；农田没接通不得计为已可用耕地。共享农路几何只计一次，不随田块数重复加面积。
- 建立村落宅基地/公共空间与农田的关系，不用低密城市标签替代农业组织。
- 牧草地必须有符合当前terrain/水体/障碍条件的实际地表与明确用途来源；不外推为作物生长/灌溉模拟。

## 4. 精确所有权与物理覆盖

1. 保留整数半开区域精确分割，source_zone贡献守恒；新增多边形若必要必须验证合法/无自交和几何并集，不引入未经核验的第三方几何库。
2. parcel=building+非建筑余地；道路、地块、农田、公共空间分配互斥，或显式分层且总地面面积不重复。
3. building footprint/yard/道路实际几何不能被区域bbox代理；楼层/楼面面积不能加进土地面积。
4. independent verify从冻结program、道路/地块来源和最终terrain重建，不信任surface.area_m2或发布ledger.status。同步改派生报告也不能绕过源一致性。
5. landUseLedger必须分开purpose计划、实际物理覆盖、requested/target deficit和未实现原因；连接development与landcover的不同口径时列出映射表，不能简单删掉unplanned错误字符串。
6. 性能上避免生成60,000,000个独立JS/Rust对象或把每平方米都重复序列化成巨型mesh。复用分块/共享terrain采样、合理合并同语义表面；压缩表示仍要有Rust权威来源和独立几何重建。不能为压缩削弱高程/保护/边界精度。
7. 若需新表面类别/字段，明示schema版本与旧读兼容。HTML适配仅B-05或必要的小接口接线，不能浏览器自己补缺失地表。

## 5. 白名单（G:/difyllmwiki/GlyphWeave）

生产：
- bevy/crates/core/src/settlement/development.rs、landcover.rs、landuse_ledger.rs、agriculture.rs、farm_access.rs、farm_program.rs、proposals.rs；
- 可新增settlement/residual_land.rs、rural_layout.rs、surface_partition.rs（只在确有边界需要时新建，别为每个函数拆一文件）；settlement.rs仅模块导出；
- worldgen.rs：program接线/来源/版本/产物写出必要点；
- worldgen/road_network.rs和frontage.rs：仅接入B-02/B-01的来源与占地，不重写前序算法。
- bevy/crates/cli/src/main.rs及tests/baked_audit.rs：仅新增字段/报告的独立复算接入和负例，不取消现有失败。

可新增bevy/crates/cli/tests/land_execution.rs、examples/b03-land-execution.manifest.json、docs/collaboration/b03-result.zh-CN.md、本轮work。

不改自然地形、水文、保护语义、实体碰撞谓词、A证据/依赖；生态密度尾差主要在B-05单独处理，不乘机改植物需求。

## 6. 必须验证

- ≤1km控制包含urban/rural/nature/unassigned，保留明确比例，至少存在B-01住宅/店铺、共享农路的田块组和明确公共空间。
- 可行控制中各新增mandatory地块/田块/公共空间均有实际几何与可达性；按实际子用途核对目标、实现和缺额。
- source面积分区精确、物理总和不超scene、共享农路不重复计量；同seed/请求顺序变化稳定。
- 自然源hash、保护区高度、旧固定入口/实体保持；已有田块模板未经授权不缩小。
- 负例：计划农田无地表；地表无合法接入；地块/田块重叠；bbox补算空洞；多田块重复加同一农路；草地冒充耕地；urban改nature或unassigned；跨保护区铺地；篡改用途来源/面积；删除实体仍保留实现数；source与派生报告同时篡改却与输入不符。
- B-03结果必须给每个purpose的planned/realized/unresolved前后表及拒绝原因分组；不得只说“全部测试通过”。

纯fixture优先，定向前缀land_execution。最多3次≤1km生成及gate，按总预算。若大范围物理实现仍未完成而工程/账本诚实稳定，可partial_content进入B-04；最终B签收不能将此缺额自动豁免。
