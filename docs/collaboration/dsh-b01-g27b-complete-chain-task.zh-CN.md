# DSH B01-G27B continuation-2：完整包合同、真实几何与WebGL验收

日期：2026-09-23。状态：**已授权执行，不是待签批稿，不回到hold。**

## 0. 本次裁定与交付目标

产品确认与开工批准持续有效。检查员接受continuation-1的P1关键修复：显式world/G24 context独立重建，报告不再是来源信任根。检查员已独立通过CLI单测59项、core engineering_保护82项、当前源码构建以及6次CLI探针；自洽篡改v1高度即使配完整伪报告也被拒绝。该证据不等于G27B整体完成，更不等于已观察WebGL。

本任务**一次性实施并交付全部剩余范围**：包格式/资源/字段合同 → 几何/拾取 → 最终CLI与真实浏览器 → 最终证据绑定。不要只完成一处就再索要开工确认；只有真实阻断或本任务明确额度耗尽才准确partial停止。

本任务替代上一续作任务“只剩cargo3”等余额约束；原安全边界、来源绑定和旧证据冻结继续有效。不得把检查员已构建的二进制当作自己后续修改后的最终二进制。

检查报告：G:/difyllmwiki/GlyphWeave/docs/collaboration/b01-g27b-continuation1-inspection.zh-CN.md。

## 1. 明确新增额度与记账

以下是本次正式执行额度，不是预算建议，不需再次审批；不抹去任何历史消耗。

| 类别 | G27B已消耗 | 原累计上限 | 本次新增 | 新累计上限 | 本任务可用 |
| --- | ---: | ---: | ---: | ---: | ---: |
| cargo | 8 | 8 | 12 | 20 | 12 |
| CLI/服务入口进程 | 9 | 16 | 16 | 32 | 23 |
| 浏览器运行 | 0 | 5 | 2 | 7 | 7 |
| 辅助驱动 | 0 | 6 | 2 | 8 | 8 |

本续作归档≤64 MiB；旧包只引用、不复制撑大归档。检查员本轮cargo3/CLI6/矩阵驱动1/browser0另列，不消耗上述worker额度。cargo-42等是历史日志序号，不是G27B已运行42次。

每次cargo test/build、失败重跑、每个CLI和临时服务启动都逐项计数；CLI矩阵与前置服务可分栏，但不得把前置服务计为免费。浏览器崩溃、提前冒烟和最终复测都计运行。辅助驱动记录每次执行及实际子进程，不把驱动内多次CLI合并为一次。普通只读文件/hash核验不冒充CLI，另列命令日志。不得新增预算后倒改旧账。

ST18、默认世界、自然生成、工程build、route、bake、realize、世界发布额度仍全部为0。cargo编译消费者不等于工程build。

## 2. 写入范围和冻结面

- 主修改：G:/difyllmwiki/GlyphWeave/bevy/crates/cli/src/context_preview3d.rs。
- 可新增该3D消费者局部几何/JS/helper及测试；main.rs只做必要接线/help。复用G25函数所需pub(super)或局部只读helper提取可做，但保持原命令语义、运行保护测试，不另写弱化parser，不借机重构。
- core G24来源/裁剪算法、G22安全算法、G23旧viewer、路径A行为冻结。旧包、失败日志、清单和回执不覆盖、不倒写；不Git写入、不派子代理。
- DSH自行确认不存在后创建新证据目录：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g27b-context-preview-3d/continuation-2/。本任务发布不预建该目录。
- 对老清单核验其原已列文件；新续作子目录不是旧清单应被追溯补列的文件，不回改旧manifest。

固定只读输入：
- world：G:/difyllmwiki/GlyphWeave/bevy/work/joint-r27-verified-baseline/。
- G24 context：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g24-saved-scene-context/continuation-4/runs/c01-export-clean/package。
- G25 preview：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g25-context-preview-consumer/runs/p16-final-package/package。
- 固定窗口256,32,64,64；不要改成260、不要重生成来源。

## 3. P2：把完整包合同做实，保留P1

### 3.1 独立格式规则与全部字段

导出与audit共用版本化必需资产/字段规则，不能从待审manifest自己推导完整性规范。

- 六个必需文件恰一次、类型正确、普通文件、路径受限、无额外文件/目录；单文件与累计读取上限、目录错误和symlink/reparse拒绝保持。
- manifest/marker schema、固定文件引用、bytes/sha256类型与实际值、声明项唯一性和数量必须逐项验证，禁止filter_map丢错项后dedup伪装合规。
- 为model、viewer3d.js、preview3d.html、README.md建立完整内容绑定；manifest与marker用无循环依赖的哈希链绑定，marker最后写。不可要求manifest自哈希或伪造递归自绑定。
- 静态viewer资源须有生产者版本/构建指纹或同等明确不可变策略，审核与预期生产者资源核对；仅同步声明哈希不能证明任意替换JS仍是本产品。不要声称哈希链能证明任意脚本安全。
- 3D模型根字段闭集、必填类型、嵌套数组形状、有限数值、维度/长度、索引范围/拓扑、单位及转换规则均检查。所有事实字段应明确归属审核阶段，不遗漏heightDisplayConversion、来源身份或状态。
- 输入v1模型也须检查完整根字段集合，不只遍历已知字段而接受额外根声明。
- sources完整重算：sceneId/declaredSceneId/world/scene/context/输入v1及其manifest/marker等所有已声明来源字段，不再仅核对两个hash。
- HTML内嵌模型与包内完整模型一致（包括schema/sources/checks/limitations等非几何字段）；实际viewer必须消费这份已绑定模型，不能审核外部JSON而运行另一份内嵌数据。
- 未知版本在该版本不可解释时拒绝，不继续部分比较伪造pass；已确认fail优先于notEvaluated，依赖未满足的阶段明确notEvaluated，每阶段恰一条。

