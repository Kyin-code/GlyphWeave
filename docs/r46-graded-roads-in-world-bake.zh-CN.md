# r46：非等高道路接入真实世界 bake、所有权与 HTML

日期：2026-09-07。工作仓库：`G:\difyllmwiki\GlyphWeave`。

## 1. 从设计研究进入实际世界，但尚未完成总目标

r45 已经证明固定水平走廊可以在自然地形上求出满足约束的纵断面，但产物只是未应用的研究 JSON。本轮把这项能力接入真实 world bake：

`manifest 显式道路扩展 → Rust 连通/接驳校验 → 纵断面设计 → 道路实体与工程操作 → 最终毫米高度场 → 真实土地所有权 → 农业可达性 → 独立 CLI 重建 → HTML`

最大 6000×10000m 场景已真实重生成，两段各 512m 的道路不是叠在自然层上的展示 mesh：最终道路/路肩栅格逐点符合 Rust 标高，所有权计入真实道路并集，农田可以连接这些实际道路。

**仍未完成：当前走廊起讫点和接驳标高由 manifest 显式给定，不是完整的自动选线/全域路网扩展算法。** 新道路上的城市住宅、公共设施和街区还没有自动展开；最大世界的完整 gate 仍 fail。不能把本轮成果重新定义为整个自然—人文目标完成。

## 2. 输入合同及兼容边界

新增可选的 `style.roadExtensions`：

```json
{
  "version": 1,
  "corridors": [
    {
      "sceneId": "scene-0",
      "start": [4256, 4512],
      "end": [3744, 4512],
      "roadWidthM": 8,
      "shoulderWidthM": 3,
      "maxFillMm": 6000,
      "maxRiseMmPerM": 80,
      "startLevelMm": 5000,
      "endLevelMm": -3000,
      "junctionLengthM": 8
    }
  ]
}
```

r46 限制如下，违反就报错，不能静默略过用户请求：

- 必须启用既有 road-first 生成，并使用毫米高度编码。
- 场景 ID 必须存在；当前仍是无结构化水体的 road-first 场景路径，不宣称桥隧/海滨路网已实现。
- 使用 8m 路面 + 单侧 3m 路肩，完整支撑宽 14m。
- 道路轴向、偶数米长度，最小 16m；不再强迫道路长度为旧网格的 64m。
- 起点必须是已有道路图节点，且标高与所有相接父道路一致。
- 两端必须显式给出接驳标高，平台长度至少 8m；后续走廊可以连接前一条扩展创建的节点。
- 道路完整支撑不得跨越保护/未知用途或场景外部；自然水体相交拒绝。
- 与已有道路的支撑交叠，只能发生在声明的端点路口附近，且每个交叠点标高一致；不偷偷生成中途交叉口或立交。
- 已有沿路建筑及其院落义务不能被新路侵占；原道路和原设施/住宅计划不能被替换、删除。
- 最大填土不超过原 6m 上限，最大纵向站点高差不超过每米 80mm，即 8%。这是项目生成合同，不是道路法规。

没有这个字段的输入继续使用旧规划路径；已有的 64m 无 profile 道路仍按旧合同验证。新增的变长道路必须有合法纵断面，不能借“支持长路”放开任意尺寸的旧道路。

实际输入：

- `G:\difyllmwiki\GlyphWeave\examples\graded-road-extensions.manifest.json`：保留多尺度农田与原服务程序的 512m 接入回归。
- `G:\difyllmwiki\GlyphWeave\examples\maximum-graded-road-extensions.manifest.json`：保留 r43 最大图的 seed=7、尺寸、自然背景、用地比例、田块规则、设施与住宅最低要求，只增加两条显式连通走廊并改展示名称。

## 3. 数据模型与责任拆分

