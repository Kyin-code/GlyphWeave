# r24：用途约束前置、自然保留地保护与统一地形预检查

> 日期：2026-09-07（本机 Asia/Shanghai）
> 权威仓库：G:\difyllmwiki\GlyphWeave；未修改 F:\modenmapbox。
> 生成修订：worldgen-2026-09-06-r24-zoned-planning。
> 状态：本阶段功能与专项验收完成；**自然地形/人文环境总路线图未完成，三个完整 gate 均 fail**。
> 接续 r23-height-precision-and-terrain-domain.zh-CN.md。本文不是新的完整验收承诺。

## 1. 为什么必须先修这一层

r23 提高了高度精度，但没有解决用地权威性：平原样例规划住宅 21 栋，最终住宅 22 栋；部分开发地块横跨自然/未指定用途边界。自然区如果只阻止建筑中心落入，工程填土仍可能从邻区扩散进来。

本轮优先处理因果顺序，而非继续增加模板或放宽坡度：

1. 自然 raster 与用途分区先确定。
2. 道路/临路住宅在候选阶段接受全支撑范围检查和可填高度约束。
3. 合法候选组成连通路网，再进入开发归属与 BuildingPlan。
4. 旧模板的未规划建筑/基础设施请求留作诊断，不得直接变成实体。
5. 预检查、工程整合和最终 bake 使用同一地形与量化口径。
6. CLI 由实际 natural/final raster 和最终实体独立复核，不能相信保存的 pass。

这不是把已生成的不合格道路/住宅删除后宣称通过；变化发生在规划和实体创建之前。但它也确实减少了部分输出内容，缺失公共服务必须后续真正规划，不能被诊断记录替代。

## 2. 实现与职责拆分

以下路径均相对于 G:\difyllmwiki\GlyphWeave。

| 模块 | 本轮职责 | 不负责什么 |
|---|---|---|
| bevy/crates/core/src/worldgen/zoning.rs（新增） | 验证用途分区；全支撑区拒绝证据；保护填方包络；全分辨率保留地审计 | 不证明法律保护区、地质安全或生态完成 |
| bevy/crates/core/src/worldgen/terrain_context.rs（新增） | 按同一 seed、水体/自然上下文、zoning、carves 和编码精度查询最终高度 | 不是新的自然地貌算法，也不是完整 provider 拆分 |
| bevy/crates/core/src/settlement/proposals.rs（新增） | 记录旧模板未规划的开发请求与明确延后原因 | 不是实际市政/商业/交通设施计划 |
| bevy/crates/core/src/worldgen/road_network.rs | 在连通网络选择前筛选用途与填高可行性；复算方案来源 | 不实现桥隧、分段竖曲线或地区街道形态 |
| bevy/crates/core/src/worldgen/engineered_placement.rs | 工程候选预检与地面审计改用 TerrainContext | 不再自己选择另一种地面精度 |
| bevy/crates/core/src/worldgen.rs | 提前建立 zones；整合统一查询；发布诊断和保护报告 | 尚未完成自然/水文/工程整体解耦 |
| bevy/crates/cli/src/main.rs | 从权威输入重建 zoning、道路计划与保护审计；保护失败进入 gate | 不以报告副本的一致性代替独立重建 |
| bevy/crates/cli/tests/baked_audit.rs | 真实 bake、全保护区像素比较及伪造证据回归 | 不替代最大地图性能与 Godot 运行验收 |

settlement/agriculture.rs、development.rs、landcover.rs 的 RoadNetworkPlan 测试夹具增加新字段；没有变更田块坡度和道路验收上限。

## 3. 用途边界的实际合同

### 3.1 当前保守策略

当前只有 urban、rural 可开发；nature、unassigned 及其他用途在这一层均视为保留。

这是本项目当前生成策略，**不是对现实规划法律的映射**。未指定土地不等于法律意义自然保护区。未来必须增加显式的用途、干预许可、跨越设施许可和生态管理合同，才能支持合法穿越、排水设施、步道等差异化规则。

ZoningMask 校验完整覆盖、范围、唯一 ID、重叠，并绑定输入 hash。用途改变后，旧路网不能沿用。