归属：sourceBinding负责独立来源事实；terrainParity负责双面顶点/单位/拓扑；entityAndRoadParity负责实体/道路/节点/access；previewCompleteness负责格式、必需资源、内嵌模型与静态生产者资产。对自洽单类语义篡改，更新允许更新的普通hash链后，应只触发对应语义阶段；格式破坏则按实际依赖记notEvaluated，不硬凑“其余全pass”。

### 3.2 本次发现的可选trace合同小缺口

检查员实测：合法来源配totalPass=false、错误modelSha256、四个pass另加同名sourceBinding fail，仍能导出。这不再构成来源绕过，但不符合“报告四阶段各恰一次”。

若提供可选context-audit留痕：受限读取；版本和字段类型遵循真实生产报告合同；四个名称完整且各恰一次、状态pass、摘要totalPass及model哈希一致；未知/重复阶段、矛盾摘要拒绝。报告缺省仍须能从真实来源独立通过。不得以加强报告检查替代P1独立重建。更新文案，不再把未提供报告的成功称为“由audit report授权”。

### 3.3 写入与失败

保留create_new报告、保护world/context/v1/3D包根、预存目标拒绝、原子取得目录所有权、只清理本次登记文件、marker最后写。新增格式字段/资源后仍要测试失败时没有可验收的complete包、外来哨兵不被删。

## 4. P3：真实绘制几何与拾取一致

以下是既有合同的明确实施规则，不再等待“观感”另批：

1. 双地形独立，顶点事实逐样本来自整数毫米，显示米只是换算；64×64样本为4096顶点/7938三角形。窗口样本x256..319、z32..95，不补到320/96。“不插值”禁止发明权威高度样本；绘制原有三角面及其数学求交可以产生派生浮点命中坐标，但不得倒写为保存毫米事实。
2. savedBounds是世界AABB事实，不再旋转一次。实体用保存worldXYZ、尺寸、rotation、pivot/grounding生成实际有面体块，不能只画AABB线框再命名精确体块；已烘焙尺寸不二次scale。只具AABB时可明确sourceAabbProxy，不冒充有向精确体块。
3. 沿权威变换：worldX=e.world_x+localX*cos+localZ*sin；worldZ=e.world_z-localX*sin+localZ*cos。保留原来源字段；缺高度/grounding不擅自推断。
4. 固定道路generated.road-network.scene-0.4-11来源为worldXYZ=(288,1,64)、width64/depth8/height0.24、rotation90、scale1、bottom pivot及保存grounding；savedBounds为x284..292/z32..96。用此具名例验证尺寸/旋转/pivot，没有要求把所有道路硬编码成该例。
5. 道路若需尺寸/高度，可按精确身份关联保存road entity并记录逐字段来源，不按距离猜测。节点/access只有x/z时显示为独立2D/元数据观测层或不生成3D primitive，并明示无高度/未定位/未贴合。禁止0.6/0.7/0.8等常数Y和地形补高。18条access是观测记录计数，不等于18条已知高度的3D线。
6. 拾取使用实际绘制几何：实体有向体块的面/等价局部射线计算，地形使用真实索引三角求交；禁用最近样本加步进代替三角面。隐藏图层不参与命中；实体优先的合同与可见面一致，结果标明entity/surface/triangle和派生坐标。
7. 具名数学反例包含旋转90度实体、非零/负原点、隐藏层、边界、退化/非法索引、2×2斜面。斜面样本[0,1000,0,1000]形成y=x时，x=.25/.75命中不得返回最近样本0/1；另测两三角不共面，锁定对角线和重心插值。

## 5. 执行顺序：先改完，再最终构建，不再拆微包

1. 只读绑定源码/来源，明确最终字段与资产规则；保留现有P1正负例。
2. 实施§3/§4和具名测试；必要时计入额度做一次早期真实WebGL冒烟，尽早发现shader/矩阵/资源加载问题，不当成最终版验收。
3. 修正后跑CLI单测与core engineering_保护回归，移除会写固定历史路径的测试导出环境变量；不跑已知会写旧证据的auto_roads全套。
4. **完成代码修改后**构建最终CLI，记录源码和exe哈希；所有正式CLI用此二进制，浏览器用其新导出的包。若再改产品，明确作废受影响最终证据，在额度内重新绑定，不拿旧包顶替。
5. 同一轮完成§6的CLI和§7五类浏览器场景，不为单项另开确认包；最后统一报告/清单/回执。