| 绝对路径 | 变化及原因 |
|---|---|
| `G:\difyllmwiki\GlyphWeave\bevy\crates\core\src\worldgen\road_extensions.rs` | 读取程序、保留基线网络/站点、验证共享路口和交叠、增加实际节点/边，不让 Agent 逐点生成标高 |
| `G:\difyllmwiki\GlyphWeave\bevy\crates\core\src\worldgen\road_profile.rs` | 可序列化的道路纵断面合同、尺寸/范围/坡度检查、按世界坐标插值查询 |
| `G:\difyllmwiki\GlyphWeave\bevy\crates\core\src\worldgen\road_network.rs` | 保留基线 levelM；扩展道路读取自己的 profile；新增带完整 style 的独立重建入口 |
| `G:\difyllmwiki\GlyphWeave\bevy\crates\core\src\worldgen.rs` | profile 进入道路 carve 和最终栅格，嵌入 HTML 模块，生成语义修订更新为 r46 |
| `G:\difyllmwiki\GlyphWeave\bevy\crates\core\src\engineering.rs` | 新世界用 v2 工程快照区分 profile 与平面支撑；新 profile 操作绑定真实道路实体 ID |
| `G:\difyllmwiki\GlyphWeave\bevy\crates\core\src\settlement\development.rs` | 去掉合法 profile 道路的 64m 硬编码，用真实长宽算跨分区道路并集 |
| `G:\difyllmwiki\GlyphWeave\bevy\crates\cli\src\main.rs` | 审计从原 manifest 重建扩展，不信任两份相同的已存计划 |
| `G:\difyllmwiki\GlyphWeave\adapters\html\render\road-profile.js` | 从 Rust 站点生成道路显示网格，不以中心 worldY 画平路、不重新采样另一套地形 |
| `G:\difyllmwiki\GlyphWeave\adapters\html\app.js` | profile 专用路径、避免重复 yaw、长路按 footprint 而非仅中心点参与活动块选择 |
| `G:\difyllmwiki\GlyphWeave\bevy\crates\core\src\preview_stream.rs` | 修复实体等紧凑元数据的 f32 JSON 数值扩展问题，保证投影与原 scene 的数字一致 |

### 3.1 道路纵断面

实体的 `grounding.roadProfile` 保存 version、start、end、整数毫米 elevationsMm 和 maxFillMm。实体 worldY 和 bottomOffsetM 表示中心基准；**整条道路不能再从一个 worldY 推导高度**。

路网中的 extensions 保存来源父道路 ID 和设计蓝图。蓝图沿用 r45 的 design-only 状态，是不可变设计输入，不代表整个 world 的执行状态；是否真正应用必须看最终栅格、v2 工程操作和独立道路审计。

基线道路/旧世界没有 profile 时不增加这段 JSON。新生成语义修订为：

`worldgen-2026-09-07-r46-graded-road-extensions`

### 3.2 工程应用

TerrainCarve 从只能持有常数平面，扩展为可持有共享的 RoadProfile。跨 chunk 复制 carve 时使用 Arc 共享纵断面样本，不为每个 chunk 复制整条道路数组。

- profile 对完整 14m 支撑宽及端部 4m 延伸提供标高；端部使用已经验证的平台标高。
- 最终地形只填高、不隐式挖低自然层；保护域仍使用原先的边缘填土包络。
- profile 自己的过渡填土贡献不超过该请求的填土上限。
- 在 profile 影响范围内，合法硬支撑面优先于相邻对象的混合过渡；道路优先级高于建筑，避免旧平路的过渡把新道路抬成错误高度。
- 无 profile 影响的旧区域保留旧填土组合行为。

**本轮没有完成走廊外边坡、路拱、排水、竖曲线舒适性或全域土方平衡。** 特别是硬支撑与相邻过渡的边界连续性，还不能凭道路内采样通过就宣称全场地科学合理。

### 3.3 土地和农业

道路实体进入 DevelopmentPlan 的真实道路并集，跨分区面积分别归属，路口重复覆盖不重复计数。

农业继续使用实际道路实体与最终地形执行原有 4m 通道、160m 长度和 8% 通道坡度等限制，没有扩大无路搜索预算或修改田块尺寸。新增道路更接近原农业候选时，候选才有机会变成实际田面/通道。

本轮保留原服务计划，没有为了给新路腾位置减少学校、便利店、教堂或住宅最低要求。新道路的沿路城市开发仍是下一阶段工作，不能用道路面积替代建筑面积。

## 4. 独立验证，不用旧容差掩盖新错误

