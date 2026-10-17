# B01-G27B-APPROVAL：真实3D/WebGL开工批示确认

日期：2026-09-22。

## 当前裁定：已批准开工

生效日期：2026-09-23（本地时间）。授权依据：用户明确指示“让他开工吧”。

**产品确认：是。批准开工：是。G27B 的 hold 即刻解除。**

确认新增真实3D/WebGL保存场景上下文消费者的需求；G27A路径A的只读2D入口继续保留，不被替换或改名。本文件下列合同与独立预算全部生效，不再是候选建议。

本批示取代本文件及旧G27B decision/任务文档中“待确认”“仅候选”“未获批准不得开工”的等待条件；不覆盖、不倒写旧交付证据。无需再次提交确认稿或申请同一授权。

授权DSH从只读重绑基线开始，创建G27B新目录，在下列额度内执行实现、测试、构建、CLI与浏览器验收，完成后一次交检查。不得将本批示写成已完成实现或已通过验收。

G27A/G27A-R1路径A及历史包与证据继续冻结。

## 正式生效批示

```text
产品确认：是
确认需要新增真实3D/WebGL保存场景上下文消费者；已有只读2D投影不足以覆盖本阶段真实3D展示需求。

3D schema/version：engineering-context-preview3d-v1

与 G25/G26 context 的关系：
与 engineering-context-preview-v1 并列的独立3D模型版本；
复用已审核 context 的 sources 绑定和四段审核概念；
不改写v1包，不把v1包改名成3D；未知版本拒绝。

坐标/单位/分层：
世界坐标；X东、Y上、Z南；窗口原点与G24/G25一致；
高度事实保留保存的整数毫米，显示米仅为无损换算；
natural与final是两层独立表面，可分别开关，禁止合并。

地形网格/拓扑/无插值：
natural顶点逐样本来自crop.naturalMm；
final顶点逐样本来自crop.finalMm；
固定64×64样本形成63×63四边形、每格2三角；
不补高度、不插值、不用另一表面兜底；
负/非零坐标与窗口边界保持来源事实。

实体/道路/access表示：
实体按savedBounds与rotation表达为地面体块；
高度使用保存heightM，缺失即显式无高度；
道路使用保存长度/旋转的矩形带；节点按保存事实标记，缺坐标为referenceOnly；
access使用原始折线，永久带sourceObservationOnly/notReverified；
connected/walkable_route不得升级为verified route。

贴合规则：
所有道路与access按保存世界坐标绘制，不贴natural/final表面；
没有对应高度时显示无高度/未贴合；
不得为视觉效果补高度或插值，任何改变须重新取得合同批准。

鼠标命中：
真实page.mouse.click；优先命中实体体块，其次地形表面；
返回entityId/kind/worldXYZ/rotation/scale/savedBounds；
不能用列表选择替代鼠标命中。

四段审核归属：
sourceBinding：context、来源与3D模型来源一致；
terrainParity：natural/final顶点、单位、拓扑一致；
entityAndRoadParity：实体/道路/节点/access逐字段一致；
previewCompleteness：模型、viewer、依赖、完成标记完整。

独立预算：cargo 8 / CLI 16 / browser 5 / aux 6 / archive ≤64 MiB。
不复用G27A/G26/G25历史余额；ST18、默认世界、自然生成、工程build、route、bake、realize、发布均为0。

批准开工：是
```

## 批示后的执行任务

**上述两个签批字段均已为“是”，合同与预算已生效。DSH现在直接执行，不再等待：**

1. 新建 `G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g27b-context-preview-3d/`；
2. 只读重绑G24/G25来源和冻结源码；
3. 实现独立版本化3D render model和viewer；
4. 具名单测；
5. 最终CLI构建和自洽篡改矩阵；
6. 五次真实WebGL2浏览器观察；
7. 报告、progress、manifest、receipt和只读核验。

## 实施白名单与停点

以新增独立3D模块、viewer和新证据目录实现；允许现有CLI main.rs/core lib.rs做必要的模块注册、显式命令分派与usage增量，须披露前后差异，不改变既有命令逻辑。此最小接线是下列历史源码冻结规则的明确例外；不得以“注册文件冻结”为理由再次等待，也不得借此重构旧模块。

预算为独立新额度：cargo最多8次、CLI最多16次、浏览器/WebGL最多5次、辅助驱动最多6次、归档不超过64 MiB。失败调用照实计入，不重复使用历史余额；所有CLI启动（含用于浏览器验收的前置服务）计入CLI额度，不能改名为前置进程规避计数。先规划测试、最终构建和验收所需调用。

预算不足或存在实质合同矛盾时准确partial停点，保留代码、失败测试和原始记录；不得因原“待签批”条件继续hold。G27B完成后仅提交delivered_for_inspection，由检查员验收，不自动推进ST18或默认世界。

## 固定禁止项

不修改G22、G23、G23R1、G24、G25、G25R1、G26、G26R1、G26R2、G27A、G27A-R1的源码、包和证据；不改G23旧Three/WebGL viewer；不把G27A的2D结果改名为3D；不接ST18、默认世界、SceneIndex默认消费者，不生成EngineeringInput，不重新求路，不建楼梯，不做工程build/route/bake/realize/release。

**A冻结、Bpartial、C不推进。**