### 3.2 不能只检测中心点

路基和住宅候选使用包含端点的支撑矩形：中心坐标加减半宽/半深，正向边再多含一个样本。原因是工程与坡度检查会查询边缘高度。归属地块本身仍使用半开区间；两者不是相同的几何合同。

拒绝记录包含 candidateId、kind、范围、conflictingZoneIds 和 reason。

注意：zoningRejections 专门保存支撑范围拒绝；因为填方包络导致的不可行标高仍可能进入既有 rejectedRelief 统计，尚未为每一个高度拒绝提供独立详细追踪。

### 3.3 邻近填土不能污染自然区

保护区样本直接取自然高度；建设区最终高度受以下上界限制：

    finalHeight <= naturalHeight + 0.5 * distanceToNearestProtectedRasterCell

其中距离使用 Chebyshev 距离，单位米。道路/住宅选址预先考虑这一可填高度，避免仅在输出时硬裁地面，造成已规划平台塌边。

**这个上界限制的是新增填土包络，不是自然坡度，也不等同于 50% 的最终工程边坡证明。** 地质、沉降、排水、挡土和切填方体积守恒尚未实现。最终路基/房基和对象规则仍需通过独立检查。

保护审计逐个比较所有保留区高度样本；硬质实体用 footprint 检测是否侵入。旋转结构目前采用保守 AABB，会有误拒绝可能；不应声称是精确旋转多边形保护审计。

## 4. 修复两个真实的失败，而不是隐藏失败

### 4.1 1 km 路灯：预检查与最终地形不一致

第一轮输出 zoning-r24-1000 在 bake 时失败：坐标 (512,260) 路灯坡度约 26.149988%，超出原有 25% 规则。

根因是旧预检查使用旧量化且未含保护边界，最终使用毫米及保护填方约束；并非应提高路灯容忍值。

修复：TerrainContext 统一预检查/工程整合的 zoned source 与存储量化。毫米采用四舍五入到 i32 毫米；legacy 保持旧 i16 四分之一米的截断和范围。实际 encoded payload 对比测试覆盖两种格式与自然边界。

提取 apply_terrain_carves，避免同一个高度查询为取得自然值重复运行自然源。尚有旧模板初始坐标/高度辅助函数没有全部迁移，不能宣称所有 worldgen 查询已经彻底统一。

### 4.2 运河：未规划基础设施绕过开发系统

统一上下文后的 zoning-r24-context-1000 bake/rules 通过，但保护审计仍发现 generated.landuse-canal.200004 侵入保留地。

修复不是删掉这条最终运河，而是在旧模板 add() 请求进入实体创建之前，将 canal、railway、parking_lot 与未规划建筑一起记录为 DevelopmentProposal；它们必须先取得基础设施/土地归属计划，才能真正生成。

已有显式 authored landmarks 仍走恢复与审计流程，不能因为作者指定或 generated 前缀而免检。保护区 authored 房屋的检测有单元回归；本轮新增真实 bake 集成测试没有覆盖所有 authored 地标输入组合。

## 5. DevelopmentProposal 的能力边界

scene.json 新增 developmentProposals，并发布 development-proposals.json。

每条记录包含 ID、类型、坐标、尺寸、decision、reason。decision 为 deferred-before-entity-generation：

- 建筑原因：requires-building-plan-and-legal-frontage-parcel。
- 运河/铁路/停车场原因：requires-infrastructure-and-land-ownership-plan。

它仅代表生成请求历史：

- 不增加已实现建筑数量、服务覆盖率或土地实现率。
- 不能证明学校、市政、教堂、市场等需求已满足。
- CLI 暂未独立重新生成请求历史，所以它不是验收权威。
- 后续应转为带需求、容量、临路/步行入口、服务半径、适用地区和土地许可的真正规划输入。

## 6. 最终实际生成结果

同 r23 的三个输入；没有覆盖中间失败目录。

输入：bevy/work/height-r23-{512,1000,plains}.manifest.json。
最终输出：bevy/work/zoning-r24-final-{512,1000,plains}。
汇总：bevy/work/zoning-r24-evidence.json。