CLI 使用 `verify_with_style` 从自然栅格、分区、服务程序和 manifest 的道路请求重建完整路网，再核对计划及最终实体。

新 profile 道路检查：

- 整个 14m 路肩支撑和端部，而不只检查中心线；
- 最终标高与逐点设计一致；
- 填土上限、不切低自然层、世界坐标、实际尺寸、旋转、缩放和 grounding 合同；
- profile 自身合法性及其与来源请求的关系。

新毫米道路使用 0.00005m 的 f32 表达误差容差，不沿用旧平路的 0.26m 存储兼容容差。回归测试将外侧路肩仅改高 **1mm**，新审计必须失败。原先无 profile 的道路合同不变。

另有双份伪造测试：同时修改 scene 和 road-network-plan.json 中的 profile，仍会因为与原 manifest/自然层独立重建不符而被拒绝。

## 5. 真实试跑暴露并修复的问题

### 5.1 旧填土过渡抬高新路肩

首个 512m 试跑虽然成功写出世界，但道路审计发现最大填土 **6.373m**，新路未通过。不能把 generate-world 退出 0 当作质量通过。

原因是旧道路的混合填土覆盖到新路外侧路肩，而新 carve 仍只强制到 5.5m 半宽。修复为与设计/审计一致的完整 7m 半宽支撑，并让验证过的硬支撑覆盖外来的混合过渡。没有提高 6m 上限。

### 5.2 土地模块把道路长度写死为 64m

第一轮最大图在 development-partition 阶段失败，耗时约 **71.45s**。真实原因是旧所有权模块拒绝 512m 道路，不是自然地形无解；旧错误包装中的 descriptors 字样不是许可证或资源加载问题。

修复后，有合法 profile 的道路按真实长宽分区。增加 128m 单元测试验证面积为 `64×8 + 128×8`；把长度伪造为 126m 但保留原 profile 必须失败。没有缩短最大图道路来通过。

### 5.3 HTML 重复旋转

几何单元测试通过后，真实浏览器矩阵核对发现南北道路又被公共缓存包装器旋转了一次。一个应在 `(220,8,96)` 的顶点被变成 `(192,8,132)`。

profile 网格现在标记已按世界轴构造，缓存包装器不再重复应用 yaw。真实浏览器逐顶点和最终高度文件核对通过，而不是只检查画面里“有一条道路”。

### 5.4 紧凑元数据的 f32 JSON 数值变化

加强投影检查后发现，`serde_json::to_value` 会将 f32 扩展成 f64 数值，例如源 scene 的 `0.2` 在预览目录变成 `0.20000000298023224`。这与 r44 修复的 f64 读取往返问题不同。

修复只对紧凑元数据使用与原 scene 相同的 typed serializer 后再转 JSON Value，没有复制/解析整份巨型 scene。实体、landmarks、入口、开发元数据和道路站点等现在保持原 scene 的 JSON 数字；新增精确回归测试，不放宽比较容差。

## 6. 最大图实测结果

原件：`G:\difyllmwiki\GlyphWeave\bevy\work\r43-maximum-baseline`。

新世界：`G:\difyllmwiki\GlyphWeave\bevy\work\r46-maximum-extensions-v2`。

两段走廊：

1. `(4256,4512) → (3744,4512)`，512m，接驳标高 `5m → -3m`；
2. `(3744,4512) → (3232,4512)`，512m，接驳标高 `-3m → -3m`，中间纵断面仍可随地形起伏。

两段是显式验证样例，不声称 Rust 已自动选出最佳城市连接路线。相对高程为负不意味着已模拟地下水/海平面安全。

| 指标 | r43 原件 | r46 新世界 |
|---|---:|---:|
| 场景 | 6000×10000m | 6000×10000m |
| 真实实体 | 623 | 625 |
| 道路边 | 246 | 248 |
| 实际农田 | 270 | 331 |
| 地表 patch | 37,462 | 37,656 |
| 植物 | 342,377 | 342,379 |
| 城市用途真实面积 | 0m² | 4,006m² |
| 农村用途真实面积 | 434,668m² | 484,472m² |
| 全图真实实现面积 | 9,464,668m² | 9,518,478m² |
| 全图未实现面积 | 50,535,332m² | 50,481,522m² |

