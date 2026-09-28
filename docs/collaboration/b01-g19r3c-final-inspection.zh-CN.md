# B01-G19R3C最终候选独立复核（2026-09-20）

## 裁定

**verified-progress / structure-support-false-positive / closure-not-accepted**。

本次确认上一轮区间分量、空测站、错层与越界终点探针均已修好，52/52清单绑定也已修好，不再重复要求这些工作。但结构边另走仅顶点的测量路径，并把缺支持或靠部件端部的顶点直接跳过，仍导致实际生产CLI放行支持带不足的路线，独立审核亦误报verified。因此接收已核实进展，不接受“生产链一次闭合”，状态仍partial。

这是原任务中“所有landing/stair/ground消费、最后端点、段中支持范围变化”的未闭合项，不追加新设施或多分辨率合同。A冻结、B partial、C不推进；禁止自动进入ST18/SceneIndex/preview/audit/default world。

## 独立核验

证据根目录：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g19r3c-final-20260920/。

- 当前worker包：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g19r3-production-chain-closure/。
- 清单52/52 hash与bytes匹配；closing-receipt对清单hash/bytes匹配。生产core SHA256 b897cce9a5eed6f061d5e5d11c2ab61b012567c0c3aae4be556e33c52f8ff1cd，CLI 99c90e46f1a926e61d654e74e35372e45eaddfaebacba2b16898be700205ecd6。
- 检查员重新构建CLI，并重跑指定core测试：11/11通过。
- 直接链接同一生产rlib的旧探针：near_only与near_plus_disjoint_far均1.6000000000000005；空owner、空折线、零水平段、错误高度、越过支持终点均0。这些结果限该helper输入，不替代结构边调用验证。
- 7次人工CLI：原0.6m窄平台拒绝；原1.6m宽平台built/found且verify verified；当前修订版折返楼梯built/found且verify verified；新反例built/found且verify verified。
- 折返正例已按worker披露修改几何。本次验证的是当前版本，不声称旧正例字节未改或旧几何同样通过。
- 检查员成本单列：cargo2、rustc2、CLI7、生产库探针执行2；自然/ST18/旧世界bake或realize/产品源码改动均0。CLI只产生独立人工审查产物，非世界发布。

## P1：结构边跳过端点与段内测量，真实窄带仍被生产与审核双放行

### 固定人工输入与结果

从G:/difyllmwiki/GlyphWeave/bevy/crates/core/tests/resources/g19r2/wide-directional-deck-control.json派生；16×16地面全0m，平台中心(8,1,8)、6×1.6m，行人带宽要求1.5m，无groundConnections。

仅更改场景标签及端口b坐标：(10,1,8) → **(10.96,1,8.5)**；端口a保持(6,1,8)，平台、阈值、输入其他约束不变。两端点仍位于同一平台上；未声称有相接平台或可豁免的楼梯转接。

独立输入：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g19r3c-final-20260920/endpoint-near-edge.json。

生产结果：built/found，单landing边，长度4.985137911833534m。读回结果：verified、findingCount=0；stored_capabilities_resampled也为true。

直接使用生产几何与生产station_band_detail测量：

| 位置 | 支持带宽m | 结构承托/内部测站 |
|---|---:|---|
| 起点 | 1.6081090038172692 | true/true |
| 路径80% | 0.8040545019086341 | true/true |
| 路径90% | 0.7035476891700571 | true/true |
| 路径99% | 0.6130915577053359 | true/true |
| 终点 | 0.6030408764314763 | true/false |

结构边顶点测量只留下1个测站，返回起点宽1.6081；同一生产库的沿段测量得到最窄0.6030。不是另一套检查员算法的舍入差异，也不是只有端点接触容差的问题：路径80%处已经明显不足1.5m。

### 生产定位

G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/worldgen/engineering_connections.rs：

- band_coverage_vertices约3627–3680只遍历折线顶点；3665–3673对!borne或!interior直接continue。没有确认实际相接部件、有效转接关系或另一承托方。
- refresh_walkable_capabilities约3994消费该顶点测量；并未消费带0.125m沿段走查的band_coverage_detail。
- verifier约6623再次调用同一顶点路径，因此“从保存几何重算”虽真实执行，仍重复同一漏检。

连续地形插值不适合直接替代楼梯逐踏支持，这点合理；但不能因此把所有部件端部和无支持顶点统一跳过，尤其不能把普通landing的段内支持问题当作楼梯特例。

本finding为实际完整生产CLI反例；不推论所有楼梯或地面接驳均错误，不要求重做已确认修好的发布原语。

## 唯一续作

沿原支持合同修复结构边的实际支持覆盖，区分普通平台与有真实转接关系的楼梯；构造和保存包独立重查均覆盖端点、段内支持变化与必要方向。不得仅给本坐标加特殊拒绝，也不得删去失败测站或移端口迁就。

保留本反例与原窄/宽平台，当前合法直跑/折返/挖填正例；有限旋转/反向变体为原任务未完成内容，可并入同一矩阵，不另加多分辨率任务。最终出一次测试、关键CLI、几何视图、清单回执核验，不交确认稿。

按worker自报累计cargo16/20、CLI13/30，最多剩cargo4、CLI17；漏计扣减，检查员调用另列，不重置或默增额度。不足则保存精确partial与剩余动作，不宣告完成。
