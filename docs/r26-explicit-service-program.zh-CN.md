# r26：显式公共设施需求、可变尺寸临路建筑与 HTML 来源一致性

> 权威仓库：G:\difyllmwiki\GlyphWeave；未修改 F:\modenmapbox。
> 生成修订：worldgen-2026-09-06-r26-service-program。
> 接续 r25-purpose-cover-ledger.zh-CN.md。
> 状态：已接通显式需求到实际建筑；**尚未完成整个人文/自然路线图，完整 gate 仍 fail**。

## 1. 本轮真正增加了什么

r25 修正了统计，但没有增加真实产出。本轮增加了一条 Rust 主导的设施生成路径：

    显式设施需求 → 输入校验 → 已规划道路/自然地形/用途约束
    → 保留已有住宅的可变尺寸选址 → BuildingPlan/土地归属
    → 地基与实际实体 → 实际入口通路/地表 → CLI 独立重建与审计

没有读取 DevelopmentProposal 旧模板历史来推断用户需求；没有把大设施缩成 24×24m 住宅；没有事后删除既有道路/住宅来让场景变绿。

真实平原用例请求 4 处设施：2 家便利店、1 座教堂、1 所学校。其中 **2 家便利店和 1 座教堂已实际生成，且具有指定道路入口连接**；40×30m 学校在本轮有界候选搜索中未找到可行地块，保留未满足记录。

这不表示学校在该地形上数学意义不可能安置，只表示当前路网/住宅优先布局和候选求解器没有找到解。

## 2. 输入合同：Agent 提需求，Rust 决定位置

新增 style.settlementProgram，可省略；没有这一字段的旧场景不自动增加设施。

示例：

    "settlementProgram": {
      "version": 1,
      "requests": [
        {
          "id": "village-shop",
          "sceneId": "scene-0",
          "kind": "convenience_store",
          "count": 2,
          "widthM": 12,
          "depthM": 10,
          "heightM": 4
        }
      ]
    }

- id 全局唯一，ASCII 字母/数字/下划线/短横线，最长 64 字符。
- sceneId 必须引用已有场景，避免多场景重复投放同一要求。
- 不接受坐标、旋转、忽略碰撞等额外字段；unknown fields 拒绝。
- widthM/depthM 使用偶数整数米；按原尺寸规划，不能裁小或强制缩放。
- count 为 1–32；最多 64 条请求、合计最多 64 处设施。
- 请求数组顺序决定当前求解优先顺序；这不是公平分配或全局优化。
- 当前支持 strict/release 的无结构水体 road-first 场景；不支持时明确报错，不悄悄退回旧模板。

当前类型与工程支持范围：

| kind | 宽 m | 深 m | 高 m |
|---|---:|---:|---:|
| convenience_store | 6–40 | 6–30 | 3–12 |
| school | 30–128 | 26–120 | 4–30 |
| church | 12–80 | 12–80 | 3–40 |
| town_hall | 30–128 | 24–120 | 8–60 |
| market | 36–128 | 30–128 | 3–20 |

这是与现有项目资产合同衔接的实现范围，**不是实际建筑法规或地区典型尺寸数据**。未支持类型报错，不用某个相近类型代替。

此阶段没有人口/学生数/营业容量/服务半径字段，也没有凭建筑存在推导容量。宗教设施仅在显式请求时走这条路径，不按国家刻板印象强行插入。

## 3. 代码拆分与合同迁移

### 新模块

bevy/crates/core/src/worldgen/services.rs：

- ServiceProgram / ServiceRequest：显式输入与严格解析。
- SiteProgram：合法选址关联到原请求、类型和高度。
- ServiceOutcome：原请求、真实计划 site IDs、未满足数量、拒绝原因计数。
- 设施候选求解与 ServiceAudit：最终实体精确性和指定道路入口检查。

### 既有模块重构