新增真实实现面积 **53,810m²**：道路并集 +8176m²，田面/地头 +35,136m²，农业通道 +10,498m²。城市新增 4006m²只包括道路和农业通道，**城市建筑仍为 0m²**，不能称为已经生成城区。

原 246 条道路和 156 个沿路站点计划完整保留；旧 623 个实体 ID 没有删除，只新增两条道路。一个既有 green_space 因最终整地从 worldY=-4 调整到 -3，位置/尺寸不变；因此不声称所有既有实体变换逐字节不变。

自然高度哈希与原件一致：

`823ef8a1fe17f607bef7c6a7808589eca235f0cd046538421defe1dcd924ba06`

独立保护审计：9,602,160m² 保护域，高度改变样本 **0**，冲突实体 **0**。设施 **4/4** 实现且有入口；道路 **248/248**、沿路站点 **156/156** 通过道路专项。

生态配额没有降低；新世界仍缺 **1 株**。数量变化是整套布局的确定性重建结果，不把它单独宣传为生态科学性改善或新增自然土地面积。

### 6.1 性能和完整门禁

| 操作 | 耗时 | 采样专用内存峰值 | 退出码 |
|---|---:|---:|---:|
| 真实最大图 generate-world | 418.541s | 1,106,329,600 B | 0 |
| 独立 world-quality-gate | 183.121s | 1,379,364,864 B | 1 |

这轮增加实际道路和农业工作量，没有声称比 r43 的 379.267s 生成更快。测量仍使用 600s/8GiB 的实验预算，不是已获用户认可的最终性能 SLA。

完整 gate 保留五类失败：用途面积未完成、地表未实现、生态缺 1 株、用地规划/实现未完成、road-first 开发未完成。不能用道路专项通过抵消这些失败。

当前测量的生成、审计、最终预览修复是不同二进制快照。实际 SHA 记录在 `G:\difyllmwiki\GlyphWeave\bevy\work\r46-runtime-provenance.json`；生成时的二进制仍保存在 r46-preview-runtime 中，与测量 SHA 相同。后续 f32 预览修复只刷新派生目录/adapter，没有重新生成权威地形。

## 7. HTML 与完整数据核对

新增 `G:\difyllmwiki\GlyphWeave\scripts\check-preview-graded-roads.mjs`：

- 不下载完整 scene.json，读取轻量预览目录及需要的自然/最终高度块；
- 比较真正 Three.js mesh 的世界空间顶点和 Rust 标高；
- 检查每个道路站点、完整 14m 支撑的最终整数毫米高度、无切低和填土上限；
- 核对源 ID、数量和重复实例；
- 支持移动到其他块，验证中心不在活动块内但 footprint 相交的长道路不会消失。

实测：

- 512m 小图：65 站、130 顶点、128 三角形、975 个支撑样本，最大填土 4817mm；
- 最大图两段各 513 站、1026 顶点、1024 三角形，共 **15,390 个支撑样本**；最大填土分别 **5854mm / 1942mm**；
- 实际网格坐标与最终高度均符合，没有 pageerror；
- 最大图 240 个渲染区块重新逐字段核对，实体/入口元数据和全部 37,656 个 patch、342,379 个植物严格一致；
- 两层各 60,000,000 个高度样本，480 个高度文件的头部/尺寸完整；
- 最大图首尾块同页反复切换回归通过，浏览器没有请求巨型 scene.json。

最大图当前源 scene 392,811,374 B，轻量目录 3,267,626 B。并非整个页面只下载这点数据，地形/素材/区块仍另计。

主要截图：

- `G:\difyllmwiki\GlyphWeave\bevy\work\r46-graded-road-preview.png`
- `G:\difyllmwiki\GlyphWeave\bevy\work\r46-maximum-graded-road-preview-east.png`

截图已查看，仍有旧草地/底色/资产和场景密度问题。能验证道路不漂浮、方向正确，不代表画质或现代城市内容已完成。

## 8. 测试、复现与运行注意事项

最终完整 Rust workspace：**421 passed**（core 347、app 44、CLI 6、integration 24），1 个既有微基准 ignored。