| 指标 | 512 m 起伏 | 1000 m 起伏 | 512 m plains |
|---|---:|---:|---:|
| 已实现/规划道路段 | 5/5 | 9/9 | 29/29 |
| 已实现/规划住宅 | 1/1 | 3/3 | 18/18 |
| 未规划建筑 | 0 | 0 | 0 |
| 开发用途冲突 m² | 0 | 0 | 0 |
| 保留区面积 m² | 65536 | 223796 | 49152 |
| 保留区变动高度样本 | 0 | 0 | 0 |
| 侵入保留区硬质实体 | 0 | 0 | 0 |
| 用途支撑范围拒绝记录 | 18 | 104 | 11 |
| 延后开发请求 | 15 | 18 | 16 |
| 实际农业田块 | 0 | 1 | 23 |
| 实际地表 realization m² | 208 | 1208 | 18560 |
| 未实现地表 m² | 258880 | 992552 | 218992 |
| 独立对象规则通过 | 16/16 | 32/32 | 51/51 |
| 道路专项审计 | pass | pass | pass |
| 自然保留地专项审计 | pass | pass | pass |
| **完整 gate** | **fail** | **fail** | **fail** |

平原相对 r23：道路 32→29，规划住宅 21→18，最终住宅 22→18，田块 26→23。不能宣传所有内容/覆盖都增长；本轮优先保证新增内容有合法来源和不侵犯保留区。

这里“自然保留地审计通过”只证明未改写高度且未被指定硬质实体侵占，并不证明生态植被/湿地/自然公园已实际生成，因此没有增加 conservation realization 面积。

## 7. 为什么整体 gate 仍然失败

1. 大片剩余地尚无实际开发/生态实现，DevelopmentAudit 仍 incomplete。
2. LandcoverPlan 候选不等于真实表面 realization，LandcoverAudit 仍 incomplete。
3. 512 m 起伏场景农业仍 0；当前道路可达性/坡度/工程能力不足，不能降低田块阈值冒充解决。
4. plains 的 gate 仍报告 urban share 约 0.948、rural 0，而目标约 0.040/0.808。用途配额、地块归属和实体类别的统计口径及真实产出均需后续检查；不能仅把 residential_home 重分类或把预留面积填入 realized 来消除警告。
5. 公共设施请求目前延后，没有完成地区人文服务组合。

本轮没有删 gate 条件，没有让请求/候选充当实际实现，没有为数量减少编造覆盖率。

## 8. 测试与反例证据

Rust 全 workspace：Core 299、App 44、CLI 单元 6、真实 bake 集成 13，合计 **362 项通过**。最终日志：bevy/work/zoning-r24-final-tests.log。
专项：bevy/work/zoning-r24-integration-verified.log。

新增/扩展覆盖：

- 分区缺失、重叠、重复 ID、输入改变与边缘支撑侵入。
- 保护填土包络与精确量化预检查一致性。
- 平地非空路网在用途约束后仍产生真实道路/住宅。
- 修改 zoning hash 或清空拒绝证据，独立路网重建拒绝。
- 单个保护像素改动、保护区 authored 房屋 footprint 入侵单元检查。
- 真实毫米平原 bake：逐个读取 payload，比较所有保留区样本；规划与真实住宅匹配且非空。
- 将保存的保护报告伪造成 fail，CLI 仍依据真实数据算 pass。
- 将真实保护区像素改动 1mm 并写伪造 pass，CLI 拒绝；对相同实际 payload 调用独立保护审计，恰好检出 1 个改动样本。
- 同时篡改 scene 和独立道路文件的 zoningHash/zoningRejections，仍在来源重建阶段拒绝。

重要测试过程说明：首版新集成测试错误地预期“像素篡改后 quality-report 会成功输出保护 fail”。实际地表来源也绑定完整 final raster，因此 CLI 在独立 landcover realization 阶段先拒绝，不写新报告。保留这一严格行为；调整测试同时断言 CLI 非零退出与公共保护审计精确检出，而非放宽验证。首轮失败日志 zoning-r24-integration.log 保留。

pnpm doc-tree:check 通过，受本轮影响的已跟踪文件 git diff --check 未报空白错误。

