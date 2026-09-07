# r47：Rust 自动道路生长、真实分区进入与验收边界

日期：2026-09-07
状态：本阶段实现与验证完成；最大世界完整质量门禁仍为 **fail**，总路线图未完成。
生成器版本：`worldgen-2026-09-07-r47-zone-aware-automatic-road-growth`
报告模型：`bounded-purpose-centre-and-zone-growth-v2`

## 1. 本轮解决什么，不解决什么

r46 已经把非等高道路写入真实工程操作、最终毫米地形、实体及 HTML，但仍要求输入每段道路的坐标和端点标高。r47 将这一决策移回 Rust：Agent/用户只表达目标用途、候选长度和预算；Rust 从已有连通道路、不可变自然地形和分区约束中选择扩展。

这不是另一个只展示线路的研究页面。接受的候选使用同一生产 bake 路径，参与道路所有权、最终地形、农业通道、独立审计和分块预览。但它也不是完整城市生成器：**道路进入城区不代表住宅、商场、街区或服务已生成**。

本轮没有更换自然地形算法、引入真实侵蚀或完成工程后排水。自然层仍是已有版本的输入，纵断面解算是道路工程近似，不应命名为水力侵蚀、地貌演化或完整道路工程设计。

主路线图：[自然地形与人文环境开发路线图](G:/difyllmwiki/GlyphWeave/docs/20260906-natural-human-development-roadmap.zh-CN.md)。前置实现：[r46 非等高道路](G:/difyllmwiki/GlyphWeave/docs/r46-graded-roads-in-world-bake.zh-CN.md)。

## 2. 输入合同：Agent 给需求，Rust 给几何

在 manifest 的 `style` 中可选增加：

```json
{
  "automaticRoadExtensions": {
    "version": 1,
    "scenes": [{
      "sceneId": "scene-0",
      "maxNewSegments": 24,
      "maxCandidateAttempts": 4096,
      "segmentLengthsM": [512, 256, 128, 64],
      "priorityPurposes": ["urban", "rural"],
      "reachDistanceM": 160
    }]
  }
}
```

校验规则：

| 输入 | 合同 |
|---|---|
| 场景 | 必须存在、不能重复，启用 road-first，使用毫米高程 |
| 新增路段数 | 1–256；耗尽后保留未满足目标 |
| 尝试预算 | 1–65,536；不是运行时间或内存的硬上限 |
| 长度集合 | 1–8 个互不重复的偶数，16–1024m |
| 用途优先级 | 非空、无重复，只接受 urban / rural |
| 距离 | 8–160m；不是允许跨障碍通行的半径 |
| 未知字段 | 拒绝；自动程序不接受用户逐条指定坐标/标高 |

旧 `roadExtensions` 显式路径保留；未指定自动配置的旧输入不自动开启该功能。执行顺序是原路网/站点、显式扩展、自动扩展。这也意味着**原住宅/设施生成尚未因新道路而重新开展局部开发**。

样例：

- 小图：`G:\difyllmwiki\GlyphWeave\examples\automatic-road-growth.manifest.json`
- 最大图：`G:\difyllmwiki\GlyphWeave\examples\maximum-automatic-road-growth.manifest.json`

最大图仍为原 r43 的 seed=7、6000×10000m、自然背景、土地比例、设施/住宅/农业要求；本轮增加自动配置，没有手填逐段道路几何。

## 3. 算法、精度与停止条件

### 3.1 有界、确定性的生长

1. 从已连接路网节点取得固定毫米标高；没有可用初始节点时返回 `no_existing_connected_frontier`，不凭空种路。
2. 从节点向四个轴向枚举配置的长度，过滤场景边界和已尝试候选。
3. 按用途优先级、新满足目标数量、距离改善排序；以长度、节点及端点作确定性平局排序。不是随机选择，也不是全局最优路径搜索。
4. 从原始自然栅格推导新端点平台的最低可行整数毫米标高；检查纵坡、填土上限、全宽支撑及端部平台。不会把端点取整到整米来掩盖误差。
5. 通过碰撞、保护和共享路口检查后，事务性追加道路；失败不留下半条边、节点或工程操作。
6. 更新分区证据与剩余预算；保存拒绝原因计数及最多 32 条样例。

报告保留输入程序、接受的实体 ID、尝试次数、目标前后状态和停止原因。停止包括目标全部满足、段数/尝试预算耗尽、无初始网络、无进展候选或无可行进展候选。失败缓存与预算避免重复试探，不等于所有规模均有固定时间复杂度上限；每轮候选枚举和评分本身仍有成本。

