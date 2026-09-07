# r27：设施预留与住宅最低供给闭环（settlementProgram v2）

> 权威仓库：G:\difyllmwiki\GlyphWeave；未修改 F:\modenmapbox。
> 修订：worldgen-2026-09-06-r27-joint-frontage。
> 接续 r26-explicit-service-program.zh-CN.md。
> 状态：本轮示例的 4 个设施和至少 18 栋住宅均真实实现；**完整自然/人文路线图仍未完成，完整 gate 仍 fail**。

## 1. 这次解决的问题

r26 的 v1 算法先放普通住宅，再追加设施。平原示例生成了 2 家便利店和 1 座教堂，但 40×30m 学校没有找到空地。

仅把顺序换成设施优先也不够：本轮中间版确实放下了学校，却使住宅从 18 栋减少到 16 栋。那只能证明设施优先分配有效，不能证明住宅目标也满足。最终实现不接受“把预期改成 16”作为修复。

最终 v2：

    明确设施需求 + 住宅最低数量
      → 自然地形/用途约束与原道路计划
      → 设施预留候选
      → 保留不冲突的原住宅候选
      → 按相同约束寻找替代住宅候选
      → 生成实际实体、地基、地表和入口
      → 独立验证设施需求与住宅最低供给

这是有界、确定性的优先级规划和住宅补足，不是全局最优城市规划求解器。

## 2. 输入版本与兼容性

### v1 保持原行为

examples/service-program-plains.manifest.json 仍为 version 1：普通住宅先行、设施追加。它依然有 18 栋住宅、3 个设施、1 个学校缺口。没有悄悄把旧输入重解释成“允许减少住宅”。

### v2 增加住宅最低供给

新示例：examples/service-program-joint-plains.manifest.json。

    "settlementProgram": {
      "version": 2,
      "housingMinimum": { "scene-0": 18 },
      "requests": [
        { "id": "village-shop", "sceneId": "scene-0",
          "kind": "convenience_store", "count": 2,
          "widthM": 12, "depthM": 10, "heightM": 4 },
        { "id": "community-church", "sceneId": "scene-0",
          "kind": "church", "count": 1,
          "widthM": 20, "depthM": 28, "heightM": 10 },
        { "id": "primary-school", "sceneId": "scene-0",
          "kind": "school", "count": 1,
          "widthM": 40, "depthM": 30, "heightM": 10 }
      ]
    }

- housingMinimum 按 sceneId 配置，必须引用已有场景。
- v2 没有给某场景显式下限时，默认使用该场景普通规划得到的住宅数量，不能因为设施占地就自动降低下限。
- 下限是建筑数量，不是人口、家庭数、床位、学生容量或精确数量上限。
- 值为整数 0–65536；0 表示没有最低数量要求，不表示必须清空所有住宅。
- v1 不接受非空 housingMinimum，避免模糊语义；设施类型/尺寸/数量约束仍使用 r26 的严格解析，不自动缩小。
- 只给住宅最低数量、没有设施请求的 v2 也会检查供给；不可把空 requests 当成忽略住宅要求的理由。
- 无相关请求且无该场景 housingMinimum 时，不改变原场景规划。

Agent 仍只表达类型、尺寸和数量；没有获得坐标、跳过规则或强制填入保护区的权限。

## 3. 代码职责与约束

新增 bevy/crates/core/src/worldgen/joint_frontage.rs：

1. 使用原自然 raster、道路、zoning 和 program，预留设施地块。
2. 检查原住宅与设施是否互相冲突，既检查地块，也检查**两个方向的入口走廊**。地块本身不重叠，不代表道路入口未被堵住。
3. 保留合格的原住宅候选；未选中的原候选 IDs 进入 displacement 记录。
4. 对最低供给缺口，复用 services 的完整支撑高度、填方包络、道路/地块、保护用途、水体、入口走廊检查，寻找 24×24×8m 住宅替代位置。
5. 内部住宅搜索请求不写入用户 serviceOutcomes，也不算成新增公共设施。最终住宅仍是 program=None 的住宅 site，使用独立稳定的 residential-recovery IDs。
6. 若候选搜索仍不能补足，保留 unfilledResidential，不能改低 residentialMinimum。

本轮没有改变道路长度、连通网络、6m 填方上限、保留区保护、对象坡度或通路规则。

当前解法先预留设施，再补住宅；若另一种设施摆放能更好地满足全部目标，当前算法并不保证会回溯找到它。多尺度道路/街区、候选空间索引和更强的联合优化仍需后续实现。

## 4. 候选调整不等于事后删除实体

v2 重新选择的是**尚未生成的候选**。已有 r26 输出目录未被修改，新场景输出到新目录。

本例：

