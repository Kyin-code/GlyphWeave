# G34-R5：像素证据、可恢复浏览器验收与最终闭集收口（执行令）

日期：2026-09-28。本任务直接授权本范围诊断、必要修复、生产与最终浏览器验收；G35不启动。R4仍为partial，不是已验收基线。

## 0. 严格执行与恢复

**请严格按任务书完成任务，不得因为与其他任务无关的问题擅自中断、停止或跳过任何步骤；不得因为上下文压缩、会话切换、摘要丢失、其他任务状态或可修复的驱动问题而擅自停止部分步骤。**
遇到上下文不足，必须在新目录写checkpoint：源码/包hash、T编号完成与剩余、当前场景和阶段、最后错误、真实账目、下一条命令；恢复后从checkpoint继续，不重置预算、不删除必测项、不把checkpoint当交付。相同原因两次失败且无新诊断，停止原样重复，换最小诊断。只有真实安全/权限/范围/外部依赖或硬上限才可交准确partial，并列最小决策请求。

## 1. 路径与冻结边界

仓库：G:/difyllmwiki/GlyphWeave/。
新目录：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g34-r5-final-browser/，必须create-new。
R4候选包：`.../ds-b01-g34-r4-atomic-evidence/cli-runs/r4-c02-export`，只读诊断输入；若源码改变，必须新build/new export后才可验收。
R4检查目录：`.../inspector-b01-g34-r4-20260928/`，只读；旧G33/G34证据只读。
允许产品修改仅 `bevy/crates/cli/src/g28_viewer3d.js` 的G34 snapshot/clip接线和 `context_preview3d.rs` 的G34 HTML/CSS/测试/生产ID；model、主图相机/拾取/几何、G32导航、G33 footprint数学、材质/shader、G30读图和core/G23冻结。生产ID升为`g34-terrain-visibility-6`。无Git写、无ST18/新世界/route-bake-realize、不修改任何旧目录。
先写合同、visual intent、源码字节备份和唯一scene表；中文源码用显式UTF-8编辑。

## 2. T0：进程、路径和挂起防护先于任何整跑

所有可写依赖解析到本轮或明确授权产品文件；旧轮accounting/runner禁止import。每次写文件/建目录先过路径守卫。产品cargo/CLI/driver/Chrome均使用统一`runRecorded`，记录invocationId、真实childPid、attempt、start/end、exitCode或timeout、stdout/stderr；未知不得写成成功/失败。Chrome诊断也计数。
每个scene必须有：`sceneId`、`beginUtc`、`endUtc`或timeout、stage、lastAction、result/error。driver总finally必须写`progress.json`和`results.json`；即使单场景超时也要退出非零、保存已完成/剩余场景，不得只留下intent。所有`page.evaluate`中的Promise必须有超时：尤其Image onload/onerror、raster、waitForFunction、page.goto。
先运行一个只含boot+单个clip像素的smoke，证明会在deadline内写结果；再运行完整矩阵。不允许再运行无超时的30分钟整合driver。

## 3. T1：R4原子快照的真实验证

保留snapshot机制，但最终浏览器必须证明：同一次`__render3d()`产生一个`snapshotId`，`Object.isFrozen(snapshot)`与cell冻结成立，camera/canvas/backing/DPR/window/footprint/resolution完整，`selfConsistent=true`。
同一snapshot内：完整cells数量=sampleCount；每个category计数与aggregate逐项相等；`hiddenCells`、aggregate hidden、`drawnCells`和DOM `data-cell`集合逐项相等；集合比较必须同时带snapshotId。独立grid hook仅作为`independent=true/fromRenderedFrame=false`对照，不能与渲染快照混称。
重复50次、A→B→A、probe交错、surface/viewport切换必须逐snapshot校验，不能只比较hidden总数。任何snapshot自相矛盾必须fatal/nonzero。

## 4. T2：五形状原生像素证据必须是新结果