### 3.2 “进入分区”与“距离足够近”必须同时成立

一个目标满足必须同时具备：

- 实际道路 footprint 与该分区有**正面积相交**；只有边界接触不算进入。
- 分区中心到道路矩形的 Manhattan 距离不超过 `reachDistanceM`。

这是两个规划代理指标的合取，不要求二者一定由同一条道路提供证据。它不是穿越河流/围墙后的可走路径证明，也不是中心以外所有地块的可达性保证，更不能兑换整个分区的已实现面积。

早期试验只看中心距离，出现“城市目标已满足但城市内无路”的假阳性。最终版本补充正面积进入条件以及回归测试。旧试验目录保留，但不作为最终验收依据。

### 3.3 不可变上下文与工程写入

`AlignmentContext` 将自然栅格源哈希和分区索引准备一次，每场景复用，不为每条候选重算整幅栅格哈希。原公开 `design` / `verify` 包装入口保留。

显式和自动扩展共用 `append_one`：接受的纵断面进入 r46 的道路 profile 合同、14m 全宽工程支撑、最终毫米高程、实际实体和面积所有权。沿用既有工程限制，不放宽填土或坡度上限，不把自然层修改成“刚好适合道路”的源。

**14m 道路支撑通过不代表外侧边坡、涵洞、排水或相邻建筑平台安全。**这些仍需独立工程模型。

## 4. 两项必须记录的修复

### 4.1 接触旧道路中部，不等于形成合法路口

发现一条候选可以接触旧长路段内部，而图上没有对应路口。新增失败测试后收紧 `shared_junctions`：接触点必须是双方道路都声明的端点节点。自动预检和事务追加共用这一条件。

当前拒绝隐式 T 形连接，并不自动拆分原道路或 profile。将来支持 T 路口必须同时拆分图边、连续纵断面、工程和面积合同，不能只让碰撞检查忽略接触。

### 4.2 f32 抵消造成的假埋地

最大图首次 gate 因 `-1 + 0.904f32` 与 `-0.096f32` 的表达误差在两条自动道路上提前失败。该次约 3.25 秒运行不是完整质量审计。

最终对带 profile 的道路先校验 profile/entity 合同，再以整数毫米中心高程对照重建的毫米栅格；旧无 profile 路径保持原样。没有增加宽容误差、没有把 1mm 埋地改成通过。新增正负标高和 1mm 篡改测试；完整重生成与审计使用修复后的 runtime。

## 5. 代码职责与来源

下表路径均相对于 `G:\difyllmwiki\GlyphWeave`，仅用于列出本阶段触及的职责，不代表整个脏工作树都是本轮修改。

| 文件 | 本阶段职责 |
|---|---|
| `bevy/crates/core/src/worldgen/auto_roads.rs` | 自动配置校验、候选/预算/目标、拒绝证据与单元测试 |
| `bevy/crates/core/src/worldgen/road_alignment.rs` | 可复用不可变上下文、最低毫米端点推导 |
| `bevy/crates/core/src/worldgen/road_extensions.rs` | 共用事务追加、合法共享端点验证 |
| `bevy/crates/core/src/worldgen/road_network.rs` | 自动报告、执行顺序和独立重建接入 |
| `bevy/crates/core/src/worldgen.rs` | manifest 校验、模块与生成器版本 |
| `bevy/crates/cli/src/main.rs` | automaticRoads 报告/门禁、毫米埋地修复 |
| `bevy/crates/cli/tests/baked_audit.rs` | 实际 bake、预算失败、独立重放和伪造拒绝 |
| `bevy/crates/core/src/settlement/agriculture.rs`、`development.rs`、`landcover.rs` | 测试构造增加可选报告字段，不削弱原断言 |
| `scripts/check-automatic-roads.mjs` | 独立源栅格/端点/几何/图/目标复核 |
| `examples/automatic-road-growth.manifest.json`、`maximum-automatic-road-growth.manifest.json` | 无逐条坐标/标高的自动输入 |

本轮新增算法为项目内实现，**未复制 Mapgen4 或 symbios-ground 源码，未新增 crate/npm 依赖或第三方资产**。本轮自然高程未换来源，不能把自动道路成果归功于已经接入某外部侵蚀实现。

本阶段不改变既有依赖许可证，也没有完成全库法律兼容性结论。未来引入外部自然核仍须逐项登记源仓库、精确版本/提交、实际复制范围、许可证全文、署名/再分发义务与数据/资产许可；默认不开启尚未核清的后端。项目自有代码也应在发布时纳入明确的顶层许可。全库发布许可证、NOTICE 和来源清单验收仍是必须项，不能以“本轮无新增依赖”替代。