1. worldgen/road_network.rs：新增 plan_with_program / verify_with_program；保留原无 program 包装入口。RoadsideSite 的尺寸、kind、height 和 front anchor 不再强制住宅常量。
2. settlement/development.rs：按旋转后的真实宽深生成 footprint 和外扩 2m 地块；归属面积和原道路父关系仍独立验证。SettlementGraph 不再把学校/商店等标成 residential。
3. worldgen.rs：校验/调用 program，规划后恢复精确实体；占用索引使用真实旋转尺寸；便利店进入建筑地基 carve；发布结果与审计。
4. cli/main.rs：从 manifest 的 program、自然 raster 和 zoning 重建完整计划；保存的成功报告不是接受证据。
5. adapters/html：增加规划设施的精确体量 renderer 和 services 相机入口；阻止旧兜底 renderer 再画同一个规划建筑。

RoadNetworkPlan 暂时仍以 roadsideSites 承载住宅和设施地块，增加 serviceOutcomes。服务求解逻辑已独立，但这个历史数据接口尚未完全拆成 RoadPlan / GroundSitePlan / BuildingProgram；不能宣称总体架构解耦完成。

新增可选字段缺省为空且不序列化，无 program 路径保持原计划语义。quality-report 沿用 r25 的 version 2，新增 serviceProgramAudit。

## 4. 候选约束与工程边界

在自然高度 raster 与用途已知后，沿现有连通道路的两侧做有界候选搜索：

- 使用原请求的真实宽、深、高和四个正交朝向。
- 保留已选住宅、道路、设施地块及已有直达入口走廊。
- 检查全支撑范围，包含端点样本；不得跨保留用途/水体/场景边界。
- 道路、既有地块与入口走廊均有占用拒绝。
- 建筑地面采用同一已规划路网标高；完整支撑区检查自然高度、最大 6m 填方与 r24 保护填方包络。
- 新设施不能把已有入口走廊堵住；自身直达走廊也检查保留地、水体、既有地块及是否先碰到另一条道路。
- 最终仍由真实 BakedTerrain、对象规则和 AccessGraph 验证；候选走廊不代替最终可步行通路。

候选沿道路使用有限偏移与三档退距；每条请求最多检查 65536 个候选。超出预算明确记录 candidate_search_budget_exhausted，不能把预算耗尽称为全局不可行证明。

本轮不允许通过切山、桥隧、自动搬迁住宅或降低坡度阈值来满足设施。道路仍是原 64m 级低地网格，这也是较大公共建筑难以找到地块的主要限制之一。

## 5. 什么才算设施实现

三个不同层级不能混为一谈：

1. ServiceRequest：用户要求多少、什么类型、什么尺寸。
2. ServiceOutcome 的 plannedSiteIds：Rust 找到了多少合法候选。
3. ServiceAudit 的 realizedWithAccess：最终实体精确符合计划，且有 verified assigned-road access。

此外整体接受还必须满足独立 RoadNetworkAudit 的支撑高度/填方、DevelopmentAudit 的来源归属、ProtectionAudit 和对象规则。ServiceAudit 本身不是地基工程或公共服务容量证明。

未满足数量不会清零，也不会把旧模板的相同类型建筑算来顶替。输出包含：

- scenes/<scene>/road-network-plan.json 内的 serviceOutcomes 与 program 关联；
- scenes/<scene>/service-program-outcomes.json（便于查看的副本）；
- scenes/<scene>/service-program-audit.json（保存的诊断）；
- quality-report 的独立 serviceProgramAudit；
- strict gate 对未满足/未实现设施的明确失败项。

CLI 不读取保存的 audit/outcomes 副本来决定是否通过，先重建权威计划，再核算最终实体和通路。即使同时篡改 scene 与道路计划副本，也不能绕过原 manifest/natural 重建。

## 6. 最终真实生成结果

可重现输入：examples/service-program-plains.manifest.json。
最终输出：bevy/work/services-r26-final-plains。
汇总：bevy/work/services-r26-evidence.json。

