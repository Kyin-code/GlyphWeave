# r43：真实最大地图基线与受控进程测量

日期：2026-09-07。权威工程：G:\difyllmwiki\GlyphWeave。

## 1. 不再以微基准代替整体生成

r39/r42分别证明生态绘制提交和保护距离查询优化，但都不是完整60km²生成验收。本轮直接运行6000×10000m、1m栅格、毫米高度、严格rules、完整自然与人文流程的release CLI。

输入 examples/maximum-6x10km.manifest.json 由既有service-program-joint-plains样例扩展场景尺寸，保留seed7、自然/城乡比例、4项设施与18住宅最低需求，不关闭生态、农业、保护区或独立验证，不把场景改成naturalOnly。该样例仍是单一平原profile，不能代表所有地貌、密度、文化样式均可处理。

## 2. 阶段追踪

worldgen.rs新增仅在环境变量GLYPHWEAVE_BAKE_TRACE=1时输出的stderr JSON事件：manifest-validation、natural-heightfield、natural-drainage、road-and-service-planning、development-partition、entity-generation-and-preflight、final-heightfield-and-semantics、final-entity-access-and-audit、landcover-realization、landcover-independent-reconstruction、landcover-serialization、landuse-accounting、scene-serialization、world-publication。

事件含begin、sceneId、elapsedMs。相邻begin之间的间隔用于定位耗时；最后stage只有起始时间，不能凭日志文件存在断言该阶段完成。

该遥测不写入manifest、revision或世界几何，不参与门禁。新增CLI集成测试分别开/关追踪进行fresh bake，逐字节核对world.json、scene.json、自然/最终高度和地表输出。

## 3. 测量脚本

scripts/measure-world-bake.ps1通过原生PowerShell启动指定CLI，默认隐藏窗口，保留进程句柄，采样周期500ms：

- Process.PrivateMemorySize64的采样最大值（可能漏掉短时尖峰）；
- OS报告的PeakWorkingSet64（进程工作集，不等于专用提交内存）；
- CPU时间、墙钟时间、PID、可执行与manifest SHA256；
- 输入scene尺寸、启动时系统内存、输出文件/字节/chunk数量；
- stderr阶段、退出码、明确停止原因。

活跃状态每5秒写process.json；记录为running并不等于进程仍活跃，继续任务时还必须核对真实PID/工具session。stdout/stderr保留原始输出。脚本只允许fresh输出与报告目录，不递归删除或覆盖既有文件。

受控预算：600秒、8GiB采样专用内存上限。达到上限只终止本次持有句柄的子进程，标为budget-stopped而非通过；不按名字杀死其他预览/任务。输出残留保留用于定位，不把部分chunk当作完整world。

运行命令（在仓库目录）：

    ./scripts/measure-world-bake.ps1 -Executable bevy/target/release/glyphweave.exe -Manifest examples/maximum-6x10km.manifest.json -OutputDirectory bevy/work/r43-maximum-baseline -ReportDirectory bevy/work/r43-maximum-measurement -TimeoutSeconds 600 -MaxPrivateMiB 8192

预算是本次实验约束，不是用户认可的产品性能验收门槛；预算内生成完成仍须独立质量审计与HTML验证。

## 4. 成功判据与范围

必须同时检查：子进程退出0、world.json与generation-report存在、240个chunk覆盖6000×10000m且尾块尺寸正确、完整自然/最终payload存在、真实非空人文与生态数据、CLI独立重建和gate结果、浏览器流式加载实际行为。

物理网格需60000000个样本；毫米高度单份原始payload约240MB，不应把256×256或512×512测试报告成全域。局部漂亮截图、空实体或只完成自然高度都不代表整图完成。

## 5. 来源与未完成范围

没有新增第三方依赖/算法或素材。测量脚本和遥测是项目自有诊断，现有许可证关系不变。

不减少原始路线图要求：自然地貌科学性/水文、城市和农村剩余开发、许可证发布审计和最终视觉仍须继续实现。即便本轮最大尺寸完成，也不能把单profile成功泛化为全部背景可用。

## 6. 当前实测

### 6.1 最大尺寸真实生成完成，但质量不通过

本机release CLI SHA256：731861110E7F5C604448C30832B1796A85C9139550D6FB7A566EC53C80937567。

| 操作 | 墙钟秒 | 采样专用内存峰值（字节） | OS工作集峰值（字节） | CLI退出码 |
|---|---:|---:|---:|---:|
| generate-world | 379.267 | 1,083,699,200 | 1,073,393,664 | 0 |
| scale-audit | 2.210 | 567,570,432 | 594,866,176 | 0 |
| quality-report | 163.337 | 1,354,399,744 | 1,237,127,168 | 0（质量warn） |
| world-quality-gate | 166.997 | 1,355,911,168 | 1,237,827,584 | 1（质量fail） |

所有操作均完成，没有命中本次600秒/8GiB预算。上述是单进程测量，不包括Node校验器、独立浏览器和系统其他进程，500ms采样不是毫无遗漏的瞬时峰值。

最大基线输出共1531文件、1,496,408,006字节（独立审计之后又增加报告文件）。其中自然与最终高度各240块、每层60,000,000采样；尾块索引11,19，真实尺寸368×272m。check-maximum-world.mjs检查全部480个高度文件的长度/头部、单层全域覆盖和surface/semantic payload大小，不能仅由scene.json声称240块。