## 6. 最终最大图实测

唯一最终验收输出：`G:\difyllmwiki\GlyphWeave\bevy\work\r47-maximum-auto-final`。

| 指标 | r43 原始基线 | r47 最终自动生长 |
|---|---:|---:|
| 实体 | 623 | 644 |
| 道路 | 246 | 270（自动新增 24） |
| 农田 | 270 | 501 |
| 地表 patch | 37,462 | 38,318 |
| 生态植物 | 342,377 | 342,379 |
| 植物缺额 | 3 | 1 |
| 实际用地面积 | 9,464,668m² | 9,734,168m² |
| 未实现面积 | 50,535,332m² | 50,265,832m² |
| 城市实际用地 | 0m² | 106,384m² |
| 城市建筑 | 0m² | **0m²** |
| 农村实际用地 | 434,668m² | 597,784m² |

总实际面积增加 **269,500m²**。城市新增面积由道路 73,528m² 和农用通道 32,856m² 构成，不能称为城市建筑增量。农用通道落入城市用途分区也反映了后续用途/地块相容规则仍需细化，不能为追求覆盖率把它当作已完成的城市开发。

自动生长尝试 41 次，接受 24 次，拒绝 17 次，停止于 `segment_budget_exhausted`。

| 目标用途 | 总目标数 | 生长前满足 | 生长后满足 |
|---|---:|---:|---:|
| 农村 | 840 | 30 | 32 |
| 城市 | 42 | 0 | 37 |
| 合计 | 882 | 30 | 69 |

**813 个目标未满足**。不得调低目标、删除分区或将预算耗尽改成成功。

其他硬证据：道路 270/270、站点 156/156、设施含入口 4/4、规则 644/644；152 个住宅保留。保护面积 9,602,160m²，改变高度样本 0。自然源哈希保持：

```text
823ef8a1fe17f607bef7c6a7808589eca235f0cd046538421defe1dcd924ba06
```

### 6.1 性能口径

| 操作 | 耗时 | 采样峰值私有内存 | 峰值工作集 | 退出码 |
|---|---:|---:|---:|---:|
| 生成 | 452.0167299s | 1,134,456,832 B | 1,116,217,344 B | 0 |
| 独立完整 gate | 216.3320191s | 1,457,954,816 B | 1,384,808,448 B | 1 |

600 秒 / 8GiB 是本次进程实验预算，不是产品 SLA。内存不包含浏览器或其他进程；私有内存每 500ms 采样。不能把通过生成的退出码当作整体质量通过，也不能将新增内容后的耗时直接包装成性能提升。

### 6.2 六类门禁失败全部保留

1. 自动道路目标未在段数预算内满足。
2. purpose-area 实现不完整。
3. landcover 未实现。
4. 生态密度预算缺 1 株。
5. land-use 规划/实现不完整。
6. road-first 开发不完整。

未分配用途还有 572,160m²；不能直接改成自然用途来消除缺额。自然审计中的采样路由完整性通过，也不等于真实水力安全、平洼处理完备或侵蚀模拟通过。

## 7. 原有对象保留：不能只比较总数

原 246 条道路计划和 156 个站点计划逐项保留，自然源哈希不变。但可选细节生成器仍使用不稳定的顺序 ID：

- 按 ID：96 个旧 ID 不再出现、117 个新 ID 出现、125 个同名对象数据变化。
- 按 kind/asset/XZ/size/yaw/scale 空间匹配（忽略 ID 和 grounding Y）：218 个几何相同对象只是改名。
- 真正没有匹配几何的是 1 个路灯 `(2780,5012)` 与 2 个绿地代理 `(3150,5150)`、`(2272,4970)`。
- 新增几何恰为 24 条道路；所以总数为 `623 + 24 - 3 = 644`。

这是不同配置的重新生成对照，不是对旧保存世界执行删除操作。但它**不能通过“存量对象全部保留”的增量编辑验收**。稳定 ID、显式已有对象约束、重置/迁移操作记录仍需实现；不能用空间匹配隐藏对象身份变化，也不能据此宣称保留全部原始实体。

证据：`G:\difyllmwiki\GlyphWeave\bevy\work\r47-source-retention.json` 与 `G:\difyllmwiki\GlyphWeave\bevy\work\r47-spatial-retention.json`。

## 8. 小图与独立验证

小图最终目录：`G:\difyllmwiki\GlyphWeave\bevy\work\r47-auto-small-final`。