| 请求 | 原尺寸 m | 请求数量 | 已计划 | 有指定道路入口的实际设施 | 未满足 |
|---|---|---:|---:|---:|---:|
| 便利店 | 12×10×4 | 2 | 2 | 2 | 0 |
| 教堂 | 20×28×10 | 1 | 1 | 1 | 0 |
| 学校 | 40×30×10 | 1 | 0 | 0 | 1 |

- 道路：29 段，全部保留且独立审计 pass。
- 原住宅：18 栋，全部保留。
- 最终规划建筑：21 栋；未规划建筑 0；开发用途冲突 0。
- 自然保留区：49152m²，高度改写样本 0，侵入硬质实体 0。
- 设施审计：requested 4 / planned 3 / realizedWithAccess 3 / unfilled 1，状态 incomplete。
- 农业田块：23→21；新增设施占用土地后，农业候选与通道重新求解，不能宣传农业也增加了。
- 总实际用途面积：43152→43160m²，仅增加 8m²；新增设施与农业产出存在真实取舍，不以建筑数量冒充大幅覆盖增长。
- 全图仍未实现：218984m²。
- **完整 gate 仍 fail**：学校需求未满足，大量土地/地表仍未实现。

学校候选具体拒绝计数见 serviceOutcomes；它没有被缩小或换成一间住宅。基础候选排序、地块占用、路网粒度都仍有改进空间。

### 无 program 回归

bevy/work/services-r26-final-baseline 与 r25 的平原输出对比：entities、RoadNetworkPlan、DevelopmentPlan、LandcoverPlan、SurfaceRealization 相同，5 类 chunk payload 逐字节相同。

这个证明只覆盖已跑的平原基线，不是所有旧世界的无限兼容保证。便利店补齐地基 carve 会影响包含该类型的旧场景，不能宣称任意场景全部位级不变。

## 7. HTML 验收中发现并修复的问题

### 7.1 同一个便利店被画了两次

初版检查只确认 planned-service 网格存在、尺寸正确。实际截图出现了额外红屋顶：app.js 后面的 coastal/prototype 兜底循环又为便利店创建了一套旧几何。

修复：

- 建立 plannedBuildingIds，后续兜底循环不再渲染这些实体。
- 规划设施、coastal 资产和 prototype 根节点记录 sourceEntityId。
- 浏览器不只验证计划组存在，还验证每个请求来源恰好只有一个渲染根节点。
- 在隔离浏览器中临时移除这一 guard 做负对照：3 个设施出现 5 个来源根节点，回归检查能检出；不改磁盘世界数据。

旧 services.png 保留作为中间结果，**不是最终截图**。权威截图为 services-verified.png。

### 7.2 浏览器断开导致整个预览进程退出

实际进程以 HTTP request ended before headers 退出，随后浏览器报 connection refused。不是页面尚未加载，也不是可以盲目重启掩盖的问题。

修复：提取 serve_preview_connection，将 EOF、取消请求、无效反馈等错误限制在当前连接；设置读写超时，提前限制超大请求。监听器不因一个坏连接退出。

真实子进程回归覆盖：空连接、截断 header、无效反馈 JSON、超大 Content-Length，之后正常页面请求仍为 HTTP 200。

这仍是本地开发预览服务，不是生产 HTTP 服务器；串行连接处理、吞吐与完整协议行为未在本轮重写。

### 7.3 示例相对路径

初版示例 rulesDir 写成 assets/objects，CLI 按 manifest 所在目录解析，实际找到了 examples/assets/objects 并失败。已修为 ../assets/objects 并重新生成验证。首轮失败日志保留，未改规则加载器放宽错误。

## 8. 当前可查看的 HTML 与运行方式

当前阶段预览端口 8840，services 视图聚焦真实设施；使用独立 runtime 可执行副本，不锁构建目标。