### 6.2 数据规模与门禁

- 623个实体：246道路、152住宅、1学校、2便利店、1教堂等；独立rules audit为623/623 pass。
- 设施4/4，住宅最低18，实际带入口住宅152；对应专项均pass。
- 270田块、37,462个地表patch、35,550生态群落。
- 生态目标342,380、实现342,377、缺额3。保留地9,602,160m²，改变自然高度采样0。
- 自然路由审计仅证明数据完整：58,844节点、1703流域、1628闭合洼地、4未解决平坦终端，不是水文安全或真实侵蚀证明。
- 地表未实现50,535,332m²；农村未实现47,562,452m²、城市未实现2,400,720m²、未分配572,160m²。地图尺寸覆盖不能代替人文环境实现。
- full gate实际fail：用途面积/地表/用地规划/road-first development未完成，加上3个生态缺额。没有更改任何阈值来通过。

### 6.3 实际阶段耗时（相邻begin间隔）

- natural-heightfield：68.038秒；
- natural-drainage：0.274秒；
- road-and-service-planning：2.149秒；
- final-heightfield-and-semantics：145.750秒；
- landcover-realization：74.894秒；
- landcover-independent-reconstruction：80.562秒；
- 两份地表/场景序列化以及其间账本步骤约1.8秒。

完整stderr逐阶段记录位于r43-maximum-measurement/stderr.log。最后world-publication到子进程退出还含世界导出和CLI报告，不把最后begin当成完成时刻。

### 6.4 最大地图HTML加载实测

使用真实367,474,626字节scene.json，headless Edge默认静态预览（非FX）。浏览器初始化只设置相机关注点，不修改地图数据。

| 关注块 | 就绪墙钟 | 活跃生态实例 | 活跃批次 | 采样usedJSHeapSize |
|---|---:|---:|---:|---:|
| 0,0 | 7.008秒 | 3718 | 3 | 675,389,713字节 |
| 11,19 | 5.489秒 | 479 | 2 | 930,419,242字节 |

分别核对sizeM=6000×10000、chunks=240、highDetailChunk、loadedChunks与实际生态批次索引；无pageerror/失败请求，截屏已实际查看。

**发现的未解决问题**：虽然GPU只加载活动chunk，CPU仍加载全scene JSON（每次约367MB传输）和全域生态索引。上表是页面性能API的采样JS堆，不是浏览器进程峰值，也不能解释为无内存泄漏或长期导航稳定。当前测试是首末块重新加载烟雾测试，不是遍历240块后的持久会话验收。

截图中的旧草过密、地形纹理和代理植物依旧不合格，末块相机/近景观感也需要改善。这里只证明真实尾块可以加载，不代表用户要求的最终HTML视觉通过。

### 6.5 测量器与常规回归

- 测量器支持generate-world、scale-audit、quality-report、world-quality-gate、validate-world；非生成操作要求已有world.json，报告目录仍必须fresh。
- 修复成功路径未显式设置脚本退出码的问题；现在成功0，CLI失败/预算中止1，测量异常记录measurement-failed并只终止本次句柄的子进程。
- 实测512m正常bake成功、无效manifest子进程退出1、独立1秒预算测试终止且不产生world.json；三个分支记录均正确。故意1秒预算试验是测量器回归，不能混成最大基线生成失败。
- workspace402通过（331 Core、44 App、6 CLI单元、21 CLI集成；1个历史微基准默认ignored），Node34、Vitest170通过；lint退出0但有既有warning，typecheck/doc-tree/diff-check通过。
- 追踪开关回归测试确认同输入开关遥测不改变权威文件；本轮世界语义revision保持r41。

### 6.6 证据路径

- 最大实际输出：G:\difyllmwiki\GlyphWeave\bevy\work\r43-maximum-baseline。
- 进程/磁盘/阶段：bevy/work/r43-maximum-measurement；scale/quality/gate分别为同前缀的scale-measurement、quality-measurement、gate-measurement。
- 汇总：r43-maximum-measurement/r43-evidence.json。
- 全域payload覆盖：r43-maximum-measurement/payload-coverage.json。
- 浏览器最终证据：r43-maximum-measurement/browser-final.json，以及.0-0.png/.11-19.png截图。
- 独立rules日志：bevy/work/r43-maximum-rules.log。
- 预览端口8863，独立副本bevy/work/r43-preview-runtime/glyphweave.exe，避免锁定后续编译产物。

## 7. 由实测决定的下一步

1. 拆分HTML所需的scene摘要与chunk实体/地表/生态payload，避免每次加载367MB场景；CLI完整审计数据仍保持权威，不靠删审计记录减小文件。
2. 大地图的主要语义缺口是区域化路网/聚落与用途面积实现：城市实际面积为0、农村仅434668m²，不能把缩小未实现分母当作进步。
3. 性能优化优先关注145.75秒最终栅格和两遍共155秒地表实现/重建；保护距离索引不是唯一瓶颈。不能去掉独立重建来获得更快的假验收。
4. 真实自然流域/湿地适宜性和视觉仍需进一步实现，不用烟雾测试替代质量目标。

结论：完成了一个真实最大尺寸profile的生成、独立重建和首尾块加载基线；**完整现代社会地图质量门禁与最终视觉仍未完成**。
