# G34-R2：状态、域裁剪、框内文字与可交互性能集中收口（执行令）

日期：2026-09-28。本文件授权本范围实现、测试、生产与浏览器验收；无需逐次另申请。G35不启动。

## 0. 严格执行、恢复与边界

**请严格按任务书完成任务，不得因为与本任务无关的问题擅自中断、停止或跳过任何步骤；不得因为上下文压缩、会话切换、其他任务状态或可修复的驱动问题而擅自停止部分步骤。**
上下文不足时保存checkpoint（输入hash、剩余编号、实际调用台账、最后失败、诊断、下一条命令），恢复后继续；checkpoint不是交付。同因两次失败无新诊断，应换最小反例/定位方法，不能删必测场景。安全/权限、真实外部依赖、越出本授权范围或明确资源上限才可交阻断，必须列证据和最小决策请求；本条不允许越权、造假、掩盖失败或无限原样重试。

## 1. 路径与冻结

仓库：G:/difyllmwiki/GlyphWeave/。
新证据目录：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g34-r2-domain-state-render/，create-new。
当前待修包：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g34-r1-occlusion-correctness/cli-runs/r1-c08-export/，只读诊断基线，不是已验发布版。
回退基线：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g33-r2-minimap-clip-closure/cli-runs/c06-export/。
反例：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g34-r1-20260928/，只读。
允许产品修改：G:/difyllmwiki/GlyphWeave/bevy/crates/cli/src/g28_viewer3d.js 内G34状态/域绘制/性能模块；G:/difyllmwiki/GlyphWeave/bevy/crates/cli/src/context_preview3d.rs 内G34 HTML/CSS/测试/生产者ID。新ID g34-terrain-visibility-3。
冻结model、G33 footprint数学/裁剪标志/导航和pointer所有权、主图拾取与相机、几何、材质/shader、G30读图语义、core/G23/路径A。不得接ST18/新世界/工程route-bake-realize；不Git写操作、不派子代理、不修改任一旧证据/检查目录。
先字节备份两源码、合同与visual intent，再实现。UTF-8明确编码，不用默认编码读写中文源码。

## 2. T1——状态合同与非空验收先修

保留已修好的米单位和terrain-surface+lift语义；先跑七例单位、四三角山脊首交1.25m、终点后交点、epsilon两侧作为防退化。
先定义互斥类别和计数恒等式：outside-domain、outside-view、unknown-data、numerical-degenerate、visible、hidden之和=256。outside两类不是未知；另设unknownCount或等价明确量，不用undefined总和替代unknown。
- 判断范围内有已知样本、全部已知且hidden=0：unobstructed；文案仅“有效采样域内通视”。域外数量不使其变为部分未知。
- 真正未知>0：knowledgeComplete=false；hidden=0且有已知样本时为partialKnowledge。hidden>0可保留occluded优先，但可见文字必须同时表明尚有未知，不能掩盖不完整性。
- 无eligible/全视锥外、全未知、观察者接触或低于当地地形、空域分别明确unavailable或empty及原因。observer依据eye位置的局部三角插值高度与epsilon，不拿全局峰值判断。
- 存在unknown且无法判定其是否在视锥内时，不将其偷偷计为outside-view；在合同说明保守分类顺序。
修独立oracle的同类语义，expected不得调用产品生成。
至少具名fixture：128可见+128域外、可见+视锥外、可见+真正缺失、可见+数值退化、hidden+未知、全已知无遮挡、正常hill遮挡、全未知、empty、observer退化。每例指定精确status/计数/knowledgeComplete/可见文案。
删除现有“若status是partial才检查”的空条件。缺实际partialKnowledge场景必须失败。增加mutation自检：把actual.status换成occluded、unknownCount换0、删场景、重复id均应非零。

## 3. T2——真实footprint裁剪与框内状态

显示单元最终区域=cellRect∩footprint∩window。可用正确SVG多边形clipPath或几何裁剪；不得用内缩矩形代替，不得为过测试改变footprint。声明clipPathUnits、坐标映射与舍入容差。
离线重放检查方三角footprint：中心(65.625,65.625)在内、(69,69)在外；后者必须不再被绘制。再测旋转四边形、贴窗边、小于一格、空域。核对实际SVG rect与clip points的交集，不能只查url/元素存在；同时证明合法边缘色块未被无理由截去。
真实浏览器必须含非矩形footprint边界格正反点、至少一张可辨认的边界截图；格ID、坐标、尺寸、最终剪裁区域逐项对oracle。实际产品renderer必须参与，不可只画检查员自己的图。
短状态放回140×140小地图框内，不得横跨画布底边。框内显示地表采样、仅地形、16×16和简短状态；长原因进现有检查面板/title/aria，不能以title替代短状态。不新增画布横条、不改默认地图面积。文字可读且不裁字，目标点/北向线/footprint仍可辨，文本不拦截拖拽。两尺寸各状态测statusRect在minimapRect内、实际命中/层级与截图；状态字符串正确不等于布局正确。

## 4. T3——性能工程（不降低正确性）