R4旧`calibration/r4-clip-pixels.json`保留并标注为历史失败，不得覆盖成通过。新结果写`calibration/r5-clip-pixels.json`，包含驱动版本/hash、package/source hash、shape points、world→px结果、内外采样坐标、RGBA、allOk。
使用窗口比例生成world shape：`minX + u*widthM`、`minZ + v*depthM`。五形状至少三角、斜四边形、六边形、贴窗边、小于一格；另加一个中心在内、角点在外的真实cell。实际复制产品renderer输出的两层clip结构和rect，隔离页仅允许换测试色，不得改points/clip结构。
每个shape的raster Promise必须有短deadline、onload/onerror双路径，超时返回结构化fatal；采样点必须在画布内并与边界保持预声明距离。内点有色、外点透明/背景，不能仅凭clip节点或polygon点判定。

## 5. T3：正确性与回归

完整矩阵前置唯一scene ID检查：不重复、不缺项；29项或修订后的等价集合绑定intent。两个面积场景使用不同门槛：1280×800≥0.80，960×640≥0.75；交互面积另报扣小地图，不混淆。
坏包测试必须破坏页面实际读取的内嵌模型/HTML/viewer输入；若是内嵌模型优先，外部json修改只能作为“外部修改不影响页面”的辅助场景，不得称拒绝。恢复后验证pageLoads、modelLoaded、snapshotId变化、hidden层/旧状态清空。
几何合同只允许明确字段：terrainIndexCount23814、terrainTriangleCount7938、surfaceVertexCount4096、declaredTriangles7938、declaredVertices4096、solidDrawCalls7；8022禁止出现。
保留G32/G33四角、clamp、第二pointer/cancel/blur、选择保持、主图pan/rotate/wheel/pick、模式/图层、两surface、状态条拖拽、GL/console/page错误、坏包恢复和snapshot atomicity。

## 6. T4：生产链和闭集一次收口

如产品源码未变，可用R4 c02包但必须在新intent中重新绑定并证明零漂移；若源码变，必须cargo test/bin build/core保护→export/audit→第二export/audit，c01/c02六文件逐字节一致，包内viewer=源码。禁止CLI先于cargo，禁止复用已有audit输出目录；每个输出create-new。
所有报告、台账、driver副本、checkpoint、校准、截图和stdout/stderr完成后再生成manifest。精确排除仅本轮`manifest.json`、`receipt.json`、`delivery.json`、`verify-log.stdout.txt`；确认脚本和最终核验器必须先列入闭集。verifier只读，核验前后全目录二次枚举一致，旧目录只读并以本轮起止快照证明零漂移。
若最终仍有真实产品缺陷、未观测、driver timeout、fatal或闭集漂移，交partial，不得用“已修源码”或probe烟测替代最终矩阵。

## 7. 资源和停止

软计划：cargo 8、CLI 12、Chrome 8、driver 20、归档128MiB；硬上限：cargo12、CLI18、Chrome12、driver30、归档192MiB。达到软计划75%必须报告实际用量、剩余场景和诊断；软计划不是自动停工理由，但硬上限仍未收口则交准确partial。历史R3/R4未知账目不估算、不转入本轮。诊断浏览器照实计。

完成条件：新版T2像素证据、同一snapshot原子一致、完整29场景结果、两尺寸正确阈值、坏包真实输入、几何字段正确、生产包绑定、闭集只读核验全部成立，才可交`delivered_for_inspection`；独立检查仍保留最终裁定，G35不启动。

## 8. 上轮未做好清单（必须逐条在R5报告回应）

1. R4把新版坐标修正后的T2像素结果留在未执行状态，旧失败JSON容易被误读；
2. R4的Image栅格化Promise无timeout，导致driver挂起30分钟且不写results；
3. R4 driver绕过统一runRecorded，真实运行次数未知；
4. R4一度CLI先于cargo，产生旧viewer包；
5. R4 audit复用已有report输出路径，造成可避免的覆盖拒绝；
6. R4 T4挂起后没有结构化progress/results/checkpoint收口；
7. R4 T5闭集、verifier和二次枚举未完成。

R5报告必须逐条写“已修/仍未修/证据路径”，不得只写“已披露”。
**请严格按任务书完成任务，不得因为其他任务无关问题擅自中断或停止部分步骤；不得因上下文压缩而跳过步骤，必须保存checkpoint并恢复继续。**