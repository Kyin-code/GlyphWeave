# r23：显式毫米高度场、共享自然地形范围与端到端验证

权威工程：`G:\difyllmwiki\GlyphWeave`；未修改 `F:\modenmapbox`。
核心版本：`worldgen-2026-09-06-r23-precise-height`。
接续 `20260906-agriculture-surface-realization.zh-CN.md`。

**状态：可选毫米高度格式已贯穿自然/最终 bake、最终审计和 HTML；默认仍兼容旧格式。完整自然/人文路线图和质量门禁尚未完成。**

## 1. 本轮要解决的真实问题

r22 的权威高度为 i16、每单位 0.25m。农业在 1m 水平网格上检查 12% 田面/8% 通道坡度，因此 0.25m 的量化跳变会使许多实际连续缓坡被保守拒绝。简单把坡度放宽或只平滑 audit 输入都会造成审计与最终几何不一致。

本轮在原连续地形函数与持久化之间分离量化：

```text
原有地貌近似 + 水体边界 + 工程填方包络
                    ↓ terrain_height_metres（f64 米）
          ┌─────────┴─────────┐
     旧版 i16 / 4m       可选 i32 / 1000m
          └─────────┬─────────┘
    同一格式的自然 raster / 最终 raster
                    ↓
  BakedTerrain 查询、hash、道路、通路、农业、CLI
                    ↓
      HTML 用同一解码器构造真实 metre mesh
```

这里的 `/4m` 和 `/1000m` 表示将整数除以 4 或 1000 转为米；水平网格始终为 1m。不是把旧阶梯扩大 250 倍，也不是对旧 raster 进行视觉平滑。

**提高数值精度不是提高真实地貌可信度。** 原山体/谷地生成算法仍是已有近似；本轮没有引入真实水力侵蚀、沉积、土壤或灌排模型。

## 2. 新高度格式与兼容策略

通过 manifest 开启：

```json
"style": {
  "heightPrecision": "millimeter"
}
```

省略或 `quarter-meter` 使用旧格式。其他值或非字符串在 manifest 验证阶段拒绝。

### GWHM v1 二进制

| 偏移 | 内容 |
|---:|---|
| 0–3 | ASCII `GWHM` |
| 4–7 | little-endian u32 version，当前 1 |
| 8–11 | little-endian u32 有效宽度 |
| 12–15 | little-endian u32 有效深度 |
| 16 起 | row-major，little-endian signed i32，单位毫米 |

文件长度必须为 `16 + width × depth × 4`；单块尺寸 1..512。旧 raw i16 长度为 `width × depth × 2`，保留有符号数语义。自然与最终 `.height.bin` 使用相同格式，不允许同一 BakedTerrain 混合编码。

- `world.json.sidecar` 与 `sidecar.json.contract` 明确声明 `heightEncoding` 和 `heightPrecisionM`。
- 新编码名 `gwhm-v1-i32le-millimeter`，旧编码名 `i16le-quarter-meter-raw`。
- `lodHeightPrecisionM=0.25`：LOD overview 仍是旧粗摘要，不能当作毫米精度工程证据。
- chunk 的 `minQuarterM/maxQuarterM` 仍是粗摘要。CLI 连续性验收改为直接读取实际米高度差，同时报告 maxBoundaryHeightStepM，不能把毫米样本先舍入到 0.25m 再检查。
- `.gemap` identity-anchor、体素协议没有改动；高精度在 sidecar 地形层。
- 旧版本客户端不认识 GWHM，必须升级 decoder；不得忽略 sidecar 直接按 raw i16 读取。

BakedTerrain 的 hash 纳入格式标签、尺寸、origin 和全部样本，毫米/旧格式即使某些实际高度相同也不会使用同一身份。CLI 检查实际格式与 manifest 一致，因此把有效的旧 raw 文件塞进毫米场景也不能通过。

## 3. 自然源函数与实际消费链路

新增 `terrain_height_metres`，将原来的最后一步量化搬到编码边界。旧 helper 继续输出 quarter-metre，保证兼容调用。新 bake 在自然生成和最终工程生成时从连续源采样，四舍五入到毫米，误差不超过 0.0005m。

- 自然 raster 保存完整毫米数据，再规划实际低地路网。
- 最终 raster 保存实际毫米工程结果，再重建通路、庭院和农业表面。
- terrain semantic 的 slope/curvature 来自同一编码后的 metre 样本与同精度跨 chunk halo；没有另造平滑检查地形。
- 表面类别仍属于离散分类；湿度和侵蚀近似未在本轮重写。
- BakedTerrain 对外 height() 仍返回 f32 米，当前生成范围 -32..256m 的浮点误差远小于 1mm；这不意味着任意 i32 极值高度都能以 f32 精确保留毫米。
- 编码器拒绝非有限源数据、超 signed-i32 范围、错误数量和多余样本。解析器在尺寸校验后计算长度，避免 32 位目标的整数乘法风险。