- 原普通规划提供 18 个住宅候选。
- 16 个原候选原位保留。
- 2 个原候选因设施/入口冲突未选用。
- Rust 找到 2 个新的住宅候选并实际生成。
- 最终住宅仍为 18 栋，不将原候选数量或替代候选数量重复计入面积/供给。

这不是对居住者、产权或旧世界保存状态进行搬迁模拟；原候选 IDs 与替代 IDs 的记录用于生成来源追踪，不代表真实人口迁移。

还修复了一条旧路径：最终入口搜索失败时，worldgen 曾把所有 generated 前缀建筑当作可删除候选。现在 authored 地标和 DevelopmentPlan 建筑都受保护，不能在最终阶段因为通路失败被删掉以改善门禁；其失败继续交给规则与独立审计处理。只有未规划的可选过程候选沿用原拒绝机制。

## 5. 来源记录与独立审计

RoadNetworkPlan 新增可选 jointLayout：

- residentialMinimum；baselineResidentialCount；
- retainedResidentialIds；displacedResidentialIds；replacementResidentialIds；
- unfilledResidential；recoveryRejections；model。

同时发布 joint-layout.json 和 joint-layout-audit.json，供查看使用。

CLI 先从 manifest、自然高度与用途重建整个道路/设施/住宅计划，再执行 jointLayoutAudit：

- 实体 ID 必须唯一、实际类型/尺寸/变换/入口与计划一致；不自动修复缺失实体。
- 实际住宅必须具有通往指定道路的已验证入口。
- 实际住宅数量低于最低要求或设施没有满足时，联合审计失败/不完整。
- 保存的 joint-layout-audit.json 即使声称 pass/零缺口也不能覆盖实际数据。
- 只把 jointLayout.unfilledResidential 改成 0 不会增加任何供给。
- 同时篡改 scene 与道路计划副本里的下限、替代 IDs 或候选坐标，也必须在 manifest/natural 重建阶段被拒绝。

严格 gate 新增对 jointLayoutAudit 的检查，未满足住房下限有明确失败项。它没有替代 RoadNetworkAudit、DevelopmentAudit、ProtectionAudit、LandcoverAudit 或对象规则检查。

## 6. 最终实测（权威输出）

**只使用下列 verified 输出作为本轮结论依据：**

- v2：bevy/work/joint-r27-verified-plains。
- v1 对照：bevy/work/joint-r27-verified-v1。
- 无 program 对照：bevy/work/joint-r27-verified-baseline。
- 汇总：bevy/work/joint-r27-verified-evidence.json。

| 指标 | r26/v1 对照 | r27/v2 |
|---|---:|---:|
| 已实现便利店 | 2 | 2 |
| 已实现教堂 | 1 | 1 |
| 已实现 40×30m 学校 | 0 | **1** |
| 实际设施 / 请求设施 | 3/4 | **4/4** |
| 实际住宅 / 本例下限 | 18/18（旧路径保留） | **18/18** |
| 原住宅候选原位保留 | 18 | 16 |
| 新住宅替代候选实现 | 0 | 2 |
| 已实现道路 | 29 | 29 |
| 最终规划建筑总数 | 21 | 22 |
| 未规划建筑 / 用途冲突 | 0 / 0 | 0 / 0 |
| 保留区高度改写样本 | 0 | 0 |
| 农业田块 | 21 | 24 |
| 全图实际用途面积 m² | 43160 | 46440 |
| 全图未实现 m² | 218984 | 215704 |
| 设施专项 | incomplete | pass |
| 联合住房与设施专项 | 不适用 | pass |
| **完整 gate** | **fail** | **fail** |

v2 的 4 个设施和 18 栋住宅都具有实际实体和指定道路入口。自然保留区仍为 49152m²，没有因学校或替代住宅而改写高度。

农业数量变化来自地块分区、实际占用和通道重新求解，不应推导为所有地形/种子都会增加农业。住房最低数量也不证明城市居住密度符合任何地区标准。

### 对照兼容性

本轮新生成的 v1 与 r26 v1 对比，以及无 program 场景与 r26 对应基线对比：entities、RoadNetworkPlan、DevelopmentPlan、LandcoverPlan、SurfaceRealization 相同；各自 5 类 chunk payload 逐字节一致。

此处不比较必须变化的 generatorRevision/世界修订，也不宣称任意旧世界或任意 seed 全部位级一致。

## 7. 中间失败和被放弃的做法

- 初版“设施优先”只有 16 栋住宅，不作为最终联合目标完成证据；它没有住宅补足机制。
- 一次未限定到具体函数的测试替换误改了旧 DevelopmentAudit 的 incomplete 断言，已恢复；也不采用把旧服务测试住宅期望降到 16 的做法。最终 v1 原测试恢复原来的 18 栋/学校缺口约束，v2 使用独立的新测试证明 18 栋和 4 个设施。
- 早期一次生成调用了修改前的 CLI，因此旧输出不能证明新代码有效；之后明确重新构建，并使用全新 verified 输出。
- 部分早期输出使用临时或尚未更新的修订标识，均不是当前证据。不要使用 joint-r27-service-plains、joint-r27-service-plains-rebuilt 或 joint-r27-service-plains-final 替代本节列出的 verified 目录。

