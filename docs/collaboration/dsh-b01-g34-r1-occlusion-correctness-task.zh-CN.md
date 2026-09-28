# G34-R1：遮挡单位、地表采样语义和证据闭环集中修复（执行令）

日期：2026-09-28。本任务书直接授权本范围实现，不需要另等单次运行批准；覆盖旧G34任务中与本文冲突的“必须y=target.y”及显示状态条款。旧记录不改写。

## 0. 严格执行与恢复条款（必读）

**请严格按任务书完成任务，不得因为与本任务无关的问题擅自中断、停止或跳过任何步骤。上下文压缩、会话切换、摘要丢失、其他任务状态以及可修复的驱动问题，不构成放弃本任务或删除必测项的理由。**
遇上下文不足：在新目录保存checkpoint，写明当前源码/包hash、已完成与剩余编号、实际账目、最后失败、新诊断和下一条具体命令；恢复后从该点继续，不重置预算，不索要已给出的授权。checkpoint不是delivered_for_inspection。
同因两次失败且无新诊断：停止原样重复，换最小解析反例/定位方法继续；不得以此删除山脊、端点后交点、unavailable等必测项。真正越出授权范围、需要修改冻结输入、安全/权限受阻、无法继续的外部依赖或达到明确资源边界时可以停，须给具体证据与最小决策请求；禁止绕过安全或伪造通过。

## 1. 路径与冻结

仓库 G:/difyllmwiki/GlyphWeave/。
新证据目录 G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g34-r1-occlusion-correctness/。
检查反例 G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g34-20260928/。
回退基线 G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g33-r2-minimap-clip-closure/cli-runs/c06-export/。
G34首轮 G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g34-visibility-honesty-impl/ 只读。

允许产品修改仅 G:/difyllmwiki/GlyphWeave/bevy/crates/cli/src/g28_viewer3d.js 内G34模块/绘制接线，及同目录context_preview3d.rs中的G34 HTML/CSS/新ID/测试。新ID g34-terrain-visibility-2。
G33方向、导航、pointer归属、footprint、clipped语义，主图ray/pick，model、几何、材质、shader、reading bar语义、core/G23/路径A冻结。不接ST18/自然生成/工程route-bake-realize/发布，不Git写操作，不派子代理。临时合成模型只作scratch fixture，绝不替换真实交付model。

## 2. 先定产品语义（本轮前瞻授权）

保留terrain-surface方案，但准确限定为“G33采样域内的地表点对当前eye的地形通视采样”，不是“参考平面是否可见”也不是完整地形可见面积。
- G33 footprint不改，只作域限制；sampleY=保存三角插值高度+明确liftM；planeY=target.y仅作域来源。记录referenceMode、footprintPlaneY、sampleSurface、lift/epsilon/resolution与数据来源。
- 每个地表目标先独立检查是否在当前视锥near/far范围内；域内但地表点视锥外=outside-view，不是visible。
- 仅保存地形作为遮挡物；实体/道路不在本判定范围；无窗口外数据不声称真实世界无遮挡。
- observer若位于已知地形下或接触容差带，用unavailable/observer-below-or-on-terrain明确原因；不把这种退化观察者当正常山脊正例。
- 16×16可保留；visible/hidden/undefined由reason分解：outside-domain、outside-view、unknown-data、numerical-degenerate等。outside不是unknown。所有eligible已知且hidden=0才称unobstructed；有unknown则整体unavailable/partialKnowledge明确，绝不全局无遮挡。
- empty仅表示G33域为空；遮挡计算unavailable时隐藏新增层并显示原因，不删除仍有效的G33 footprint。此项覆盖旧visual intent中“unavailable也隐藏整个footprint”的冲突表述。
- 小地图内短状态必须可见：“地表采样”“仅地形”“16×16/分辨率”，可用同一框内title/aria和既有检查面板显示完整限制；不得新增框外浮层。target圆点、窗框/北向线保持可见。
修改前写合同、visual intent和源字节备份，记录本任务批准的语义变化，不追认首轮偏离。

## 3. P0：统一有限线段单位与独立oracle

产品与oracle当前同错；先用检查员解析反例分别证明红，再修。
可选一种且只能一致使用：
A. dir=(target-eye)/L 为单位向量，求交距离s单位米，epsilonM < s < L-epsilonM；
B. d=target-eye 不归一化，参数u无量纲，epsilonM/L < u < 1-epsilonM/L。
不得混用，不得把米再乘L。点在终点之后永不遮挡；origin和target容差前后有明确决定。记录s与u及L供审计。

解析fixture强制项（不调用产品函数生成expected）：
- O=(0,0,0),P=(10,0,0),三角形平面x=5：hidden；
- P=(.5,0,0),平面x=.75：visible；
- 长10m，交点9.998/9.9995，eps=.001：分别hidden/visible；
- 四三角高度场山脊剖面(0,0),(5,4),(10,0)，z跨[-1,1]，O=(0,1,0),P=(10,1,0)：首交1.25m，hidden；
- 将同一山脊移到终点后：visible；再遮挡起点后1m以外的地形正例，防止“只看前1m”；
- 比例缩放、平移、旋转后几何分类保持（epsilon随声明规则），单位/坐标与参数不混；非有限/退化/null明确。