先在新目录保留R1可复现基线，固定同机同浏览器、相同真实鼠标交互路径/场景相机、同DPR/视口、同一计时边界；若G33不能同样触发，就只作指示引用，不强拼倍率。
记录包含通视计算和SVG更新的同步耗时median/p90/max、每次grid重算数、候选/实际三角求交数、去重数量、缓存命中、SVG更新数。20次不同相机和20次相同状态分别测，不以相同状态缓存冒充运动性能。
允许G34内部候选三角去重、正确空间遍历、局部计算/DOM缓存；不修改主图渲染/拾取。cache key必须覆盖model版本、surface、相机、canvas纵横比、窗口/domain、resolution/lift/epsilon；切面、resize、坏包恢复全部测失效。
不得降低16×16、放宽epsilon、跳过相机状态、异步漏更新、只计缓存命中或把同步耗时叫FPS。优化前后逐格结果和状态必须完全一致。
本轮前瞻目标：同机20次不同相机同步p90≤33.3ms、median≤25ms，另报最坏值；测量方法在优化前封存。达不到则报告精确热点/残余风险，仍完成所有其他T项；不得自行放宽数字或称全面完成。此门槛不追溯用于改判旧轮。

## 5. T4——真实验收、生产链与冻结保护

required场景表和expected在启动前固定，缺项/fail/fatal/notObserved/重复/未批准N/A强制非零。每run create-new；必传preview/bindings，不准回退旧包；启动前绑定驱动及模块、exe/source、六文件。
离线与browser均覆盖T1/T2，partial未知可在隔离页给实际renderer传受控数据；明确区分“真实合法包自然可达”与“隔离受控状态”。至少一个完整grid的真实unknown fixture，不只手工改状态文本。不得绕过交付包坏包校验。
最终包两尺寸1280×800、960×640，两surface；正常hill hidden、真实数据无遮挡、partial、empty、unavailable、hidden+unknown、clip边界。保存实际camera==请求、DOM矩形、画布尺寸、格ID/类别、文字、SVG裁剪、截图。合成输入永不替换交付model。
G33/G32回归：四角/拖出clamp、第二pointer/外来up/cancel/blur、既有选择保持；主图pan/rotate/wheel、entity+terrain真实拾取；模式往返/层开关/切面；坏包无旧层残留及同页恢复；几何23814/7938/4096/8022/solid7，console/page/GL=0。
默认布局面积≥0.80/0.75，另列小地图和其他实际遮挡扣除后的可交互面积，不以canvas矩形当无遮盖。摘要/详情可达，不靠压小旧文字牺牲可读性。
最终源码：cli bin test + core engineering_保护 +正确bin build → export/audit →第二export/audit六文件一致 →最终包真实浏览器。源码再变则重建重绑必要证据，不能拿前一包浏览器封新包。41函数及G33已接受函数、model、shader/材质按实际diff证明冻结。

## 6. T5——证据和账务一次收口

修复核验器错误路径，用实际存在的两份旧manifest（16项/39项）；继承的G33-R2 verify日志漂移用已封存C1快照hash证明，不只mtime。没有可信先前hash的条目明确unknown，不猜修改者。
R1保持原样：663已列匹配+confirm-delivery.mjs漏列作为既有例外；新轮绑定实际旧目录快照，不删除漏列、不补写旧manifest。首轮台账误写及指针注释同样登记，不能宣称旧输入全部零漂移。
唯一追加台账按绝对路径写新目录，启动前验证路径在本轮内、拒绝旧轮根；processId唯一，attempt/start/exit分开。cargo执行test/build按真实子进程分别计，不用一个shell链掩盖次数；所有诊断/scratch Chrome计真实启动。管理读取/hash另列。
历史cargo：可证明3、执行者披露6、明细不可恢复处分别记录，不伪造stdout/时间。历史CLI16/12、Chrome12/6保留，不追溯抵扣。
报告/台账/驱动/诊断脚本先完成再枚举；精确排除manifest、receipt、delivery、核验stdout四类本目录具名文件，禁止泛路径排除。核验器只读；以实际磁盘文件集核对，最后一次确认脚本也必须提前列入。核验后全目录二次枚举；受绑定物变更就重新封存本轮，不能事后悄加文件。
交付逐T编号给证据、真实缺口、语义变化、性能结果、账目和例外；报告不得称“全部完成”而某必测只有条件蕴含。

## 7. 资源和完成条件

资源不是本轮主要约束，不再采用“仅一次aux/Chrome”的脆弱配额。
软计划：cargo实际子进程12、产品CLI16、Chrome真实启动10、验证driver24，归档128MiB。75%写进checkpoint和进度；合理诊断修复可在本授权内继续至150%（18/24/15/36，归档192MiB），不得把软计划说成必须停工。接近上限时先集中离线定位，优先最终整合，避免重复整条链；硬上限仍未完成则保存准确partial、列最小剩余工作请求决定，不暗中超支。
管理文档/只读hash不挤产品额度。任何额外外部付费服务/发布/扩范围另需授权。
完成=所有T项必测真实成立、生产与包绑定、默认布局/交互/性能达标、证据闭集只读核验通过，才可delivered_for_inspection；仍由独立检查裁定，不自宣G34通过，不进入G35。
**请严格按任务书完成任务，不得因为其他任务无关问题擅自中断或停止部分步骤；保存checkpoint后应恢复继续，不得把上下文压缩当成放弃必测项的理由。**