旧失败日志保留；最终没有通过删除门禁、放宽坡度或改低住宅目标来清理失败。

## 8. 验证与 HTML

Rust workspace：Core 311、App 44、CLI 单元 6、真实集成 17，合计 **378 项通过**；日志 bevy/work/joint-r27-v2-final-tests.log。

新增/扩展验证：

- 学校预留后住宅在另一合法地块补足，非零 origin、原道路保持不变、确定性重建。
- 地块不重叠但入口走廊被阻挡时拒绝两个方向的冲突。
- 无路/不可开发场景无法满足住宅要求时，保留缺口，不信任保存的零缺口。
- 真实 v2 bake：18 栋住宅、4 个设施、原尺寸学校、道路/保护/归属一致。
- 同时修改两份计划的住宅下限、替代 IDs、替代住宅位置仍被拒绝。
- 全自然用地区域的最低 1 栋住宅请求无法实现，严格 gate 明确失败。
- 最终入口失败的已规划建筑不进入可选候选删除集合。
- 旧 v1、无 program、r23–r26 篡改/高度/地表回归继续通过。

前端 Vitest 170 项、Node 高度/住宅/设施合同 27 项通过。浏览器检查分开保留 v1 未满足需求与 v2 全设施实现预期，不能简单去掉旧 unfilled 断言。

阶段预览端口 8842：services 视图优先聚焦学校，ownership 视图显示住宅最低数量、保留候选、替代候选与未补足数量。表格明确为规划诊断，不代替 CLI 实际验收。

浏览器证据与截图：

- joint-r27-preview-services.log / joint-services.png：4 个设施的实际网格、尺寸、门面位置和来源唯一性；隔离浏览器负对照仍能检出旧重复兜底绘制。
- joint-r27-preview-housing.log / joint-ownership.png：实际 18 个住宅网格，包括两个替代住宅；然后验证 ownership 的联合规划表。
- joint-r27-preview-camera.log / joint-camera.png：相机朝向和实际毫米高度 mesh。

以上日志位于 bevy/work，截图位于 bevy/work/joint-r27-verified-plains。已实际查看两张截图：学校原尺寸体量和联合住宅规划表可见，但灰色设施仍是简化体量，草过密、地面偏白等旧视觉问题仍在；浏览器合同通过不等于最终美术验收。

最终 lint/typecheck/doc-tree 退出 0，lint 仍有既有警告，不能称为零警告。

## 9. 如何复现

从仓库根目录执行，选择尚不存在的输出目录：

    bevy\target\debug\glyphweave.exe generate-world examples\service-program-joint-plains.manifest.json bevy\work\my-joint-world
    bevy\target\debug\glyphweave.exe world-quality-gate bevy\work\my-joint-world
    bevy\target\debug\glyphweave.exe preview bevy\work\my-joint-world 8843

示例完整 gate 仍预期返回非零，因为还有大片土地、自然生态和原路线图缺口。这不是本轮住宅/设施审计失败。

## 10. 仍未完成的工作与限制

1. 当前是沿现有道路做设施预留与住房补足的启发式方法；没有服务/住宅/农业的全局最优保证，也未联合重排道路。
2. 道路仍是 64m 级低地网格；没有完成大公共地块、多尺度街区、桥隧、立交或更复杂竖向工程。
3. 住宅仍使用 24×24×8m 原型；没有实现地区住宅形态、人口密度/家庭规模、设施容量与服务半径推导。
4. 森林、湿地、自然公园等生态地表仍未实际实现；本例自然区域实现面积为 0，不能把“高度没被改写”算成生态完成。
5. 候选补足复用 r26 有界搜索，每条内部住房搜索同样有候选预算；大地图空间索引、时间/峰值内存基准尚未完成。65536 的配置上限不是性能或可行性保证。
6. 自然 provider 拆分、更自然的原始地貌/侵蚀候选及源代码/许可证审计、Godot 运行验证和最终美术仍是主路线图任务。
7. 替代地块的 ID 是新生成来源，不实现旧世界存档或真实居民迁移。不能把本轮当作已有城市改造模拟器。

## 11. 来源与许可证

本轮为项目自己实现的 Rust 规划/补足/审计及只读 HTML 展示；复用现有服务候选、地形与入口检查，没有复制外部实现、引入依赖或下载资产。

没有新增 Mapgen4/symbios-ground 兼容性判断，也没有把旧近似地形处理称为真实侵蚀。全项目开源审计和最终自然地形目标保持原范围，不因这个场景专项通过而缩减。