### 必须公开的中间管线限制

一些**最终 bake 之前**的旧模板/规则预检查仍调用 quarter-metre helper，实体 worldY 也仍是整数米。它们可能保守拒绝或提出随后不合格的候选。不能称为“整个生成器所有阶段已经改成高精度”。

本轮保证的是：持久化自然/最终高度、最终独立道路/实体/通路/农业验证以及 HTML 读取一致。预检查不能替代这些最终门禁。下一步应拆出统一 TerrainContext/自然 provider 与工程查询，迁移剩余旧预检查，不再把地形选项塞在 WaterGeometry 里。

## 4. 非零 origin 与共享 TerrainDomain

跨场景测试暴露了旧逻辑的两个事实：

1. 径向自然背景依赖 scene width/depth，因此不同场景尺寸本就代表不同背景范围，不能无条件要求两者重叠区相同。
2. 原中心为 width/2、depth/2，忽略 scene origin，负坐标场景的背景中心错误。

现在默认背景中心包含 origin。需要多场景共用同一自然背景时显式指定：

```json
"terrainDomain": {
  "originX": -400,
  "originZ": -90,
  "widthM": 1012,
  "depthM": 512
}
```

共享范围必须合法，所有场景完整位于其中；未知字段、错误尺寸、坐标溢出和场景越界拒绝。相同 seed/seedOffset、相同 terrainDomain 及相同自然水体背景下，重叠坐标取得相同自然高度。

测试覆盖两个真实 baked 场景、非零/负 origin 和 513m partial chunk，逐样本比对跨 chunk 的重叠范围。该测试不是说不同地形背景、不同 authored 水体也应该相同。

负 origin 的旧生成结果会因中心修正而变化，这是明确行为修复；不能笼统声称所有旧地图重新生成都逐字节相同。

## 5. HTML 与 Godot

### HTML 已实际验证

- 新共享 `adapters/html/core/height.js` 同时解码两种格式；app 与备用 core/world 查询一起迁移。
- app 检查解码结果与 world sidecar 声明一致。
- terrain geometry、PBR 高度查询和场景 heightAt 使用同一读数，不除错单位、不额外舍入。
- 修复 mesh 将 `width-1` 个样本间隔拉伸到 `width` 米的问题。现在内部顶点落在整数米位置，最后边界顶点使用最后样本延展；LOD partial cells 同样不拉伸。
- 最后一个样本到场景边界的一米延展是既有 raster 的边界策略，不是额外自然数据。

浏览器对实际 terrain mesh 逐顶点验证：263169 个 land 顶点，217484 个不是 quarter-metre 值；最大高度误差约 **9.46e-7m**（Float32），水平位置残差为 0。并验证 26 个耕作面、68 个 farm-track patch 与 Rust 顶点一致。不是仅依据页面 ready 标记。

### Godot 尚未运行验证

Godot loader 已加 GWHM header/长度/维度检查，同时修复 get_16/get_32 读取无符号整数后没有恢复负高程的问题。但当前环境未发现 Godot 可执行文件，**没有运行 Godot/GDScript 测试或截图验收**，不能称为已经验证的跨引擎交付。

## 6. 真实结果及不能掩盖的退化/缺口

最终产物：`bevy/work/height-r23-final-{512,1000,plains,legacy}`。
旧两个 terrain 输入为 seed=20260906；plains/legacy 共享 seed=7 平缓地形及农业比例输入，仅 heightPrecision 不同。

| 指标 | 毫米 512m 起伏 | 毫米 1000m 起伏 | 毫米 plains 512m | 同输入旧精度 plains |
|---|---:|---:|---:|---:|
| 真实道路 | 5 | 9 | 32 | 35 |
| RoadsideSite 计划 | 1 | 3 | 21 | 23 |
| 最终 residential_home | 1 | 3 | 22 | 24 |
| 实现农业田块 | 0 | 1 | 26 | 4 |
| 耕作面 m² | 0 | 256 | 6656 | 1024 |
| 地头作业带 m² | 0 | 320 | 8320 | 1280 |
| 农用通道 m² | 0 | 8 | 2208 | 48 |
| 实际归属面积 m² | 3264 | 7448 | 48508 | 36376 |
| 最终高度文件字节数 | 1048592 | 4000064 | 1048592 | 524288 |
| 完整 gate | **fail** | **fail** | **fail** | **fail** |

