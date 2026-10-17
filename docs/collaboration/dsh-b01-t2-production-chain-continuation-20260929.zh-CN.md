# DSH续行任务：B01-T2 机械同步、拒绝留存与真实基础道路合同

日期：2026-09-29。状态：实现与离线验证授权，T4 世界运行未授权。

本文件补充而非回写旧 OPT-B 任务书；与旧任务书冲突时，以本文件明确修订的范围、验证顺序和停止规则为准。不切 OPT-A，不重写生成器，不启动 FUT/渲染/GIS。

## 0. 执行纪律与本轮判定

**请严格按任务书完成任务，不得因为其他任务无关问题擅自中断或停止部分步骤；上下文压缩、会话切换和可修复的脚本问题不是跳过验证的理由。需要中断时先保存 checkpoint，恢复后继续。**

真正范围扩张、硬合同冲突和证据污染必须报告；不得无新诊断反复运行。范围内的编译错误、测试红项是待修工作，不是自动结束本轮的理由。

当前前轮交付为 partial / implementation-present-unvalidated。不能再说“T2生产已完成，仅缺机械同步”。只批准以下范围，不执行 Git 写、切换、回退、stash、提交或推送；不改 F:/modenmapbox，不回写历史报告/台账/manifest，不调用子代理。

仓库：G:/difyllmwiki/GlyphWeave/
新证据目录：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-t2-production-chain-continuation-20260929/
检查依据：G:/difyllmwiki/GlyphWeave/docs/collaboration/b01-t2-opt-b-independent-review-20260929.zh-CN.md

## 1. 上轮未做好清单（须逐项回应，不归因于动机）

1. 声称28处/8文件机械缺字段，检查员当前编译实际仅5处/4文件；需用原始诊断更正，不能估算补造。
2. helper虽收集拒绝，生产调用丢弃返回Vec；“拒绝已保留”不成立。
3. 未发现 auto_roads::base_only_plan 和 trusted_base_contract 的“base等于无profile”旧假设，基础道路升级可能反而被供给流程排除。
4. 没有对实际 entities() 产出的实体执行 validate_entity；部分手工比较不等于完整合同。
5. 高程边界/溢出与NaN朝向处理不充分。
6. 没有最小真实消费fixture就先宣称生产完成；注释 locked by test 尚无测试支持。
7. 库编译结果与6次cargo缺乏本目录原始日志；可找回则引用，找不回标“历史声明、未独立核验”，不得补造旧日志。

检查员同时承担任务边界不够清晰的责任：公共字段所需测试字面量同步不是广泛结构重写；未编辑文件现在备份/登记就是有效pre-edit，不是回溯补写。

## 2. C0 新基线与精确写集

编辑前读取检查证据，复核当前源码hash。若执行者或其他进程已改动，记录差异后续接，不覆盖别人的工作。

新一轮逐文件备份和hash，记录mechanical-test-sync、semantic、test-only。已改的 road_network/road_extensions 以本轮起点记录hash，链接原轮pre-edit，不将现在hash伪装成首次修改前hash。旧证据只读，旧数量的更正在新report中写。

以下路径均位于 G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/：

### 2.1 明确授权的机械同步

- settlement/agriculture.rs:661 一处。
- settlement/development.rs:930、941 两处。
- settlement/landcover.rs:1013 一处。
- worldgen/road_parcel_feasibility.rs:538 一处。

仅在上述旧RoadEdge字面量补 profile: None，不能改期望或删除测试。先按当前编译器重定位行号；出现更多同类字面量，可在编辑前记录“文件/行/诊断/pre-edit备份”，仅作同样机械补字段，不必为此再请求一轮授权。不得改其他同名/不同类型结构；不得机械全库替换profile字段。

### 2.2 明确授权的生产修复与相关测试

