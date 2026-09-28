# V4.1 下一任务：B04-C1-R2 小收口 + B02-W1 宽路施工

## 1. 基线与工作范围

仅工作于 `G:/difyllmwiki/GlyphWeave`。先读 `docs/collaboration/b04-c1-r1-inspection-20260917.zh-CN.md`。R1的T1/T3/T4/T5（8/12m误报修复）保留，不重做。记录HEAD及开始时相关源码hash，保留所有旧证据和dirty改动。

不启动agent，不Git暂存/提交/推送/切分支/重置/清理，不修改F盘/A冻结材料。R6模块只读，不重新打开joint加固任务。不要跑原自然四档扫描，不强求其24/24，不推进C、不自行签收B。

本轮两个交付分开报告：R2修编译器剩余语义；W1使已宣称支持的16m道路在生产施工上成立。允许W1在**新控制产物**上改变必要的工程final，禁止改自然输入或回写旧世界。不是禁止修生产施工，也不是授权重新设计自然地形。

## 2. B04-C1-R2：每个请求用实际院落推导 footprint

### 要做什么

当前编译器在requests循环外用profile院落算footprint，忽略了用户显式院落对缺省建筑尺寸的影响。按请求先确定并严格解析生效parcelInsets，再逐轴计算：

- 缺省 width = 确定性 profile完整地块宽 − 2×生效side。
- 缺省 depth = 确定性 profile完整地块深 − 生效front − 生效rear。
- 显式width/depth分别保留；只有缺省轴才推导，checked、正偶数、consumer/asset合同照旧。
- 显式insets不缩小；不合法组合报错，不能回退旧profile footprint来凑可用性。
- 来源表记录依赖的profile完整地块与用户/默认院落；显式footprint导致偏离要如实记录，notes不能误称用户院落为profile院落。

### 验收用例

直接使用检查员 `bevy/work/collaboration/inspector-b04-c1-r1-20260917/inset-probe.rs` 的逻辑，不覆盖检查员文件：

1. low-density + storefront +缺尺寸+默认院落：10×16建筑，26×50完整地块，保持。
2. 同输入但显式front=8/side=2/rear=26：**22×16建筑，26×50地块**，真实boundary确认，不是14×50。
3. 显式2/2/2：推导22×46，storefront深度合同应拒绝；不能接受旧10×16。
4. 分别仅显式width、仅显式depth、全显式footprint、显式insets+部分缺省，验证逐轴优先级与来源。
5. 四朝向完整地块宽深关系成立；未知字段/错类型仍拒绝；有效文档重编译篡改仍拒绝。

更新编译语义revision并说明影响，不覆盖旧v1/v2文档。不要调整六profile统计范围或放宽asset/consumer来让矩阵变绿。既有几何默认不可用的情况可以继续明确拒绝。

完成该项即可单独交付R2结果，不受W1是否最终可行影响。

## 3. B02-W1：16m graded collector完整路面施工支持

### 设计原则

先列清四个范围：实际路面、整平带、工程支撑/shoulder、过渡带；说明宽度来自哪个权威合同、需要精确设计高程还是只限fill/cut。不要把所有范围再混成同一矩形，也不要无论地形都把运输足迹当平顶。

- 当前8/12m生产整平半宽为7m，16m至少要覆盖8m半路面。
- 选择最小、合同一致的宽度规则；`max(7, width/2+1)`或其他方案只是候选，不能无论shoulder语义直接套公式。
- 优先保留既有8/12m数值/行为；新宽路的必要final变化必须来源于生产carve，天然raster、seed、需求和fill/坡度上限不变。
- corridor设计/候选接受、最终carve、独立audit共用一致几何含义。不能计划阶段宣称合法、最终才发现路面不受支撑而把它留在已实现清单；真实超fill等不合法候选仍拒绝。
- 结构性coverage错误每条路最多一条，不在逐采样循环中刷同一失败；失败路不计入realizedEdges。

### 必须的验证

1. 8/12/16m，各覆盖两个正交方向；至少16m正例必须非平自然地形，并用生产plan/carve生成final，再生产verify。不能只测helper返回数值或全平地。
2. 同自然输入真实生成16m collector +12m local连接，固定接点高程不变，连接成立，完整路面受支撑，roadAudit pass。只证明道路能力，不冒称24建筑完成。
3. 单变量负例区分：实际路面边缘±1mm、整平外缘±1mm（仅当其应精确整平）、超过fill上限、未经授权cut、固定接点不一致。过渡带只按其真实合同判断，不能人为要求完全等于设计高程。
4. 构造一次窄band/无覆盖反例，拒绝且每条路错误数有界，realized不计入失败road；不要故意保留错误helper让正常16m永远失败。
5. 8/12m现有控制回归通过；用只读或新副本确认不写旧世界。W1若未改既有8/12施工值，给可核对的比较；若确需改，说明来源与版本，不宣称字节不变。
6. 纠正旧结果中“任何合法graded路都不能通过”、6m点称7m边缘、未独立证明的overfill覆盖措辞。历史证据不重写，追加勘误。

## 4. 范围与预算

允许修改：
- `bevy/crates/core/src/settlement/morphology_compile.rs` 及就地测试。
- `bevy/crates/core/src/worldgen.rs` 仅graded几何/carve和必要版本/测试。
- `bevy/crates/core/src/worldgen/road_network.rs` 审核范围/计数及定向测试。
- `bevy/crates/core/src/worldgen/road_extensions.rs`、`road_profile.rs`、`auto_roads.rs` 仅根因确需的宽路几何/候选接线，不重写搜索架构。
- `bevy/crates/cli/tests/morphology_compile.rs` 若revision/自给fixture需同步。
- 必要时新建 `bevy/crates/cli/tests/graded_road_width.rs` 的小型自给集成。
- 新结果 `docs/collaboration/b04-c1-r2-b02-w1-result.zh-CN.md`；R1结果只能追加勘误。
- 新证据 `bevy/work/collaboration/ds-b04-c1-r2-b02-w1/`，create-new。

其余R6 joint、资产、依赖/lock、自然生成算法、渲染只读。需要白名单外变更时说明阻断和最小提议，不自行展开。

先跑定向编译和小函数集成；单项120s，首次必要编译300s，不跑全core/workspace/CLI全量、release、最大图或浏览器。不要为了证明修复重复大量相同世界。

W1小集成通过后，最多**一次**新的512m CLI控制及独立quality/gate（生成120s/4096MiB）。沿用原C1两控制之一的自然参数/seed/需求，必要时用已支持的显式道路程序表达collector/local，记录输入增量，不冒充同输入比较。若CLI表达仍阻塞，提交函数级生产见证和具体接口阻断，不为用完次数盲跑。测试内部bake与命令行新图分别披露次数。

## 5. 交付及验收口径

分别给出R2和W1状态，不能一个通过就都写完成。交付源码hash、实际命令/exit/log、反例前后结果、effective来源表、8/12/16宽度/朝向/支持带矩阵、collector+local生产见证、旧证据完整性和remaining项。

frontage0/24、剩余六项质量缺额、原自然unresolved仍独立列出；本轮不要求解决它们，也不允许因此重新硬跑同一困难案例。下一阶段再处理小图道路-地块真实供给和内容缺额。

跑doc-tree:check、git diff --check。不自行签收B，不提交Git，不启动agent，交付后停止等检查员。
