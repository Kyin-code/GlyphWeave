# B01-G23：已验证工程连接包的只读3D预览消费

日期：2026-09-20。用户已授权继续推进；本文件为下一单一执行包任务，不是待批方案。无需回复收到或再开设计确认轮；在包内写清数据流后实施，完成一次交付。超白名单、预算不足或缺必需环境时如实partial停止。

## 0. 为什么推进这一包

G22专项修复已经检查员限域验收，**停止围绕同一支持判据继续打补丁**。下一步让已通过的结构几何进入实际消费者，从“CLI说可走”推进到“人在项目预览中能看见、检查同一套真实几何”。

本包只接**一个消费者：项目HTML工程预览适配层**。数据链为：

    保存的EngineeringInput + committed EngineeringBundle
      → 现有verify_bundle复验
      → 新目录只读预览导出（来源/几何hash绑定）
      → adapters/html体系下的显式工程预览入口
      → 对导出包的独立只读audit与浏览器实际显示核验

不是同时接ST18、SceneIndex、world-quality-gate和默认预览流。不启动自然生成，不修改默认城市行为，不新增楼梯之外的设施。用户所关心的城市丰富度、交通和改造自然的长期方向保留，本包首先实现可见、可核对的空间消费链。

## 1. 冻结基线与现有代码事实

已验收基线不得作为本包顺手重构对象：

- G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/worldgen/engineering_connections.rs：431267B，SHA256 fe442d464c7c9d4863863a4085e337deb77e2dc89cf290c95e16d1f5afbb91f4。
- G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/worldgen/engineering_connections/support_coverage.rs：32557B，SHA256 a3ab64dcc2c8841429c03c51af8d8cac24145763c2bb7e6d9e114abb5d4e897b。
- G:/difyllmwiki/GlyphWeave/bevy/crates/cli/src/engineering_connections.rs：19152B，SHA256 99c90e46f1a926e61d654e74e35372e45eaddfaebacba2b16898be700205ecd6。
- 验收裁定：G:/difyllmwiki/GlyphWeave/docs/collaboration/b01-g22-boundary-acceptance.zh-CN.md。

本次只读梳理确认：

- SceneIndex定义在G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/worldgen.rs，当前含terrain、access_graph、access_surfaces等；本包不加必填字段、不修改其默认生产结构。
- G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/preview_stream.rs及preview_stream_audit.rs负责现有世界渲染投影与其审核，本包不改其既有语义/输出。
- G:/difyllmwiki/GlyphWeave/adapters/html/已有Three.js渲染适配、vendored three与OrbitControls；可复用本地依赖，不需要外部CDN或新框架。
- G:/difyllmwiki/GlyphWeave/bevy/crates/cli/src/main.rs的preview命令可对指定目录提供本地静态HTTP服务；工程预览输出可采用preview/index.html布局复用该入口，不伪造world.json或SceneIndex表示“已世界集成”。
- adapters/html/core/three.js目前含CDN动态导入，不直接用于本包离线入口；从现有vendor目录建立本地import map/相对导入，不为此修改全局默认预览加载行为。

起步只读核对上述基线、资源和调用入口。发现漂移先明确来源，禁止自动回滚未提交工作。旧包不改名、不清理、不覆盖。

## 2. 新入口与产品范围

新增两个显式CLI入口，名称可在实现前包内定稿（推荐如下）：

    glyphweave export-engineering-preview --input INPUT.json --bundle BUNDLE.json --out NEW_DIR
    glyphweave audit-engineering-preview --preview DIR --report NEW_REPORT.json

它们只消费人工工程包，不自动触发generate/build-engineering/bake/realize。

HTML入口放在现有适配体系下，例如新增G:/difyllmwiki/GlyphWeave/adapters/html/engineering/；由导出器复制必要静态模块及vendor依赖到NEW_DIR/preview/。它是工程检查预览，不是新世界生成器，也不是用SVG/OBJ文件存在假装浏览器已经消费。

默认旧preview/export-preview-stream命令不带新参数时保持既有行为。独立工程预览可以用现有preview命令指向NEW_DIR服务，不要求变更旧页面。不要为了本包改造整个app.js或SceneIndex存档模型。

## 3. 导出门：只接受当前真正可审核的保存包

### 3.1 验证先于成功产物

导出必须：