精度提高也会使道路/住宅候选改变，不是所有数量都增加。例如旧 1km 用例道路 10、临路住宅 9，毫米版为 9 和 3；这些变化来自不同高度/可行标高/连通分量，不能用田块增加概括全部质量。

plains 的毫米与旧版**都仍有** `buildings_without_development_plan` 和保护/未指定 Zone 内住宅冲突；最终房屋比 RoadsideSite 多 1，明确不能把它计为规划成功。完整 gate 没有被修改为忽略这些问题。

`quality-report` 中 RoadNetworkAudit 通过，四组直接 CLI rules 与 bake rules 一致；整体 fail 仍含土地/地表未实现、道路先行开发未完成。

### 旧格式回归

当前代码重新生成 legacy plains，与 r22 相同输入的五类 chunk payload 逐字节比较全部相同：

- heightFile；naturalHeightFile；surfaceFile；terrainFile；lod2File。

这个证明限定于同输入、零 origin、未指定新 terrainDomain 的已跑场景，不是对任意种子/旧版本输出的广泛承诺。

汇总：`bevy/work/height-r23-evidence.json`。各场景实际 JSON、独立规则和 gate 日志保留在输出目录及 work 日志中。

## 7. 测试和当前验证范围

`bevy/work/height-r23-complete-tests.log`：Core 292、App 44、CLI 单元 6、真实 bake 集成 12，**共 354 项通过**。

新增重点：

- signed mm 编解码、header version/dimensions/长度、非有限/溢出/多余样本。
- 栅格 mixed encoding 拒绝且插入原子性、partial chunk、非零 origin、小梯度。
- 连续函数到毫米误差、不是旧量化阶梯放大。
- 精度配置/共享范围合法性、两场景跨 chunk 自然高度相同。
- 真实 strict 毫米 bake 的 validate/scale/quality/rules 一致；自然像素改 1mm、header 错误、截断、downgrade、sidecar 欺骗均失败。
- 庭院/农业既有篡改与面积测试没有移除。

前端 Vitest 170 项；新增高度 decoder/实际 mesh 的 Node 测试 3 项，加既有住宅几何 4 项，共 7 项；typecheck/doc-tree 通过；lint 无错误但仍有既有警告。

当前预览为独立可执行副本，端口 8837：`http://127.0.0.1:8837/preview/?view=farm`。
实际截图 `bevy/work/height-r23-final-plains/precision-farm.png`；浏览器合同证据 `bevy/work/height-r23-preview.log`。

最后一次增加 app 声明格式校验只影响预览读取，不改变 baked 地形；已同步当前 plains 预览并重新验证正常渲染。`scripts/check-height-preview-mismatch.mjs` 在独立浏览器请求里注入合法旧格式，页面明确显示格式与 sidecar 不符且未完成渲染；没有修改磁盘真实高度文件。证据在 `bevy/work/height-r23-browser-downgrade.log`。

## 8. 性能和后续必须工作

- legacy BakedTerrain samples 为 120MB/60百万点；mm 为 240MB。自然/最终 raster 同时存在时可到约 480MB 的样本数组，**不是进程峰值内存**，不包含 JSON/mesh/其他索引。
- 新格式 header 每 chunk 16 字节；没有压缩。本轮去掉 precise 生成时先算一次 legacy 再算一次连续源的重复工作。
- 更多农业候选通过会增加 route/mesh/独立重建成本；本机 debug 单次 plains mm bake 约 22 秒，legacy 约 10 秒，不能拿单次结果当基准结论。
- 未执行 6×10km 全 mm bake 和峰值内存基准，默认不切到毫米。

下一步应按真实缺口继续：

1. 将剩余旧预检查迁移到统一高精度 TerrainContext，拆开自然 provider、水体几何与工程状态。
2. 道路/住宅先服从土地与自然保留边界，解决 plains 已暴露的多余未规划住宅和保护地冲突；不能删审计条件。
3. 如要建设不合格坡地，实现有预算的切填方与排水/挡土方案，而不是再降低 8%/12% 限制。
4. 分块高度/表面/空间索引和最终内存性能验证，再决定是否默认启用毫米。
5. 自然地貌质量、真实侵蚀/沉积候选、地区人文布局、公共设施、保护地、全量许可证和最终视觉仍属原始目标，不能被这次格式迁移替代。

## 9. 来源与许可证

本轮是自有数值存储与集成重构，没有复制外部地形代码、新增依赖或下载资产；保留旧地貌函数，并明确其谷地处理是近似而非真实侵蚀。未作新的 Mapgen4/symbios-ground 许可证结论。任何下一步外部实现仍需固定版本、核对 LICENSE/NOTICE/依赖树。
