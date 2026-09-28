# G34-R3：先证明重复正确，再收口裁剪、状态和性能（执行令）

日期：2026-09-28。本文件直接授权下述修复与验收，不需逐次申请运行；G35不启动。G34-R2仍是待修候选，不是已验基线。

## 0. 严格执行与恢复条款

**请严格按任务书完成任务，不得因为与本任务无关的问题擅自中断、停止或跳过任何步骤；不得因为上下文压缩、会话切换、其他任务状态或可修复的驱动问题而擅自停止部分步骤。**
需要压缩上下文时先保存checkpoint：源码/包hash、T编号完成和剩余项、当前失败及诊断、实际调用台账、下一条具体命令；恢复后继续，不能重置预算、删除必测项或拿checkpoint当交付。同因连续两次无新诊断，停原样重试并换最小反例定位，不是停止任务。
真正范围/安全/权限、不可继续的外部依赖或硬资源上限可以阻断，必须给准确partial和最小决策请求。此条不授权越权、伪造结果或无限重试。

## 1. 路径与范围

仓库 G:/difyllmwiki/GlyphWeave/。
新目录 G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g34-r3-repeatability-clip/，create-new。
待修包 G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g34-r2-domain-state-render/cli-runs/r2-c18-export/。
检查反例 G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g34-r2-20260928/（只读）。
回退包 G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g33-r2-minimap-clip-closure/cli-runs/c06-export/。
仅允许修改产品 G:/difyllmwiki/GlyphWeave/bevy/crates/cli/src/g28_viewer3d.js 的G34判定/优化/绘制接线和同目录context_preview3d.rs的G34模板/测试/ID。新ID g34-terrain-visibility-4。
保持model、主图几何/相机/拾取、材质/shader、G32导航、G33 footprint数学及裁剪标志、G30读图语义、core/G23/路径A不变。不接ST18/新世界/工程路线；不Git写操作、不派子代理。合成模型只在新目录scratch使用，不能替换交付model。
源码字节备份、修复合同、visual intent和测量方法写定在前；中文源用明确UTF-8字节安全编辑。

## 2. T0——先堵住旧目录误写与验证假绿

旧模块不能直接作为可写依赖import。先审计本轮所有执行入口及递归依赖的解析后绝对路径，尤其CLI链；共享旧模块只有证明纯读才可引用，否则复制到新目录再修改。
运行前快照所有旧轮/检查目录（按当前现存状态，不假称恢复历史原状）；执行入口统一包装attempt/start/exit、stdout/stderr和唯一invocationId/实际child PID。父进程PID不可冒充每个子进程唯一身份。记录null只能代表未观察，不能计成功/失败。
先在新scratch做旧路径写入的负例，必须在写入前拒绝；核对旧目录零变化。每个生产批次后核验旧目录；若发现新增漂移，立即停止该写入路径、留新目录证据后修入口再继续，绝不能到旧文件加指针注释、补写或“搬回后算未动”。
历史cargo计数是下界+未知，保留原记录；本轮必须从第一个进程完整计账。历史错误不要再拆文档C1，随本轮集中更正。
required表在运行前绑定。missing/fail/fatal/notObserved/重复/未批准N/A强制非零。引入mutation反例：坏union clip、旧stamp重置、unknown16显示136、数值退化全通视；四种均必须被同一正式验收器拒绝。

## 3. T1——修跨帧stamp污染，建立纯结果一致性

先在旧c18/source抽出原函数复现检查方反例：同一墙/同一输入的完整grid连续两次出现hidden1→hidden0，清stamp恢复hidden1。保存红记录，不能只改expected。
选择一种明确正确的生命周期：每grid清空stamp；或index内单调全局epoch并定义整数溢出重置；或独立局部set。不得让不同grid复用同一marker与旧stamp。测试seam不得复用固定marker=1污染主运行。
required：相同状态连续≥50次、A→B→A、surface切换、viewport切换、单格eligible/稀疏eligible、连续两个ray probe再grid、坏包后恢复；模型替换/缓存版本失效在隔离fixture中测。cold/warm每格类别、首交距离和聚合状态一致；性能计数可以变化，正确性不能变。
手推5m墙、0.5m线段后的0.75m墙、epsilon两端、四三角山脊1.25m全保留。必须同时验证底层原语与完整grid，不靠一次场景总hidden计数。

## 4. T2——裁剪测实际像素，不再测声明存在

显示区域必须为cellRect∩footprint∩window。可选：footprint已经正确裁到window时用单一polygon clip；或嵌套两个clip实现交集；或直接几何求交。不得在同一clipPath并列polygon与rect冒充交集。
删除没有合同依据的半格内缩逻辑，不是仅改节点名。clipPathUnits与坐标系、舍入容差明确。
复用检查方三角域及点(69,69)：实际paint在域外必须为零、域内必须保留。三角/斜四边形/六边形/贴窗口边/小于一格/empty全部测；不要求多边形必须>4点。
真实浏览器从实际产品renderer输出DOM复制SVG隔离栅格化（可将被测色层改唯一测试色，但不能改clip结构/points）；在离边界足够远的内外像素做原生像素断言，同时对几何交集。至少一个中心在内、角点在外的格，证明外部像素实际被去掉，而非仅“理论上应裁”。保存SVG、采样坐标/RGBA、正式截图和对照。空域不得因残留rect继续显示。
保留状态框内定位、目标点/北向线可辨、不阻断pointer；默认布局面积和读图栏不降级。

## 5. T3——数值退化和显示数值必须诚实