前端：Vitest 170 项、Node 高度/住宅 mesh 合同 7 项通过；pnpm typecheck 通过；pnpm lint 退出 0，但有既有 unused/self-assign 等警告，不应称零警告。

## 9. HTML 实际查看

当前平原预览： http://127.0.0.1:8838/preview/?view=farm

预览使用 bevy/work/zoning-r24-preview-runtime/glyphweave.exe 独立副本，不锁构建目标，不影响旧 8837 预览。

- 截图：bevy/work/zoning-r24-final-plains/zoned-farm.png。
- 浏览器证据：bevy/work/zoning-r24-preview.log。
- 检查脚本：scripts/check-preview-camera.mjs。
- 检查实际相机朝向、高度编码/terrain mesh 对齐、真实农业 mesh 和页面运行错误。

已实际查看截图：道路、住宅、田面可见，但地面偏白、草密度过高、整体外观仍较粗糙。通过浏览器合同不等于视觉验收；本轮不声称解决材质、自然美感或真实城市场景。

## 10. 后续必须完成与可选项

### 必须，按依赖顺序

1. **用地语义和统计闭环**：从 zone→合法地块→建筑/地表实现建立统一可复算统计；区分目标、可行容量、预留、拒绝、实际 realization，调查当前农村 0 的具体原因。
2. **公共设施和基础设施规划**：消费需求而非复用旧模板直接 add；道路/步行连接、容量、服务范围、土地许可缺一不可。延后请求必须向用户暴露缺口。
3. **生态/保留地 realization**：自然高度保全只是必要条件。植被、湿地、缓冲带、步道许可、管理边界需要可审计实现；不可简单全涂绿色算完成。
4. **自然 provider 与工程状态解耦**：TerrainContext 已解决关键一致性问题，但 WaterGeometry 仍承载自然背景上下文，旧 helper 尚存。拆分时必须保持同 seed、origin、terrainDomain 和精度一致。
5. **山地工程能力**：桥隧、道路纵断面、切填方/挡土/排水有独立预算和可行性证明后，再提升困难地貌的开发覆盖。
6. **最大 6×10km 性能**：当前填土距离逐查询扫描保留矩形；完整保护审计逐像素；尚未跑最大图 bake/峰值内存。要做距离场/空间索引候选基准，但引入 chunk halo 后也必须保证跨块一致。
7. **自然地貌与地区形态**：本轮没有新增真实侵蚀，也没有完成中美欧日港建筑分布规则落地。继续遵循原研究文档与来源证据，不能由“地形未污染”推导“地形足够自然”。
8. **跨适配器与最终验收**：Godot r23 解码仍无运行验证；不同种子/范围、多场景、大地图和 HTML 视觉均需单列证据。

### 可选、应在正确性稳定后推进

- 更精确的旋转 footprint/多边形保护判定，减少保守拒绝。
- 细分合法穿越许可、步道、生态管理道路和开发例外，但必须显式合同与审计，不是白名单免检。
- 对每条填高拒绝增加可解释的限制区/高度证据。
- 管理用预览展示 proposal 缺口与服务覆盖，不把诊断记录渲染成真实设施。
- 材质、光照、植被 LOD 和地区建筑资产美化。

## 11. 来源与许可证边界

本阶段为项目自己的 Rust 规划/审计/上下文重构与测试；未复制 Mapgen4 或 symbios-ground 代码，未新增外部依赖或资产。没有将现有近似谷地处理重新命名为真实侵蚀。

因此本文不作任何新的外部许可证兼容性结论，也不宣称全项目开源许可证清理已完成。后续引入自然算法必须先固定仓库/commit，核对源码和依赖的 LICENSE/NOTICE，并区分算法思想、直接移植、运行时依赖和资产数据来源，更新 THIRD_PARTY_NOTICES.md。

## 12. 结论

r24 解决了“自然保留边界不参与规划、预检地面与最终地面不一致、旧模板绕过开发来源”三个具体问题。现在可以在这些已验证约束上继续做人文与自然系统，而不是继续累加无来源的建筑。但整个路线图远未完成，完整 gate 和剩余覆盖缺口仍如实保留。