Node **43 passed**，Vitest **170 passed**；lint、typecheck、doc-tree:check、git diff --check 通过。lint 仍有既有警告，不声称零警告。

重点新测试包括共享路口、原网络保留、独立重建、硬支撑优先级/顺序不变、变长道路面积、1mm 路肩篡改、双份计划伪造、实际 bake 与工程实体 ID、四方向网格和 f32 元数据精确投影。

复现命令示意（生成输出应使用新目录，避免覆盖已有证据）：

```powershell
& 'G:\difyllmwiki\GlyphWeave\bevy\target\release\glyphweave.exe' generate-world 'G:\difyllmwiki\GlyphWeave\examples\graded-road-extensions.manifest.json' 'G:\difyllmwiki\GlyphWeave\bevy\work\graded-road-new-run'
& 'G:\difyllmwiki\GlyphWeave\bevy\target\release\glyphweave.exe' world-quality-gate 'G:\difyllmwiki\GlyphWeave\bevy\work\graded-road-new-run'
```

本轮遇到 Windows 运行中的 exe 锁定：在长 bake 或先前测试尚未结束时重新链接，会报 Access denied / LNK1104。该错误不等于算法失败或测试通过。已等待原进程结束后重建/重测；未来长测量应使用指定 runtime 副本，完整 workspace 测试串行运行，不能按进程名杀掉其他预览。

证据目录和文件：

- `G:\difyllmwiki\GlyphWeave\bevy\work\r46-maximum-generation-v2-measurement`
- `G:\difyllmwiki\GlyphWeave\bevy\work\r46-maximum-gate-measurement`
- `G:\difyllmwiki\GlyphWeave\bevy\work\r46-maximum-comparison.json`
- `G:\difyllmwiki\GlyphWeave\bevy\work\r46-source-retention.json`
- `G:\difyllmwiki\GlyphWeave\bevy\work\r46-maximum-projection-parity.json`
- `G:\difyllmwiki\GlyphWeave\bevy\work\r46-maximum-payload-coverage.json`
- `G:\difyllmwiki\GlyphWeave\bevy\work\r46-maximum-graded-road-preview-east.png.json`
- `G:\difyllmwiki\GlyphWeave\bevy\work\r46-maximum-streaming-regression.json`
- `G:\difyllmwiki\GlyphWeave\bevy\work\r46-workspace-final-complete.log`
- `G:\difyllmwiki\GlyphWeave\bevy\work\r46-js-verified.json`

## 9. 来源、许可证与下一步

本轮代码为 GlyphWeave 自有实现，没有引入新的 crate/npm 包，没有复制 Mapgen4、symbios-ground 等候选地形源码，也没有新增第三方模型/纹理。Arc、整数数组、序列化与现有 Three.js adapter 使用既有依赖；不改变它们各自许可证。开源发布前的完整依赖/资产/数据通知清单仍未完成，不能以这轮没有新增依赖代替发布合规验收。

下一步必须保持原总目标，而不是继续只增加手工走廊样例：

1. **Rust 自动候选/端点标高与路线选择**：从建设需求、地形和保护约束产生连通扩展，不让 Agent 手动填写逐条道路和标高。
2. **非等高沿路开发**：设施/住宅选址应使用局部道路接点标高，而非旧全局 levelM；保持原最低需求并增加真正的城市用地实现。
3. **外侧边坡和排水**：建立完整土方/边坡边界、工程后水文与自然层的差量验证，不能拿道路内 14m 通过代替全地块可用。
4. **土地用途与覆盖继续兑现**：还有 50,481,522m² 未实现，城市建筑为 0，仍是主要内容缺口。
5. **自然地形及文化形态**：真实侵蚀/水文的来源和许可证审查、自然骨架、区域聚落模板仍按总路线图推进，不能被道路阶段替代。
6. **性能与最终效果**：全密度实体的分块/LRU、全 240 块漫游、素材/材质和整体质量门禁仍需最终验收。

整体目标仍在进行中。本轮证明“自然层 → 非等高道路工程 → 最终地形 → 农田可达性”开始产生真实增量，不证明整个现代社会地图生成器已经完成。