1. 读取原始input与bundle字节，计算来源bytes/SHA256，并记录bundle自身inputHash/payloadHash/contractVersion/publication状态；不同hash口径不得混写。
2. 用现有生产load_input与verify_bundle完整复验，检查committed和支持事实版本。只有verified才进入可展示导出。
3. 只读消费source files；不修改其publication、facts、mesh、route、地形或hash来适配预览。验证失败结构化拒绝，不生成貌似成功的预览包。
4. 校对sceneId、units、栅格范围/原点、自然/工程面来源与结构几何/路由身份，不能因两个文件恰好有同名sceneId就视为同源。
5. path/URL均由本包安全相对路径生成；拒绝路径穿越、绝对路径、越根目录符号链接/重解析点等。审核器同样约束，不盲目信任包内路径。

默认展示域的路由是已保存query的结果；roadId为该人工输入中的声明，不宣称已绑定ST18真实道路。

### 3.2 导出内容与只读事实

最小导出内容可包括：

- 原input与bundle的**原字节副本**或同等可复验文件；
- 版本化engineering-preview描述文件，来源hash/bytes、scene/domain/units、verified时的引擎/证据版本、导出状态；
- 用于渲染的结构几何、部件ID/结构ID/levelId/surfaceId、原端口、边及有向route；
- 每个数据文件的hash/bytes、对象数、几何bounds及来源映射；
- 本地HTML/JS/CSS/vendor依赖；
- 独立审核报告不作为“信任自己”的依据，而是复验输出。

不要求新格式重复保存一份已在bundle中的全部数据，可让viewer在验证文件hash后直接消费bundle；若另作渲染投影，必须有明确可重算的一对一映射，不能丢失高度层或改变坐标。

发布采用新目录、预检、staging及最终完成标记；未完成状态消费者拒绝或显式显示未就绪。预存目标不得覆盖，写失败只清理由本次创建的对象，不删除他人文件。不要重写G22的构建发布原语，局部复用或实现本导出器所需的最小事务。

## 4. 浏览器必须真正消费生产几何

### 4.1 必须显示

- 直跑/折返的实际踏步、平台、支柱/支撑；使用保存的生产corners/vertices及既有盒体拓扑，不根据“stair”标签另画装饰模型。
- x/y/z世界坐标与尺度原样保留，不贴地、不把多层压成一个平面、不按视觉需要抬高真实部件。
- 已保存有向路径、三维端口、surface/level/structure/part身份。
- 保存工程高程面作为地面背景；可切换自然/基线面，标签说明当前显示的面及hash。这里只将保存栅格转成**显示mesh**，不得重新运行地形/carve合成，不回写样本。
- 输入里的实体障碍/墙与水域作为只读检查层；显式为空与缺字段不同，缺必要上下文不伪装为空场景。
- 不同结构角色采用简洁可辨识材质；尺度参照/坐标轴、俯视/侧视/透视及轨道相机满足检查高差和接续。真实感先来自准确空间，不追加艺术资产、植被或建筑美术。

### 4.2 最小交互

- 结构、支持件、地形、路由、端口、障碍检查层开关；开关仅控制可见性，不改变几何、审核结论或通行能力。
- 点选部件/端口显示ID、角色、level/surface、位置、所属结构；避免将名字相同误认为同一对象。
- 显示已保存query的能力、起终点/roadId、route状态、三维长度和爬升；不在前端擅自重新寻路。
- 能力过滤若提供，只表示边的已保存能力声明；切换轮椅/车辆不能把同一楼梯路径重新标成found，也不能伪造“此输入已作过该能力query”。
- 同场景重载/切换：dispose旧资源，防重叠叠加、重复ID、旧路径残留；失败清空相关verified展示或明确stale，不继续显示上一包绿灯。

### 4.3 信任边界

浏览器检查导出文件hash、完整性与来源标识，成功标签应是“导出时工程验证通过／当前文件完整性一致”，**不声称浏览器独立执行了Rust工程审核**。

若数据文件或hash不符，显式拒绝加载对应几何/路径，不画绿线；不能catch后用空数组回退为正常。浏览器显示不是安全授权依据，独立audit命令才重新执行生产verify和渲染映射检查。

## 5. 独立audit与几何一致性

`audit-engineering-preview`必须重新：

1. 读取并校验文件路径、bytes/hash、完成状态、schema/domain/units；缺字段/文件/不支持版本明确拒绝。
2. 从导出的原input/bundle调用现有verify_bundle；不能只信任保存的verified字样。
3. 从生产bundle重建应显示的部件几何、端口、route与能力信息，核对导出投影或viewer读取的数据绑定；同名但不同几何、错层、漏部件、错连接要能被指出。
4. 验证显示坐标转换为固定约定，不让路由与实体分别缩放。JS以f32上传GPU时可声明有限显示误差上界（如1e-5m，需按本人工坐标范围验证），不能回写工程数据或将显示容差作为工程通过容差。
5. 输出分离的结论：engineeringVerification、sourceBinding、renderParity、previewCompleteness。工程包有效不等于渲染投影正确，hash自洽也不等于工程有效。