- 权威截图：bevy/work/services-r26-final-plains/services-verified.png。
- 设施几何/来源唯一性及负对照：bevy/work/services-r26-preview-verified.log。
- 相机及毫米高度 mesh 对齐：bevy/work/services-r26-preview-camera-verified.log。
- 服务器回归：bevy/work/services-r26-preview-server-test.log。

从仓库根目录运行，输出必须选择新目录：

    bevy\target\debug\glyphweave.exe generate-world examples\service-program-plains.manifest.json bevy\work\my-service-world
    bevy\target\debug\glyphweave.exe world-quality-gate bevy\work\my-service-world
    bevy\target\debug\glyphweave.exe preview bevy\work\my-service-world 8841

这个示例预期完整 gate 返回非零，不能把非零当作安装失败。

已实际查看修复后的截图：规划体量和门口可见，重复便利店已消失；但草过密、地面偏白、设施建筑仍是简单体量，教堂不具备完整建筑风格或内部空间。不是最终美术/现代城市视觉验收。

## 9. 测试范围

最终 Rust workspace：Core 307、App 44、CLI 单元 6、集成 15，合计 **372 项通过**，日志 bevy/work/services-r26-verified-tests.log。前端 Vitest 170 项通过，lint/typecheck/doc-tree 退出 0；lint 仍有既有 unused/self-assign 警告。

新增核心回归：

- 显式 40×30m 学校在可行的独立路段夹具上形成原尺寸建筑/44×34m 地块，既有住宅保留。
- 保留用途阻止选址且保留 80×60m 原请求，不缩小或隐藏未满足。
- 设施实际实体、尺寸、指定入口缺失/篡改不能算实现。
- 错误类型、重复 ID、奇数尺寸、零数量、额外字段被拒绝。

真实 bake 集成：精确尺寸、原 18 栋住宅保留、计划与实际设施/入口一致；保存报告伪造无效；同时篡改两份计划的尺寸或未满足数量，独立来源重建仍拒绝。另新增真实预览子进程的断开/坏请求存活回归。

Node 渲染合同共 27 项：旧高度/住宅 7 项，新增 5 类设施×4 朝向共 20 项；检查旋转后宽深、上下高度、真实门面位置、不虚构校园/停车场/院落。实际浏览器再验证 baked 场景，不用单元测试替代运行验收。

## 10. 下一步仍必须做什么

1. **需求驱动的联合布局**：现在是既有路网/住宅保留后追加设施，并非全局土地优化。必须在规划阶段协调住宅、公共设施和农业目标；不能在最终实体层随意删建筑/道路以消除矛盾。
2. **更灵活的街区与场地**：当前 64m 路网和有限候选限制学校等设施。下一步应以需求保留公共地块、支持多尺度街区，再验证道路及住房需求的真实取舍；不只是继续扩大候选次数。
3. **人口、服务容量与地区逻辑**：尚未从人口/聚落规模/中美欧日港地区资料推导设施数量，也未证明服务半径、学生容量、营业/供给能力。不能把本轮 3 个建筑当作完整社会服务网络。
4. **生态、农村和自然核心**：大片土地未实现，学校仍缺失；自然 provider 拆分、侵蚀/沉积候选、生态表面、细分用途配额都仍在主路线图中。
5. **最大图性能**：候选搜索会遍历道路/已占地和自然样本；尚无最大 6×10km 耗时/峰值内存证明。需要空间索引和基准，不把候选数上限称为性能优化完成。
6. **架构与跨适配器**：继续拆开道路与地块程序的数据合同，Godot 实际运行及复杂场景仍需验证。

## 11. 来源与许可证

本轮为项目自己实现的 Rust 需求/选址/审计、既有 HTML 适配与预览错误处理重构。没有复制外部地形代码、下载资产或添加第三方依赖；渲染测试沿用仓库 Three.js 和现有 Playwright 环境。

未新增 Mapgen4/symbios-ground 许可证兼容性判断，不改变“外部实现必须固定版本、核对 LICENSE/NOTICE 与依赖”的原路线图要求。全项目开源来源审计仍未完成。