## 6. 最终CLI矩阵：至少18个有名进程项，23额度含服务/重跑

建议按以下18项一次编排，负例均从本轮干净包受控复制构造，旧包只读：

| 项 | 内容及目标 |
| --- | --- |
| 01 | 显式world/context/v1、不提供trace，干净export成功 |
| 02 | 该包audit四段pass |
| 03 | 同源同窗二次export，模型确定性一致 |
| 04 | 改v1高度、同步hash、完整伪trace，export拒绝 |
| 05 | 合法来源+矛盾/重复trace，拒绝 |
| 06/07 | 分别natural/final自洽顶点篡改，仅terrainParity失败 |
| 08 | 有向实体几何/rotation自洽篡改，entityAndRoadParity失败 |
| 09 | access观测标志升格，entityAndRoadParity失败 |
| 10 | 损坏viewer并同步普通资源声明，生产者资源合同拒绝 |
| 11 | 3D来源身份自洽篡改，sourceBinding失败 |
| 12 | 仅HTML内嵌schema/来源/状态不同，previewCompleteness失败 |
| 13 | manifest版本/类型/重复/固定引用代表反例，格式拒绝 |
| 14 | 删必需文件并删声明，仍拒绝 |
| 15 | 未知模型版本，拒绝且不做虚假部分比较 |
| 16 | 预存输出目录哨兵，拒绝且不变 |
| 17 | report预存/位于保护根代表反例，拒绝且输入不变 |
| 18 | 负例后独立干净审核四段pass |

13/17其余维度参数化单测全覆盖，CLI取代表而不宣称穷尽。其余5个进程额度用于实际服务/失败重跑/必要补例；可等价调整但须说明覆盖映射，不得为凑数量省略类别。每项保留参数、起止时间、真实退出码、stdout/stderr和报告原文，create_new run record；驱动不能因Select-Object等输出截断而隐形漏跑。

## 7. 真实WebGL2五类观察，最多7次运行

五类是覆盖要求，不强制启动恰好五个进程；允许一个会话覆盖多类，余量用于提前冒烟与真实缺陷修复。最终证据必须对应最终源码/二进制/包哈希。

- B01：真实WebGL2上下文、shader编译/link、GL错误、实际三角绘制与像素差异；natural/final分别可选且各自绑定正确样本/网格，显示单位和来源事实，不合并表面。
- B02：干净包重载至少3次，实体/道路/观测记录/图层对象计数稳定，不重复；HUD保留drift、limitations、无高度、不贴合和观测语义。
- B03：用真实page.mouse.click命中有向实体体块，返回entityId/kind/worldXYZ/rotation/scale/savedBounds；可按相机计算屏幕点，但不得调用选择helper或列表选择替代鼠标事件，不只信任viewer自报期望。
- B04：真实鼠标命中当前可见地形三角，记录surface/triangle/源顶点及派生命中点，以独立射线-三角数学预期核对；覆盖斜面/非采样点（受控夹具与真实包来源分开标注），不能用最近样本结果或自调用被测函数作为唯一oracle。
- B05：加载真实受控损坏包，必须看到实际拒绝状态、modelLoaded=false/清空模型与视口，再恢复干净包。不是访问历史上无服务的8793端口，也不是把连接错误等价为viewer拒包。保存坏状态发生当时截图，不得在回到干净页面后截图代替。

**禁止假绿**：page.evaluate异常必须记录notObserved/失败和原因；不能用异常分支填false/0/空串充当“实测无模型”。读取受限可以改用可观测DOM/渲染检查并明确证据层次，不能捏造浏览器状态。像素多样性、状态对象或正则扫描单独都不是3D语义证明。浏览器截图实际查看，记录console/page/GL错误；截图与断言对应同一状态。软件WebGL如存在须披露，不能把WebGL2成功另称硬件GPU验收。

## 8. 一次性交付与停点

交付report.zh-CN.md、progress.json、源码/二进制/包source-binding、具名测试和CLI矩阵、浏览器actual记录/原始截图、完整命令日志和逐进程台账。报告分开本轮实测/检查员观察/旧worker观察；失败尝试原样保留。

先结束运行并定稿报告与日志，再递归生成manifest绑定本轮全部证据（排除自身及最终receipt，明确排除项），receipt绑定manifest和关键报告；随后只读复核0 stale/0漏列/全部receipt匹配，核验结果若另落文件须采用预先明确的无循环绑定方案，不反过来改被绑定文件。总归档≤64 MiB。

退出标准：P1仍通过；完整包合同及几何具名测试通过；最终生产CLI矩阵到达目标；五类真实WebGL2观察有实际证据；保护源码与旧证据无未声明漂移。满足后仅报delivered_for_inspection，验收由检查员决定。任一未达成则准确partial并列具体剩余项和可用余额；不得自动超支、不得把已有授权误写成hold。

**授权现在生效：直接按本任务继续实施，不另交确认稿。A冻结、B partial、C不推进。**