- 10 次尝试生成 3 条自动路，所有已配置目标均进入且中心足够近。
- 共 34 条道路，道路/站点检查通过，设施 4/4。
- 推导端点包含 2374、15843、8249mm，输入没有这些数值。
- 农田为 17，原小图为 18；自动扩路不是对所有输入都单调增加田块的保证。

独立 Node 检查从自然毫米文件推导端点最低值，以 O(N²) 参考包络校核纵断面，检查全宽及端部支撑、最终/自然高程、限额与体积；从分区和道路矩形重算目标证据，并检查接触处都有共享图节点。最大图复核 24 条自动路、539 处道路接触关系、28 个高程文件；小图 3 条、68 处、2 个文件。

Node 检查不独立证明候选排序的正确性或全局最优；CLI 从源输入独立重建完整自动选择，拒绝双份同步篡改的派生报告。二者职责不同，不能混为一个检查结果。

Rust 完整 workspace：**431 passed**（core 355、app 44、CLI 7、integration 25），1 个既有微基准 ignored。新增覆盖自动推导、预算保留、保护屏障、空网络、错误输入、近而未进入、非零 origin 平移、上下文复用等价、隐式 T 路口拒绝、事务不变、毫米埋地和真实 bake 重放。

Node 测试 **43 passed**，Vitest **170 passed**；lint/typecheck 退出 0，保留既有 lint 警告，不声称零警告。完整测试日志为 `G:\difyllmwiki\GlyphWeave\bevy\work\r47-workspace-final-tests.log`；前端状态见 `G:\difyllmwiki\GlyphWeave\bevy\work\r47-js-checks.json`。

## 9. 预览、数据保真与人工观察

最终 HTML 预览服务端口为 8869，场景目录是上述最终最大图；预览路径 `/preview/?view=graded-road`。预览截图：`G:\difyllmwiki\GlyphWeave\bevy\work\r47-automatic-road-preview.png`。

- 实际可见的 23 条 graded road 核对 142,425 个支撑采样，检查无错误；不是宣称截图显示全部 24 条。
- 240 个区块、480 个自然/最终高程文件，每个高程层 60,000,000 样本。
- 权威 scene 为 478,220,647 B；预览目录 3,463,425 B，派生 payload 共 172,090,022 B。
- 原件与派生投影逐字段保真通过：38,318 个独立 patch、342,379 个植物；patch 跨块重复记录为 40,408，不将重复记录计为新增地表。
- 首尾重载与同页首—尾—首导航通过，没有请求完整 scene；不是所有 240 块连续漫游、浏览器长期内存或最终视觉验收。

本次已人工查看截图：可见道路贯穿起伏地面、农田边缘和植被块，但大片未开发空地、直线网络与简化地表仍十分明显。截图不能证明现实城市形态、原始地貌自然度或精细材质合格。

权威 JSON 已接近当前 Node 全字符串读取路径的容量风险区。接下来城市实体大幅增加前，应评估流式读取/紧凑权威表示，不能删减权威内容或只核对浏览器可见块来回避问题。

## 10. 必须继续实现与可选优化

### 必须：下阶段优先沿路实际开发，而非再堆手工线路

1. **局部标高 frontage 合同。** 从道路 profile 的接入位置求毫米标高，把地块、建筑平台、入口与接入路面分开；旧全局 `levelM` 仅保留兼容，不作为新城市开发统一标高。道路采样失败应记录缺额，而不是回退成悬空或全域填平。
2. **明确城乡需求与建筑形态。** 按用途、密度、建筑类型/尺度、设施需求和地块约束选址。先提供可审计的小范围闭环，不用“有路”替代住宅/商业/公共服务；文化模板来源与参数范围依原人文研究文档维护，不硬编码整个国家一个比例。
3. **开发顺序与存量保护。** 接入最终道路图后，独立安排新增地块/建筑；保留原住宅/设施最低数、既有实体约束及稳定 ID。候选失败不能删除既有路灯/绿地后悄悄重试，显式工程移除必须作为输入/操作记录。
4. **逐地块兑现与独立审计。** 真实建筑 footprint、入口净空、步行路径、平台/边坡和合法用途都满足才记账；加入城乡用途与农用通道相容规则。报告“请求/候选/实现/拒绝原因”，不能只报告新增总面积。
5. **补充路网能力。** 支持内部节点拆分后的合法 T 路口、绕障路线与无初始路网时的种子选址；仍遵守保护区、全宽高程及预算，不简单扩大通道最大距离。
6. **自然—工程耦合。** 保留自然源层，补工程边坡、排水/涵洞、整地土方与工程后水文差量。自然骨架、侵蚀近似命名和外部实现来源仍按总路线图推进，不能把 r47 当作自然科学问题已解决。
7. **最大图与开源发布门禁。** 保留 6×10km 同输入对照、全量审计、测时测内存、源身份和许可证/资产许可检查。权威数据增长须有完整性不变的读取方案。

