# G34-R4：原子可见性证据、正确边界验收与最终收口（执行令）

日期：2026-09-28。本任务直接授权本范围的诊断、必要产品修复、生产链和浏览器验收；G35不启动。

## 0. 严格完成与恢复条款

**请严格按任务书完成任务，不得因为与其他任务无关的问题擅自中断、停止或跳过任何步骤。不得因为上下文压缩、会话切换、摘要丢失、其他任务状态或可修复的驱动问题而擅自停止部分步骤。**
上下文不足时必须在新目录保存checkpoint，包含源码/包hash、已完成和剩余T项、最后失败、实际账目、下一条命令；恢复后继续，不能删除必测项、重置账目或把checkpoint当交付。相同原因两次失败且无新诊断时停止原样重试，换最小反例定位。只有真实安全/权限/范围/外部依赖或硬上限才可阻断，并交准确partial和最小决策请求。

## 1. 路径与冻结边界

仓库：G:/difyllmwiki/GlyphWeave/。
新目录：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g34-r4-atomic-evidence/，必须create-new。
R3待修包：.../ds-b01-g34-r3-repeatability-clip/cli-runs/r3-c02-export，只读诊断输入。
R3反例与本检查：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g34-r3-20260928/，只读。
仅允许修改产品 `bevy/crates/cli/src/g28_viewer3d.js` 的G34可见性/快照/绘制接线，及 `context_preview3d.rs` 的G34 HTML/CSS/测试/生产者ID；生产ID升为 `g34-terrain-visibility-5`。model、主图相机/拾取/几何、G32导航、G33 footprint数学与裁剪标志、材质/shader、G30读图、core/G23冻结。不得接ST18/新世界/工程route-bake-realize，不Git写，不修改旧证据，不派子代理。源码先做字节备份，中文源码必须显式UTF-8。

## 2. T0：先固定证据模型和执行入口

在任何产品构建前写合同、visual intent、required场景表和测量方法。所有可写模块只能解析到本轮或明确授权产品文件；旧轮 accounting/runner 不得import。写入前路径守卫，入口统一记录 invocationId、真实childPid、attempt/start/end/exit、stdout/stderr；null只能表示未观察。
启动前绑定driver、oracle、package六文件、source hash、build ID。required表唯一；重复ID、缺项、fail、notObserved、fatal、未授权N/A均非零。把R3四种验收错误写成mutations：错误union clip、两尺寸都用0.80、外部model修改冒充坏包、8022字段错配；同一recorder必须拒绝。

## 3. T1：建立同一渲染的不可变VisibilitySnapshot

新增只读快照结构（名称可调整，但字段必须等价），在一次 `drawMinimap`/render 中生成并冻结：

- `snapshotId` 单调；camera完整值、canvas CSS/backing size、DPR、window、footprint points/hash、surface、resolution/lift/epsilon；
- 完整256个cell records（i,j,x,z,category,klass,reason,sMetres/referenceY）；
- 聚合字段 `hidden/visible/unknownData/numericalDegenerate/outsideDomain/outsideView/unknownCount/unresolvedCount/knownCount/status/categorySum`；
- 实际绘制的 `data-cell` 集合与clip points/clip structure，均绑定同一snapshotId。

`__minimapVisibility3d()` 返回该不可变快照副本；不能只返回聚合再另调grid。`__minimapVisibilityGrid3d()`若保留，必须返回新快照或明确标注为独立诊断，不得用于声称“同一帧”。

验收：
- `count(snapshot.cells klass==='hidden') === snapshot.hidden`；同样逐项核对visible/unknown/outside/degenerate与categorySum；
- DOM drawn集合必须等于该snapshot的hidden cell集合；
- a05? exact repeated snapshot digest可不同ID但内容相同；A→B→A内容复现；surface/viewport切换稳定；probe不会污染。
- 同一快照与独立oracle使用同一camera/canvas/footprint输入；差异必须列具体cell、不是只给总数。发现任何自相矛盾立即非零并停止宣称。