至少两类自洽篡改负例：

- 改保存结构几何并重算相关hash：仍被生产工程审核/指定支持检查拒绝；
- 只改渲染投影的顶点、level/part绑定或route映射并更新外层文件hash：被renderParity拒绝；若采用直接bundle渲染、没有独立投影，则用前端构造函数测试注入错坐标/漏part，证明其与bundle几何断言失败，不强行增造一份格式来凑反例。

原payloadHash损坏、input不匹配、staged包拒绝保留。任何篡改必须只作用于本轮独立副本，不改G22冻结文件。

## 6. 固定输入与最小验收

### 6.1 三个人工正例

优先读取已保存并绑定的：

- 最小直跑：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g22-bearing-state-safety/straight-build-bundle.json + G:/difyllmwiki/GlyphWeave/bevy/crates/core/tests/resources/g19r2/minimal-straight-stair.json。
- 最小折返：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g22-boundary-completion/switchback-build-bundle.json + 同资源目录minimal-switchback.json。
- 带地面接驳/工程面的terrace：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g22-bearing-state-safety/terrace-build-bundle.json + 同资源目录positive-terrace.json。

输入/包同源以实际hash和当前verify为准，不因文件名推定兼容。若现存bundle因明确版本问题不能复用，可在新目录用冻结人工输入显式重建最多3包（计CLI/构造次数），保留旧拒绝原因；不得更改输入或旧包使其通过。本包不产生任何新的自然/ST18面。

### 6.2 核心/CLI验收清单

最低14个CLI调用（独立audit算单次）：

1–6. 三正例各导出+独立audit；
7. input/bundle来源不匹配拒绝；
8. staged或缺支持schema包拒绝；
9. payloadHash破坏包导出拒绝；
10. 自洽几何破坏包拒绝（不只依靠外层hash）；
11. 成功导出后缺少/破坏数据文件，audit拒绝；
12. 自洽渲染事实破坏的audit拒绝；若无独立投影则改为level/route映射输入拒绝并用前端单测补足渲染变换验证；
13. 预存导出目录冲突，原文件未动；
14. 提交中故障注入/真实失败，未产生可消费完成包且无越界清理。

如为安全路径测试使用额外CLI，计入总数。失败用例未到目标阶段时必须写“被前置拒绝，目标分支未观测”，不得像G21同步注入那样用output_conflict冒充后置失败。

新增Rust只测本导出/读取逻辑、CLI参数/发布；保留engineering_connections32项定向回归，不运行会写旧证据的全库auto_roads测试。JS/浏览器测试验证真实render数据转换、hash失败、场景切换释放、能力显示不越权。

### 6.3 真浏览器验收，不能只交SVG/OBJ

通过本地HTTP实际加载导出目录，使用项目已有Playwright或浏览器工具；离线阻断外网后仍能加载所有必要模块与数据。按工具技能与本机权限规范使用，后台服务默认隐藏且仅绑定127.0.0.1，记录PID/端口，任务结束关闭本次启动的服务。

至少留存并实际查看：

- 直跑透视全景：每级踏步、顶部接驳与支持；
- 折返透视及侧视：两梯段、平台、转向路径、支撑相对位置；
- terrace工程地形/自然地形切换对比，明确两种表面而非偷偷替换高程；
- 点选一个踏步或平台，显示身份与标高；
- 破坏数据后的明显拒绝状态；
- 重载/切换后对象数与路径不重复。

保存console/pageerror/network failure与数据/mesh对象计数。截图只证显示结果，几何一致性仍由数值测试和audit支撑。WebGL不具备时准确blocked/partial，不用合成截图或仅2D图冒充3D消费通过。

## 7. 白名单与禁止事项

允许新增/修改：

- G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/engineering_preview.rs（或同职责局部模块）及G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/lib.rs的最小注册；
- G:/difyllmwiki/GlyphWeave/bevy/crates/cli/src/engineering_preview.rs、main.rs最小分派/用法及对应测试；
- G:/difyllmwiki/GlyphWeave/adapters/html/engineering/中的新显式入口、渲染/加载模块与局部测试；可只读引用现有vendor，不能改vendor源码；
- 必要独立测试资源、一个本包验收脚本、目录树/文档；如需复用既有资源导出helper，只允许等价提取且有旧入口回归，不进行架构大重写。