### 下一批沿路开发的最低验收

- 同一自然地形/路网条件下，新增的合法城市建筑 footprint 必须大于 0，并有真实对象、局部平台和入口证据，不只是总城市面积增加。
- 测试正负毫米标高、非零 origin、道路坡段/节点接入、地块跨保护边界、不可实现需求、重复生成确定性以及 1mm 篡改。
- 任意新增模块失败不得部分修改既有网络/场景；旧最低住宅/设施数与自然源哈希保持，存量对象变化须明确列出。
- 独立审计从原始输入及最终地形复算，不信任生成器自行填写的 `pass`；最大图门禁继续报告所有未完成项目。
- 运行小图再运行最大图；如果新增内容增加运行成本，报告真实代价，不通过减少内容维持旧数字。

### 可选，不应阻塞生成主线

候选评分的更多美学权重、非必要预取/压缩、材质光照、研究页面图表可以后置。外部自然后端的具体选型也可分阶段，但来源/许可核查必须在引入代码前完成；真实自然地貌和人文生成质量本身不是可选项。

## 11. 复现及证据索引

使用新的输出目录，不覆盖最终证据。先完成 release 构建，再复制到独立 runtime 运行长任务；Windows 不要在预览锁住 `target/release/glyphweave.exe` 时重新链接。不要按进程名终止其他预览，完整 workspace 测试串行执行。

以下是复现命令，当前文档记录的结果不是在撰写文档时重新执行生成取得的：

```powershell
# 此处 exe 是本阶段留存的 runtime；重新构建后应另建 runtime 副本。
$exe = 'G:\difyllmwiki\GlyphWeave\bevy\work\r47-final-runtime\glyphweave.exe'
$inputFile = 'G:\difyllmwiki\GlyphWeave\examples\maximum-automatic-road-growth.manifest.json'
$out = 'G:\difyllmwiki\GlyphWeave\bevy\work\r47-reproduction-new'
& $exe generate-world $inputFile $out
& $exe world-quality-gate $out
# 完整门禁预期仍退出 1；不要因此将未实现用地删除或改名。
node 'G:\difyllmwiki\GlyphWeave\scripts\check-automatic-roads.mjs' $out 'G:\difyllmwiki\GlyphWeave\bevy\work\r47-reproduction-independent.json'
```

测量脚本：`G:\difyllmwiki\GlyphWeave\scripts\measure-world-bake.ps1`，以新 `ReportDirectory` 分别运行 generate-world 和 world-quality-gate，保存 exe/manifest SHA256 及进程数据。

最终证据均位于 `G:\difyllmwiki\GlyphWeave\bevy\work`：

| 文件/目录 | 证明范围 |
|---|---|
| `r47-maximum-final-generation-measurement/process.json` | 最终生成进程与输入/二进制身份 |
| `r47-maximum-final-gate-measurement/process.json` | 最终完整门禁成本与失败退出码 |
| `r47-maximum-auto-final/world-quality-gate.json` | 六类真实失败 |
| `r47-maximum-auto-final/quality-report.json` | 实际道路、站点、用地及生态指标 |
| `r47-maximum-independent-roads.json`、`r47-small-independent-roads.json` | 源高程/道路/图/目标独立复核 |
| `r47-maximum-projection-parity.json` | 全量派生投影保真 |
| `r47-maximum-payload-coverage.json` | 全域高程数据覆盖 |
| `r47-streaming-regression.json` | 首尾块和同页导航，不是全世界漫游 |
| `r47-automatic-road-preview.png.json` | 可见道路几何及支撑核对 |
| `r47-source-retention.json`、`r47-spatial-retention.json` | 对象身份与空间保留差异 |
| `r47-junction-red-test.log`、`r47-junction-fixed-tests.log` | 隐式路口先失败后修复 |
| `r47-workspace-final-tests.log`、`r47-js-checks.json` | 测试与前端检查 |
| `r47-evidence.json` | 最终证据摘要与文件 SHA256 索引 |

早期 `r47-auto-trial`、`r47-auto-zone-entry`、`r47-maximum-auto` 和非 final 的过程测量不是最终验收输出；不混用早期耗时或通过状态。完整目标仍在进行中。
