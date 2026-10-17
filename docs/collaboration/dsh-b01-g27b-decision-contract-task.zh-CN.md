# B01-G27B-DECISION：真实3D/WebGL开工合同确认

日期：2026-09-22。

当前状态：**G27B hold，不得开工。**

依据：
- G27A-R1路径A收口：G:/difyllmwiki/GlyphWeave/docs/collaboration/b01-g27a-r1-inspection.zh-CN.md
- G26R2路径A收口：G:/difyllmwiki/GlyphWeave/docs/collaboration/b01-g26r2-inspection.zh-CN.md
- 现有G27B可选任务：G:/difyllmwiki/GlyphWeave/docs/collaboration/dsh-b01-g27b-context-preview-3d-task.zh-CN.md

## 0. 这不是实现包

本任务只要求产品/主检查员完成一次**开工批示与合同确认**，不运行 cargo、CLI、浏览器，不修改源码、旧证据或固定来源，不创建 G27B 产品目录。

G27A路径A已经完成：保存场景上下文的显式只读2D交接入口成立。不能通过措辞把它改称真实3D/WebGL。

## 1. 必须同时确认的三项开工闸门

### 闸门 A：产品需求

必须书面确认：

> “只读2D上下文投影不再满足需求，需要新增真实3D/WebGL保存场景上下文消费者。”

如果未确认，G27B继续 hold，优先维持G27A路径A冻结。

### 闸门 B：独立3D合同

确认以下合同，不允许留作实现阶段临时决定：

1. **版本**
   - 新3D render-model schema/version号；
   - 与G25/G26 `engineering-context-preview-v1` 的关系；
   - 是否复用G25来源字段和四段审核，或新增版本化字段；
   - 未知版本必须拒绝。

2. **坐标和单位**
   - X/Y/Z轴方向；
   - 高度单位为保存的毫米事实还是转换后的米；
   - world坐标原点和窗口坐标保持方式；
   - natural/final 必须为两层独立表面，不能合并。

3. **地形网格**
   - 顶点来自 `crop.naturalMm` / `crop.finalMm` 的逐样本映射；
   - 明确网格分辨率和三角形拓扑；
   - 不补高度、不插值、不用另一表面兜底；
   - 边界和负/非零坐标保持来源事实。

4. **实体、道路与 access**
   - 实体使用体块、轮廓拉伸或其他明确几何表示；
   - 道路/节点按保存事实映射；
   - access path 保持 `sourceObservationOnly` / `notReverified`；
   - 不把 `connected`、`walkable_route` 变成 verified route。

5. **贴合规则**
   - 道路和 access 是否悬浮、贴最终面、贴自然面或仅按世界坐标绘制；
   - 若保存数据没有对应高度，必须显示为无高度/未贴合；
   - 不为了视觉贴合补高度或插值。

6. **鼠标命中**
   - 命中体块、地形表面，还是二者都可；
   - 命中优先级和遮挡规则；
   - 返回字段至少包括 entityId/kind/worldXYZ/rotation/scale/savedBounds；
   - 必须使用真实鼠标事件证明，不以列表选择替代。

7. **四段审核归属**
   - `sourceBinding`：context、source和3D模型来源一致；
   - `terrainParity`：natural/final顶点、单位和拓扑一致；
   - `entityAndRoadParity`：实体、道路、节点、access逐字段一致；
   - `previewCompleteness`：模型、viewer、依赖和完成标记完整。

### 闸门 C：预算

明确批准独立新额度：

- cargo **8**；
- CLI **16**；
- 浏览器/WebGL **5**；
- 辅助驱动 **6**；
- 归档≤64 MiB；
- 不复用G27A、G26、G25历史余额；
- ST18、默认世界、自然生成、工程build、route、bake、realize、发布均为0。

## 2. 批示格式

只有以下信息全部填全，DSH才可开始G27B：

```text
产品确认：是/否，需要真实3D/WebGL
3D schema/version：________________
与 G25/G26 context 的关系：________________
natural/final 坐标、单位、分层规则：________________
地形网格/拓扑/无插值规则：________________
实体/道路/access 表示：________________
贴合规则：________________
鼠标命中对象与返回字段：________________
四段审核归属：________________
独立预算：cargo 8 / CLI 16 / browser 5 / aux 6 / archive 64 MiB
批准开工：是/否
```

## 3. 批示后的执行顺序

获得完整批示后，另建新目录：

`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g27b-context-preview-3d/`

执行顺序：

1. 只读重绑G24/G25来源和冻结源码；
2. 实现版本化3D render model与viewer；
3. 先做具名单测；
4. 构建最终CLI；
5. 执行自洽篡改矩阵；
6. 执行5次真实WebGL2浏览器观察；
7. 生成报告、progress、manifest、receipt和只读核验。

未获得完整批示前，以上步骤全部不执行。

## 4. 冻结面

G22、G23、G23R1、G24、G25、G25R1、G26、G26R1、G26R2、G27A、G27A-R1 的源码、包与证据保持冻结；G23旧Three/WebGL viewer不改写、不作为兼容借口；G27A路径A不改名为3D。

**A冻结、Bpartial、C不推进。**