六类互斥保留。数据缺失与数值退化分开计数，但都属于未完成判定：可设unresolvedCount=unknownData+numericalDegenerate；unknownCount若仍专指数据缺失，必须明确，不再直接用它判完整知识。
knowledgeComplete不能在存在numericalDegenerate时为true。没有一个known visible/hidden样本时不能unobstructed；全未决→unavailable并说明原因；已知+未决→partialKnowledge；hidden+未决可occluded优先，但“至少hidden格/下界”和未决数须可见。outside-domain/outside-view继续不算未知。
原短线段分类器与聚合器分别测试；数值退化fixture标记受控输入，不强称正常相机自然可达。
显示必须与数值一致：旧a12的unknownData16、outsideDomain96、outsideView24不能再显示136未知。正文、数据属性、title/aria和详情用相同语义；独立检查器按磁盘fixture expected解析显示数字，不能只查“部分未知”关键词。
必须观察：outsideOnly、可见+outsideView、可见+16缺失、可见+数值退化、hidden+16缺失、hidden+数值退化、全未知、全数值退化、正常无遮挡、正常遮挡、empty、observer接触/地下、全视锥外。每项精确status/count/complete/visible text。无自然可达路径的场景可隔离页接入实际renderer，但至少保留真实grid产出路径，不能手写状态字符串顶替。
observer按eye处局部插值高度与epsilon判断，不用地形全局峰值。

## 6. T4——性能先证安全，再复测达标

保留已有效的局部优化；可先禁用无法证明安全的fast path定位，不必追求四种优化全部保留。
对高度上界预筛给出“整段覆盖且不漏潜在遮挡”的依据：离散步长小不等于证明；尤其边界入/出、斜擦角、水平/竖直、极短段、陡坡/窄脊、缺数据、起终点epsilon。可用每个穿越格的实际线段参数区间与该格高度上界，而不是只取采样点和下一对角格。
独立无优化全三角oracle与产品差分：确定种子，多高度场/坡面/孔洞/相机，至少10000条有效线段及完整16×16 grid矩阵；记录失败种子和最小输入，不以随机全部通过替代保守性解释。高层cache/去重cold/warm与重复调用均纳入。已修单位规则不变，不把近似march当最终真值。
不得降16×16、放宽epsilon、跳相机状态、漏更新、只测缓存命中。所有正确性闸门全绿后再测20个不同相机、20次相同状态，各状态结果仍逐格一致；每步记录render同步时长、triangle测试、dedupe、fast path、grid/SVG更新和缓存命中（未实现cache就写不适用，不伪造0）。
性能门槛沿用：median≤25ms、p90≤33.3ms，同机同浏览器同计时范围；另报max，不称FPS。修正确性后变慢须优化或准确partial，不沿用R2快但可能漏算的数字。

## 7. T5——最终生产、集成与证据一次收口

最终源码执行cli bin tests、core engineering_保护、正确bin build；export/audit、第二export/audit六文件一致；最终包浏览器。修源码后旧包浏览器结果不能替新包验收。
受保护41函数及G33接受函数按逐函数byte diff，model/shader/材质冻结；不能以源码搜关键字代替。包/source/exe/driver依赖六文件绑定在启动前固定；preview和bindings必传，缺失拒绝，禁止静默默认包。
两尺寸1280×800、960×640，两surface；G32/G33四角/拖出clamp/第二pointer/外来up/cancel/blur、已有选择保持、主图pan/rotate/wheel/entity+terrain真实拾取、模式/层切换、坏包无残留和同页恢复；默认地图面积≥80%/75%，另列扣小地图遮挡面积；几何23814/7938/4096/8022/solid7，console/page/GL=0；摘要/详情和状态条拖拽可用。
正式场景必须包含修正后的裁剪像素、partial数值、重复渲染和unknown/degenerate分支。所有T项以编号映射具体记录，不以“24个大项pass”掩盖大项内部未观测。
旧R2 manifest1002项保持原样；旧台账漂移作为历史已知例外读入，不再修旧文件。报告中删掉与披露矛盾的“所有旧轮从未改动”；只说本轮启动快照到结束是否漂移。
本轮报告/台账/脚本/诊断/截图全部先完成再枚举；仅本目录顶层manifest/receipt/delivery/verifier stdout四个具名控制文件排除。核验工具仅读，前后独立全目录枚举；确认脚本先列入闭集，不封存后补脚本。计数未知与已知分开；字段null不等于记录完整。
交付新报告中对本检查四类产品/显示问题及历史表述逐条回应；已知例外集中登记，不再人为拆小轮次。

## 8. 资源与停止

软计划：cargo实际子进程12、产品CLI16、Chrome真实启动10、driver24、归档128MiB；75%报告用量/诊断/剩余工作；本授权允许合理定位修复继续至硬上限18/24/15/36、192MiB。超过软计划不自动停，不需为每次辅助运行再授权。所有scratch浏览器照实计；只读管理hash不挤产品额度。
硬上限前安排最终整合；到达仍未完成交准确partial，不能先超支后披露。历史用量未知不得估算造证，也不能转入本轮额度抵扣。
完成条件：所有产品正确性、原生渲染、状态显示、性能、冻结与闭集闸门成立，才交delivered_for_inspection；最终由独立检查裁定。G35不启动。
**请严格按任务书完成任务，不得因为其他任务无关问题擅自中断或停止部分步骤；上下文压缩时保存checkpoint并恢复继续，不得擅自裁掉必测项。**