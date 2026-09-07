# r39：生态植物chunk实例化与可核对的渲染性能

日期：2026-09-07。权威工程：G:\difyllmwiki\GlyphWeave。

## 1. 当前缺口与重构目标

r38实际生成1915个Rust植物实例，但HTML为每个植物创建独立Group、Mesh、ConeGeometry和Material。大地图流式预览不应按每棵植物不断分配/保留GPU对象；也不能通过漏画植物减少工作量。

本轮不修改Rust生态预算、地形、实体、密度或缺额合同，仅修改其HTML消费方式。生成器语义revision仍为r38；渲染器单独报告 r39-chunk-instancing-v1，避免把渲染变更冒称为新的地形生成算法。

## 2. 新模块与职责

新增 adapters/html/render/ecology.js：

- indexEcology验证并按source scene的chunk索引、kind分类植物；
- createEcologyRenderer负责活动块GPU批次的创建、复用、移除和释放；
- Rust core的write_html_preview_modules将该文件加入include_bytes清单，fresh bake导出可离线使用；
- app.js移除逐植物Group/Mesh路径，调用该模块；
- 仍使用原来的树/芦苇/灌木代理圆锥及颜色，不新增资产，不改变占地或三角形细节。

## 3. 位置合同与原有问题修复

来源chunk以scene origin和chunkSizeM定位，随后按validWidthM/validDepthM检查半开区间归属，处理边界和不足512m的尾块。重复ID、未知来源patch、未知kind、非法尺寸/非有限值、缺失或越界chunk均拒绝，不默默跳过实例。

实例矩阵仅保存chunk内X/Z、原始世界Y和Rust宽高。Mesh.position保存chunk世界原点，共享scene root只减一次scene origin。这同时避免Float32直接存大世界坐标导致精度损失。

原逐植物路径先减scene origin，父root又减一次，非零origin可能发生双重偏移。现在通过真实Three矩阵测试在0、负origin和2,000,000,000 origin下核对世界底点、顶点和尺寸。

Float32实例矩阵的检验容差单独用于渲染数值量化，不改Rust毫米高度格式或几何净空阈值。

## 4. 有界缓存与重绘

- 一个immutable scene snapshot构建一次CPU分类索引；相同snapshot重绘不重新扫描所有实例。
- 每个活动chunk/kind一个InstancedMesh。共享种类模板最多3份，每个实例保留ID、sourcePatchId、kind、layer与instanceIndex映射。
- 活动块不变时复用批次和矩阵，不重新上传；失活块解除父子关系并触发mesh.dispose释放实例缓冲。
- 模板和CPU索引在更换scene/surface/plant数组引用时失效，dispose释放旧资源。
- 输入约定是Rust的不可变JSON snapshot；对同一对象原地篡改并不自动检测。更新须换snapshot引用或显式dispose。这不是新的前端编辑接口。
- 同一父root在重绘前clear旧瞬态Group，防止旧生态等未缓存对象残留导致重复渲染；缓存对象可以重新挂到新Group。

本轮只保证**生态GPU批次**按活动块受限。CPU仍持有完整scene的植物索引，原有terrain/entity缓存不在本轮范围；不能据此宣称整个应用内存已经有界。

## 5. 不降低验收标准的测试

### Node真实Three测试

使用项目已有vendor Three，而非mock：

1. 三种origin、部分chunk及变换后底点/顶点/宽度正确；
2. 半开边界点只归属一个chunk；非法记录拒绝；
3. 重绘复用、重复active列表不重复实例；
4. 导航淘汰实例缓冲，返回旧chunk不会无限缓存历史GPU对象；
5. snapshot替换刷新旧矩阵，并验证材质/几何dispose事件；
6. 6km×10km、240块的**合成渲染fixture**逐块遍历，每块3个代理实例，检查驻留批次上限。

最后一项不是完整6km×10km地图生成、真实全密度渲染或峰值内存基准，不能替代路线图的最大地图验收。

### 浏览器合同

check-preview-conservation.mjs从真实InstancedMesh读取矩阵，乘实际父变换，逐个核对Rust实例的ID、来源、层、类型、世界底点和尺寸；不只比较计数。连续重绘3次，确认没有多画/漏画、地表重复、或者同活动块无谓重建。

### 独立WebGL对照

check-ecology-drawcalls.mjs用同一份真实baked植物，分别构建旧的逐植物Mesh和新批次，在独立Three场景中实际render，读取renderer.info.render.calls/triangles和geometry对象数。

必须同时满足：植物数相同、提交三角形数相同、调用数下降。该指标只说明生态层绘制提交变化，不是整场景FPS、GPU执行时间或进程峰值内存。

## 6. 尚未完成与许可证

仍未解决代理模型质量、过密旧草层、生态物种/演替/湿地水文、农村与城市剩余土地、整个最大地图性能和内存证明。本轮不会改变world-quality-gate门槛或放大土地面积信用。

没有下载新素材或新增第三方依赖；沿用已vendored Three及其原有许可证文件。模块是项目的消费/缓存代码，没有接入Mapgen4或symbios-ground源码。

## 7. 实测结果

最终输出目录：G:\difyllmwiki\GlyphWeave\bevy\work\ecology-r39-instancing-final。

### 7.1 同等几何的真实WebGL对照

| 指标 | 旧逐植物路径 | 新批次路径 |
|---|---:|---:|
| 植物记录数 | 1915 | 1915 |
| render.calls（独立生态场景） | 1915 | **3** |
| render.triangles | **21630** | **21630** |
| GPU登记geometry对象数 | 1915 | **3** |

新批次的实例矩阵数组为122,560字节（1915×16×4），只计矩阵，不是总显存。真实完整预览的1915个实例经ID、来源、层、尺寸、世界变换核对通过；连续重绘3次没有重复、丢失或新建批次，indexBuilds=1、builds=3。

### 7.2 当前态回归

- Rust workspace：318 Core + 44 App + 6 CLI单元 + 18 CLI集成，386通过。
- Node合同34通过，其中7个为新实例化/缓存/坐标测试；Vitest170通过。
- lint退出0（仍有既有warning），typecheck、doc-tree、git diff --check通过。
- 本轮导出的preview/app.js与preview/render/ecology.js逐字节等于最终源码，未拿旧CLI嵌入版本作为当前证据。
- 与r38比较，scene.json、自然高度bin、最终高度bin均逐字节相同。1,915个目标全部生成且0缺额，设施4/4、住宅18/18、29道路保持不变。
- 新输出scale-audit为pass、对象规则55/55 pass；quality-report为warn；world-quality-gate实际退出1、status=fail，仍有农村157,504m²、城市9,048m²未实现及相关用途规划项失败。

### 7.3 证据路径

输出目录内：r39-evidence.json、drawcalls.json、ecology-preview.png.json、ecology-preview.png。

外层日志：bevy/work/r39-workspace-tests.log、r39-node-final-tests.log、r39-vitest.log、r39-browser.log、r39-drawcalls.log、r39-final-generation.log、r39-final-quality.log、r39-final-gate.log。

预览端口8859，独立runtime为bevy/work/r39-final-preview-runtime/glyphweave.exe，不锁住target/debug编译产物。截图已实际查看；渲染身份与性能专项通过不代表视觉完成，地面偏白、旧草过密、代理资产质量仍待处理。

**总体目标未完成**，特别是合成240块遍历测试不等于Rust生成完整60km²世界，更不是其峰值内存/帧率证明。