冻结不改：G22三个源码基线、access/auto_roads、SceneIndex与world.json默认schema、preview_stream/preview_stream_audit原格式、默认世界生成/路由/质量门、旧HTML默认页面行为、合法楼梯坐标、安全阈值。新消费者应调用现有公开API，不为接预览顺手修G22求证算法。

确有缺少公开只读数据转换API导致无法接入时，记录具体符号和最小必需修改，停partial等裁定；不要自批扩大白名单。不要通过复制一套工程验证器来绕过API限制。

无Git add/commit/stash/reset/rebase，无子代理，无新增第三方大依赖/在线CDN，无全量格式化，不清理无关未提交文件。

## 8. 新包明确预算（无历史余额）

G22已用满cargo12/12、CLI24/24，不结转、不抵消历史G20超支1。

本包新增：

- cargo **12次**，含build/check/test/失败调用；预留最终核心回归与CLI构建。
- CLI **24次**，含导出、独立audit、必要人工工程包重建、失败尝试、preview服务启动；最多3次人工工程包build是其中子上限。
- 前端自动化/JS测试/浏览器验收驱动进程 **12次**；每次浏览器运行内的页面/视角切换单列，不按截图数重复扣进程；建议固定一份矩阵脚本一次跑多个案例。
- 人工正例3组、派生负例≤12组；栅格≤64×64、结构≤64；预览只读显示网格转换允许，但自然生成/地形或carve重合成/ST18/realize/bake/世界发布 **0**。
- 单次CLI≤30s，累计CLI≤600s；浏览器每次≤180s；人工验证执行总墙钟≤1800s（编译另列）。预览服务的驻留等待不计为自然运行，必须关闭。
- 证据归档≤64MiB；vendor副本、截图计入，避免录长视频。程序二进制只记录hash/bytes，不复制进包。

所有底层进程逐次计数，多个cargo命令写在一个shell不算一次。只读Get-Content/Get-FileHash不计人工驱动进程，但源码生成/试运行脚本如实记录，不用它们隐瞒构建或CLI调用。

## 9. 台账驱动必须先可靠，不再事后重建退出码

禁止沿用已两次失效的PowerShell内联汇总模式直接启动整矩阵。可用现有Node child_process或已验证的等价驱动：

- 每次进程结束立即写一份create_new run record，记录argv、cwd、实际exit code、开始/结束/耗时、stdout/stderr路径及hash、report路径和是否存在；
- 先运行一个成功和一个预期拒绝案例验证记录器（实际CLI计入预算，可同时作为矩阵对应行，不额外重跑）；
- 解析报告与观测退出码分开。缺字段/缺report写明确状态，不能继承上一条记录；
- 汇总只读取已写原始run records，不能用预期值填观测，也不能以事后推导exit替代本包实测；
- 台账/脚本失败保留日志并扣预算，不能默认为CLI未发生。

## 10. 实施与一次交付顺序

1. 只读绑定G22及三输入/包，包内写明单消费者数据流与信任边界。
2. 实现小型导出/独立audit，先完单测和负例；不动已验收求证器。
3. 实现现有HTML适配层下独立工程入口，真实消费几何；完成JS数值映射测试。
4. 绑定最终CLI构建，跑计数准确的CLI矩阵。
5. 启动本地预览，浏览器截图/实际查看/重载与错误状态验证；不把人工fixture宣称真实ST18成功。
6. 最终源、静态资源、测试与输入版本绑定；报告区分工程verify、导出audit、渲染数值一致性、视觉观察、unknown。
7. 全产物/progress定稿后清单→回执→只读核验；旧G22两包按文件hash复核未变。停止后不再写已入清单文件。

新证据目录：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g23-engineering-preview-consumer/。检查员此刻只写任务，未创建运行目录。

## 11. 正负分支

正分支：三个已验证人工工程包成功进入实际浏览器3D预览，独立audit与数值/视觉证据一致，负例不误显示为verified，旧默认入口及冻结G22不受影响；交delivered_for_inspection，等待独立验收。

负分支：任一来源验证、完整几何呈现、保存映射、浏览器实际消费、默认兼容或预算门未达，准确partial停止。不得改阈值迁就显示、不得只交HTML壳/截图或离线SVG宣称消费者完成，不自动追加额度/接真实世界。

A冻结、B partial、C不推进。下一消费者或真实站点接入另批，不由本包自动连带授权。