高度场第二算法要独立，例如按穿过的网格格点区间/三角线性方程解析检查，不靠固定96步保证无漏。若保留采样march，仅作近似诊断而非权威expected，差异不得静默忽略。独立oracle先对解析fixture全绿，才能评价产品；两者同错不能靠真实数据扫描次数背书。
合成山脊、目标后交点、epsilon两侧必须在完整grid路径也通过，不只底层函数；禁止以真实“无遮挡相机”替代。不需要修改生产模型或关闭验证。

## 4. P1：绘制、无定义与域边界

hidden格按中心采样并记录该语义，但整个显示单元必须裁剪到footprint∩window（实际SVG clipPath或几何裁剪），不得涂到域外。比对格ID集合、每格(i,j,x,z,class,reason)、SVG坐标/尺寸/裁剪多边形；不能只比hidden计数。
部分未知必须有可见状态/原因，不能把undefined误称unobstructed；起点在地下、缺surface、非有限三角、无效窗口全部有确定状态。
缓存须按模型/surface版本正确失效，坏包和恢复不得留下旧层/旧计数。无G34能力数据的valid G33输入，仍保留G33底图与交互。

## 5. 验证器与真实浏览器

独立required列表把empty/unavailable分开；每项具名精确匹配；missing/fail/fatal/notObserved/重复/未批准N/A强制非零。去掉“无hit就直接true”等空证明。
每个run create-new、driver与依赖hash、argv/cwd/package six-file绑定先落盘；包路径必填，禁止默认回退。ledger不事后造时间/退出码，attempt/start/exit唯一processId写入。

最终集成Chrome至少：
1. 两尺寸和两面：真实相机完整快照target/yaw/pitch/distance、DOM canvasRect和绘制canvas尺寸，区分CSS/backing；渲染触发不得改变目标spec。若用受控状态设置须明确标注、assert实际==请求；小地图点击只作为交互测试，不拿它触发固定相机实验。
2. 已校准合成fixture正负例在scratch隔离展示；真实包也验证至少无遮挡和可达到的可靠遮挡场景。若真实数据无可靠正例如实记录，由合成几何作正例，不制造噪声正例。
3. 实际SVG格ID/坐标/裁剪与独立oracle逐格一致；空/unavailable/部分未知均单列。unavailable可通过隔离测试页受控传入缺数据的显示输入，用实际产品renderer证明UI状态，不能当成真实合法包自然可达；不准绕过坏包验证、污染交付包。
4. 合成山脊/视锥边界/footprint边界有截图。至少一个格中心在内而角点在外，确认色层裁剪；无hidden时子元素清空；target永远可见；UI文字范围准确。
5. G33方向/导航/clamp/第二pointer/cancel/blur、已有entity选择、小地图外pan/rotate/wheel/entity+terrain pick、模式/图层/final-natural、坏包无残留及同页恢复、几何和错误底线。
6. 性能：固定20次相机交互，记录包含新增计算的render耗时中位/p90/max、计算次数、缓存命中、SVG单元数；与同机G33基线比较，不称FPS，不偷跳帧数据冒充同步。主要结果由真实UI而非只有state判定。
7. 各状态截图/元数据真实落盘；不要求空白/不可用截图强行两两不同，相同有合理状态解释即可。真实鼠标、受控事件、离线fixture三栏计数。

## 6. 最终生产链与范围核验

cli bin tests → core engineering_保护 → 正确bin build → export/audit → deterministic export/audit → 新包浏览器验证。若候选再改，最终接受证据必须绑定更新后的包，不靠相同buildId猜一致。
41保护函数及G33新增已接受函数按白名单diff，不能仅搜字符串。原模型hash必须相同。
manifest最终按精确路径枚举（包含诊断/保留scratch及嵌套manifest）；报告/ledger等先完成。允许排除manifest/receipt/delivery/verify stdout；verifier仅读，检查前后独立枚举。旧轮按逐文件sha而不是只比文件数量；历史已知缺口单列。

## 7. 资源与完成纪律

新轮软计划：cargo实际命令10、CLI12、Chrome6、验证driver16、归档64MiB。75%写进度和余量；允许在当前授权修复链内说明调整，不因一次夹具失败再申请C1/C2。外部费用/发布等无授权；到150%仍无法闭合且无可执行收口计划才交准确partial/请求决策，不掩盖失败。
管理读/hash不占产品预算；验证脚本和Chrome scratch启动都算实际运行。先定位再重跑，不重复G33整条历史生产链。资金便宜不改变真实性、范围和质量要求。

完成条件：上述逐项检查、产品/独立oracle/实际绘制一致，旧输入不改、最终证据绑定、所有必需项有观察；仅交delivered_for_inspection。未达到时明确partial，不得“完整完成但必测项放弃”。
**请严格按任务书完成任务，不得因为其他任务无关问题擅自中断或停止部分步骤；如确需中断，必须保存可恢复checkpoint并列出未完成项，恢复后继续，不能自作主张裁掉步骤。** G35不启动。