- worldgen/road_network.rs：拒绝处理、实际实体验证、数值边界、base身份/调用次序及fixture。
- worldgen/road_extensions.rs：保持扩展来源唯一、必要一致性检查/测试。
- worldgen/auto_roads.rs：仅base提取/供给接线/相关回归，不改搜索预算和算法目标。
- worldgen/road_parcel_feasibility.rs：仅真实基础profile与固定节点合同的兼容和不可变审核/测试。

允许在 RoadNetworkPlan 增加一个serde向后兼容的具名拒绝诊断字段（例如 baseProfileRejections，含edge ID、类别、原因），这只是持久诊断，不是OPT-A profile注册表。添加该字段带来的计划字面量机械初始化也适用上述机械同步授权。

worldgen/frontage.rs、frontage_supply.rs、engineering_connections.rs、settlement/development.rs、agriculture.rs、landcover.rs 允许对应新增test-only消费/回归测试与机械同步；禁止借此改生产放置、质量判据或工程语义。road_alignment.rs、road_profile.rs验证核心保持只读；不改zoning/reserved/保护/水域/用户需求/退界政策/reachDistance/ratio/尺寸/坡度/填方上限。

## 3. C1 编译恢复与证据更正

补5处字段后先完成一次测试编译并保存stdout/stderr/exit。报告按编译器实数，不把warning算error；不为凑旧28处修改不存在的站点。

原始出错记录与修复后记录都保留。修改已存在，新的mutation只能叫“回归敏感性验证”，不得伪称已经在旧实现之前写了红测。

## 4. C2 修复拒绝丢失与实体检查

1. 生产层消费 attach_base_edge_profiles 的全部拒绝，持久化到权威计划并能由报告/审计读取，不仅stderr或临时Vec。
2. 区分合法legacy未尝试（如既有无zones输入）、可解释的单边不可设计、结构损坏/实体不一致。结构损坏返回错误；单边拒绝可保留原合法基础道路但不得提供frontage资格，必须具名诊断。道路连通状态和profile供给状态不能混为“全通过”。
3. 在进入实际frontage消费前，对每个新profile对应的生产 entities() 实体调用 validate_entity。不得复制一套伪Entity构造冒充真实消费者。
4. checked_mul和明确 -32000..=256000 mm 域；检查极端i32、负高程、NaN/Infinity及合法0/90几何方向。不允许溢出panic、饱和截断伪造有效高程、放宽epsilon。
5. 调用返回错误时不得遗留半更新公开plan或丢失nodes。优先清晰借用/暂存后提交；mem::take不是正确性证据。
6. 增加混合成功/拒绝边的生产级离线fixture，保存/读回计划后拒绝仍在，成功边可消费，拒绝边不能产生station。

## 5. C3 消除base身份与profile存在性的混淆

### 必须先复现的反例

基础边全部成功带profile时，现有 base_only_plan 会清空base；即便修筛选，trusted_base_contract仍拒绝profile。用真实生产入口的离线小场景证明，不消耗512m/1km世界额度。

### 允许修复的语义

- 基础/扩展身份按规划来源与extensions成员关系校验（ID唯一、边节点/拓扑一致），不再以有无profile判身份。不要靠未经验证的字符串前缀就授予“可信”。
- TrustedBaseContract 必须从当前真实、已校验基础计划建立，保存节点高程约束和基础边几何/权威profile的不可变身份；候选不能给自己另造基线。
- 有profile基础路允许沿线起伏，但固定节点/接入平台、端点、高程域、几何、自然来源和地形支撑仍须通过原强度合同；沿线profile不能在候选里静默变更。
- 禁止为了兼容先把profile清空或flatten；禁止复制一个与真实世界不符的“未分级基线”；禁止只删除 trusted_base_contract 的拒绝条件而不补等效验证。
- 保持旧无profile/等高基础计划可读可用、扩展路唯一设计来源、两者混合无重复边。
- 受检读取路径包括 auto_roads supply、road_parcel_feasibility节点审计、road_network测试helper及按基础道路分类的所有受影响消费者；先搜出清单再修改。

### 配套地形/站点检查