## 4. T2：真实裁剪像素闭合

保留R3两层单形状clip，但必须补齐边界负例。使用产品真实renderer输出的clip points与rect，建立至少三角、斜四边形、六边形、贴窗边、小于一格五种隔离栅格；不改clip结构和points，只把唯一测试色注入隔离副本。对每种记录内点/外点原生RGBA，外点必须零，内点必须保留；保存SVG、坐标、RGBA、截图。若真实数据边界没有合适整格，使用独立合成 footprint 仅验证clip renderer，不伪称真实包场景。
删除并验证所有半格内缩；window clip必须是0/0/140/140或合同明确值。clip结构断言之外必须有像素断言。

## 5. T3：修正验收器，清掉三项伪红

- 面积拆为两个唯一场景：1280×800 stage fraction ≥0.80；960×640 stage fraction ≥0.75；另报扣minimap后的交互面积，不将960按0.80判红。
- 坏包测试先确定页面实际读取源。若页面内嵌模型，损坏内嵌模型或HTML/viewer，并断言错误页无旧minimap/旧hidden计数；若产品合同是外部模型优先，才修改外部模型。恢复交付包后断言pageLoads、status、hidden/clip层清空旧状态。
- 几何字段从实际产品/历史冻结基线逐项绑定：terrainIndexCount=23814、terrainTriangleCount=7938、surfaceVertexCount=4096、declaredTriangles=7938、declaredVertices=4096、solidDrawCalls=7；若要保留8022，必须给出字段名、来源和历史证据，否则禁止使用。
- required表一项一ID、一语义；启动前写入intent并由recorder校验唯一性。

## 6. T4：正确性与回归

保留R3 T1/T3通过项并补完整矩阵：重复50次、A→B→A、probe交错、surface/viewport返回、单位7例、山脊1.25m、端点后、epsilon两侧；outsideOnly、unknown、numerical-degenerate、hidden+unknown、empty、unavailable；两尺寸两面、G32/G33四角/拖拽/第二pointer/cancel/blur/选择/主图pan-rotate-wheel-pick、模式/图层、状态条拖拽、坏包恢复、几何和错误底线。
最终整合浏览器必须在最终包上运行；保存camera、canvas/backing、DPR、snapshotId、DOM rect、cell集合、clip像素、console/page/GL和截图。不得用R3结果替代R4包。

## 7. T5：生产、性能和闭集

先做无优化oracle差分，确认epoch/快照/clip修复没有改变逐格分类；再做20不同相机与20相同状态同步测量，记录median/p90/max、grid重算、候选、求交、去重、预筛、SVG更新和缓存命中。不得把重复状态当运动性能，不得改分辨率/epsilon或跳render。性能目标沿用不同相机median≤25ms、p90≤33.3ms；若正确性成立但性能超标，准确partial并列热点。
完成cargo bin tests/build/core保护→export/audit→确定性第二export/audit。manifest在所有脚本/报告/截图完成后生成；verifier只读，核验前后全目录零漂移。精确排除仅本轮manifest/receipt/delivery/verify stdout四项；确认脚本先绑定。旧目录只读，以本轮起始/结束快照证明，不加指针注释。

## 8. 资源与交付

软计划：cargo12、CLI16、Chrome10、driver24、归档128MiB；合理诊断可继续至硬上限cargo18、CLI24、Chrome15、driver36、归档192MiB。达到软计划75%报告用量、剩余T项和下一步；超过软计划不自动停，不需把每次可修夹具失败拆成文档轮次。硬上限仍未完成则交准确partial，不隐藏未观测。
只有所有T项、最终包绑定、原生裁剪像素、原子快照、状态文字、回归、性能和闭集均成立，才能交 `delivered_for_inspection`；最终裁定仍由独立检查，不自宣G34通过，G35不启动。
**请严格按任务书完成任务，不得因为其他任务无关问题擅自中断或停止部分步骤；不得因为上下文压缩造成的懈怠而删除必测项，必须保存checkpoint并恢复继续。**