旧roadside_sites按level_m选支撑，新solver的中间段可能低于level_m；禁止假定“端点同高就全线同高”。用非平坦毫米地形生成实际carves/final terrain，验证原有站点、入口、共享节点、道路平台与profile相容。新增profile若使既有正确站点退化，不能放宽支撑或把站点删掉：在授权文件内修调用顺序/基线衔接，若确需改变站点或地形工程算法，准确报告该具体越界点，不试世界掩盖它。

## 6. C4 必测清单（离线真实代码路径）

- 合法64m基础边、非零origin、真实毫米terrain和zones，至少X/Z两轴。
- profile进入生产entities、shape/entity校验、stations非空、至少一个实际candidate/site通过，非只数profile字段。
- 全profile基础域、部分profile+拒绝、legacy无profile、base+extension混合，均不丢基础边、不重复实体。
- trusted基线对篡改profile内部样本、端点/节点高程、几何、自然hash、缺边拒绝；mutation确实使验证失败。
- 共享节点与既有站点保持；非平坦final terrain与profile/入口/支撑一致。
- 具名拒绝保存再读取后仍可见，拒绝不得给frontage/realized贡献。
- 六类原合同负例：节点/scene、轴/长度、工程参数、缺样/编码、不可行工程约束、entity不一致；明确各例是helper还是下层solver测试，不声称固定常量参数已被高层实际注入。
- checked整数、高程上下边界、NaN/Infinity；无panic、无静默截断。
- 旧JSON缺字段兼容；新JSON round-trip；相同输入重复运行得到稳定profile/拒绝顺序/ID（离线，不扫世界）。
- T1四朝向退界回归。

## 7. C5 T3门与后续运行边界

保存定向 road_profile/alignment/network/extensions/auto_roads/frontage/frontage_supply/parcel_geometry/services/development/landcover/agriculture/landuse_ledger/zoning 测试原始日志，失败先分类诊断。既有失败不能凭口头称“基线”；如有可靠的前后备份可作隔离对照，不能checkout/reset当前树。

离线必需项全绿后构建CLI，绑定新exe和源码hash。此时不运行旧世界quality来冒充新生产结果，也不要求未生成的世界提前有quality status。旧任务书“所有JSON status=pass”收窄为：每个实际运行的检查按其schema成功字段及required项判定；不存在该字段的输入JSON不参与该断言。

世界级scale-audit和quality-report留给T4，必须同时检查exit和JSON status。T4仍需另行明确授权，当前512m/1km/6×10km生成均0次；1km保留额度未被本文件释放。

## 8. 资源与停止规则（解除机械性反复停工）

本轮cargo调用采用软计划12次；达到时汇报已用量和剩余诊断，有新证据且仍在写集内可继续，不为便宜离线调用机械停工。每次按真实命令、PID、开始/结束/exit单独记账；短fixture设合理超时，大套件按实际规模设超时并保留进度。禁止无诊断原样重试。

真正硬边界：世界生成/产品CLI生成与审计/浏览器为0；不改用户需求和受保护语义，不污染旧证据，不无授权改工程算法。范围内机械同步和新测试编译红项本身不是硬停止。不要向用户再次询问同一已授权事项。

## 9. 交付与下一步

新目录交：report、原始日志、run/process账目、基线和唯一写集、源码差异、拒绝持久化fixture、trusted基线与篡改反例、最终test summary、CLI构建绑定、checkpoint。沿用轻量只读核验，不新增多轮文书闸门。

每项注明 static/compile/fixture/production-world 的证据层级。本轮没有世界测量，不能宣称城市产能增长、B01全通过或B/C通过。

完成条件：机械编译障碍关闭，C2/C3生产合同修复，C4/T3通过，交 delivered_for_inspection；否则准确partial并给出一个具名阻塞及最小测量。后续检查员再决定1km真实验证；先看profile→station→site→building→ledger业务增量，再排B02～B05，不自动转人口/GIS等新体系。
