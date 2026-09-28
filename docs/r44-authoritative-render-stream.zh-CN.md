# r44：Rust 权威数据与 HTML 分块读取解耦

日期：2026-09-07。工作仓库：`G:\difyllmwiki\GlyphWeave`。

## 1. 目标、完成范围与不能声称的结论

本轮继续落实自然地形/人文环境开发路线图中的大地图性能基础设施，不替代地形科学性和聚落生成工作。r43 已经真正生成 6000×10000m 地图，但近景浏览器仍先读取 367,474,626 字节的完整 scene.json，即使只看一个区块也会解析全世界生态与地表数据。

本轮完成：

1. Rust 从权威 SceneIndex 导出轻量渲染目录和内容寻址区块；
2. HTML 仅读取当前活动区块的地表/生态数据，退出区块后释放这部分缓存；
3. 不裁剪、不简化、不重新生成 Rust 的地表三角网和植物；
4. 保留完整 CLI 审计源，已有地图可只刷新派生预览；
5. 对真实最大地图做全量投影保真检查和持续页面导航验证；
6. 修复实测发现的 JSON 浮点往返精度问题。

**这不是“现代社会地图生成器已经完成”。** 未改变城乡面积需求、设施最低需求、生态密度目标、规则阈值、保护区范围或质量门禁。也没有实现真实水力侵蚀、农业排水、地块整地或新的城市形态算法。

## 2. 哪些保留，哪些拆分

### 2.1 保留的权威数据

- 完整 `scene.json`；
- 自然/最终高度栅格与 terrain semantics；
- 地表实现、生态群落和拒绝/缺额记录；
- manifest、world revision、土地用途/覆盖账本；
- Rust 独立重建、scale-audit、rules audit、quality-report 和 world-quality-gate。

这些继续决定世界是否成立。预览目录不是第二套生成算法，也不是质量认证文件。

### 2.2 新增/调整的代码边界

| 绝对路径 | 职责 |
|---|---|
| `G:\difyllmwiki\GlyphWeave\bevy\crates\core\src\preview_stream.rs` | 校验场景域、分块、序列化、哈希与发布，只导出派生数据 |
| `G:\difyllmwiki\GlyphWeave\bevy\crates\core\src\worldgen.rs` | bake 后调用投影导出；提供单独刷新 HTML adapter 的入口 |
| `G:\difyllmwiki\GlyphWeave\bevy\crates\cli\src\main.rs` | `export-preview-stream WORLD_DIR`；限制 scene 路径在指定 world 内 |
| `G:\difyllmwiki\GlyphWeave\adapters\html\core\render-stream.js` | 读取目录、验证 payload、合并边界副本、活动集缓存 |
| `G:\difyllmwiki\GlyphWeave\adapters\html\app.js` | 消费投影，释放退出活动集的地表 mesh，仍不拥有生成规则 |
| `G:\difyllmwiki\GlyphWeave\bevy\Cargo.toml` | 为现有 serde_json 启用 `float_roundtrip`，不升级依赖版本 |
| `G:\difyllmwiki\GlyphWeave\scripts\check-render-stream.mjs` | Node 独立读取完整源并核对每个投影区块，不在浏览器加载全图 |
| `G:\difyllmwiki\GlyphWeave\scripts\check-maximum-preview.mjs` | 首尾块加载、连续切换、真实实例 ID 和网络检查 |

Agent 仍只负责理解背景/表达需求，不参与逐植物、逐面片位置计算。

## 3. 派生文件合同 v1

每个场景增加：

- `preview-scene.json`：目录与适度规模的元数据；
- `render-chunks/{x}-{z}-{sha256}.json`：当前世界修订下的地表和植物。

目录保留 sceneId、原点、尺寸、chunk descriptors、landmarks、entities、accessSurfaces、开发/土地覆盖规划、用地分区，以及 roadNetworkPlan 的 roadsideSites 和 jointLayout 投影。地表元数据中保留农田列表、生态全局汇总，但不加载完整 patches、plants、communities、rejections。

`renderStream` 必须包含：

| 字段 | 约束 |
|---|---|
| format/version | `glyphweave.render-directory` / 1 |
| worldRevision | 与 world.json 一致 |
| chunks | 与 scene.chunks 一一对应；不得重复或漏块 |
| totalPatches/totalPlants | 原件的唯一 ID 总数，不是区块副本数量 |
| chunkX/chunkZ | 非负整数，与文件内部标识一致 |
| file | 固定内容寻址文件名，不接受随意相对路径 |
| sha256/bytes | 对实际传输的 UTF-8 JSON 字节计算 |
| patchCount/plantCount | 当前 payload 数量，不伪装成全图数量 |

区块格式是 `glyphweave.render-chunk` / 1，带 sceneId、worldRevision、chunkX、chunkZ、patches、plants。

`agriculture.previewFocusHeights` 是轻量的相机辅助投影，复制既有 cultivated_soil patch 首顶点高度，保证首次农田视角不会因为尚未读取 patches 而退回 Y=0。它不能用于地形、接地、面积或审计。

### 3.1 分块与边界

- 地表按 XZ 包围盒分配，不裁剪顶点或三角形；开始块取 floor(min)，结束块取 floor(max)，世界末端钳制至最后一个合法块；
- 正方向最大边界恰落在接缝时保守复制到下一块；最小边界不额外复制到前一块；
- 跨块 patch 在浏览器按 ID 合并，重复 ID 的几何必须完全一致；
- 植物中心按半开区块范围唯一归属，最后 368×272m 的部分块仍按真实尺寸验证；
- 植物所在 payload 必须含其 sourcePatchId，不能只在全局目录声称来源存在；
- 非有限顶点、越界 patch、非法三角形索引、重复 ID、缺块、非法原点/块尺寸均拒绝。

旧 HTML 用“是否有顶点落进当前块”决定绘制，会漏掉包围整个内部区块的巨大面片。本轮改为保守包围盒相交；同一修复用于入口路面和地表，避免 Rust 已导出却被前端再漏掉。

### 3.2 发布与缓存

1. 完整序列化区块，计算 SHA256；
2. 已存在的同名内容寻址文件必须逐字节相同，否则报错；
3. 新内容写同目录唯一临时文件，flush/sync 后 rename；
4. 全部区块成功后，最后以同目录替换发布 preview-scene.json；
5. 不删除旧内容寻址文件，避免旧页面持有目录后取不到文件。

该机制避免正常读取半份 JSON，不是跨平台断电恢复、多个发布者协调或多场景事务保证；发布失败不能把部分输出报告成成功。旧版区块清理需要未来单独的离线 GC，不在本轮递归删除。

浏览器先核对描述符，再核对字节长度/SHA256/身份/数量，最后解析并装配。SHA256 检查的是内容一致性，**不是签名、可信来源认证或独立语义审计**；源文件和目录一起被伪造时仍应由 Rust 权威审计发现。

只保留当前活动集的已解析区块。相同活动集重复绘制复用结果；离开的地表 mesh 释放 geometry/material，生态矩阵按既有 r39 规则释放。异步请求带序号，旧返回值不得覆盖较新的缓存；即使最新选择命中缓存，也必须使先前挂起的请求失效。

注意：退出活动集并不保证浏览器 HTTP cache 清空，也不保证 JS GC 立即运行；既有地形/普通实体缓存仍没有全局内存上限。本轮只对投影缓存、地表 mesh 和生态活动矩阵作明确承诺。

### 3.3 旧世界兼容

只有 `preview-scene.json` 返回 HTTP 404 时，才使用旧完整 scene.json 路径。目录损坏、修订错配、payload 缺失/哈希错误不允许偷偷退回巨型完整文件。

已有世界刷新命令：

```powershell
& 'G:\difyllmwiki\GlyphWeave\bevy\target\release\glyphweave.exe' export-preview-stream 'G:\difyllmwiki\GlyphWeave\bevy\work\r43-maximum-baseline'
```

这是派生导出，不会自动修好生成内容，也不会将旧报告升级为通过。新 bake 会自动导出，无需额外执行该命令。

## 4. 全量实测发现的浮点问题

第一次真实最大图全量投影核对失败，而小样例已通过。具体值 `3.8610000610351562` 经原来的 serde_json 快速浮点读取路径后变成 `3.861000061035156`，相差一个 f64 ULP。

虽然差异远低于毫米，仍违反“保持原件数值、不静默近似”的合同，因此没有放宽测试容差。处理方式：

- 对现有 serde_json 启用 `float_roundtrip`；
- 增加 f64 `to_bits()` 往返回归测试；
- 重建 CLI 并重新从未改动的原始 scene 导出；
- Node 以严格 deepEqual 核对每个顶点、三角形、植物字段及其来源；
- 重跑完整 Rust workspace，而不是只测新模块。

该 feature 影响 workspace JSON 读取，不只是预览模块。因此不能笼统声称“所有读入代码都没变”；正确表述是修复 JSON 数值恢复精度，保持 Rust authored 数值。现有 Cargo.lock 没有版本变化，生成语义修订仍为 `worldgen-2026-09-07-r41-explicit-field-dimensions`，没有添加新的地形/聚落算法。

## 5. 真实 6×10km 验收结果

### 5.1 权威保留与全域数据保真

输入世界使用 r43 的原件，没有重跑自然/人文生成来替换其缺陷。

| 检查项 | 实测 |
|---|---:|
| scene.json 原件大小 | 367,474,626 字节 |
| 渲染目录大小 | 3,236,773 字节 |
| 当前目录引用的全部 240 个 payload | 138,730,726 字节 |
| 唯一地表 patch | 37,462 |
| 含边界副本的 patch 条目 | 39,516 |
| 唯一植物 | 342,377 |
| 导出前后原有非预览文件 SHA256 | 1,498 个全部一致 |
| 上述不变文件字节数 | 1,493,996,290 |

Node 全量核对全部 240 个区块的哈希、长度、块身份、期望空间分配、每份几何、全部植物唯一归属与来源，通过。它不是抽查首尾，也不把重复面片计作新面积。

最终 release 派生导出耗时 **3.951 秒**，采样专用内存峰值 **290,353,152 字节**，OS 工作集峰值 **267,624,448 字节**，退出 0。只是读取已有权威场景并导出的时间，**不是 6×10km 生成耗时**；r43 生成基线仍为 379.267 秒。

当前实验目录保留了最初精度失败版本的旧内容寻址文件，因此磁盘总大小包含历史 payload，不能当作全新一次导出的净增量。单个当前有效投影的目录加 payload 合计 141,967,499 字节。

### 5.2 浏览器首尾块与同页导航

以下是单次同机 Edge/Playwright 观测，不是统计分位数、总进程内存或严格隔离环境基准。JS heap 是观察时刻 usedJSHeapSize，不是峰值，也不包含 GPU/全部浏览器进程。

| 视角 | r43 ready 时间 | r44 ready 时间 | r43 JS heap | r44 JS heap |
|---|---:|---:|---:|---:|
| 首块 0,0（3718 植物） | 7.008s | 5.311s | 675,389,713 B | 61,571,112 B |
| 尾块 11,19（479 植物） | 5.489s | 3.812s | 930,419,242 B | 28,141,622 B |

新路径首块场景数据为目录 3,236,773 + payload 1,071,955 = **4,308,728 字节**；尾块为 **3,385,381 字节**。相较每次完整 scene 的约 367MB，大幅减少场景数据，但不包括高度、材质、模型、库脚本、账本等其他资源，不能称为整个页面的下载量。

在同一页面继续：尾→首 4.001s、首→尾 3.622s、尾→首 3.852s；观察时刻 JS heap 分别约 66.2/88.0/63.1MB。投影请求累计从 1→2→3→4，证明退出活动集后再次进入会重新走读取/校验，而不是永远保留所有解析对象。

每次均核对：

- 实际高精度块、loadedChunks 与预期一致；
- 首块 381 个 patch、3718 个植物；尾块 72 个 patch、479 个植物；
- 实际 InstancedMesh 中的每个植物 ID 集合与全量 Rust 原件投影核对结果的 SHA256 一致，无重复；
- 每块不超过 3 个生态 batch；
- 浏览器网络记录中没有 `/scene.json` 请求；
- 无 pageerror、失败请求；
- 同页切换不是仅靠两次 reload 代替。

这是首尾块与反复导航验收，**不是全部 240 块逐一浏览、长时间漫游稳定性或最终画质验收**。截图已检查，仍有旧草地渲染密集、发白底色、原始资产表现等问题；本轮不以“能显示”宣称“视觉完成”。

### 5.3 小场景 fresh bake 回归

`G:\difyllmwiki\GlyphWeave\examples\multiscale-farms.manifest.json` 新 bake 成功：

- 512×512m，生成 2.763 秒；
- 444 个 patch、1698 个植物，全部投影严格一致；
- 农田 18 个，其中 6 个有绕行通道，96 个农业 patch；
- 浏览器农业 mesh 顶点、索引与材料核对通过；
- 1698 个植物实际 instance matrix、基座/高度/宽度、Rust 来源 ID 核对通过；
- 同块重复重绘三次保持 3 个 batch，不重复生成矩阵或植物。

### 5.4 测试

- Rust workspace：**407 passed**（core 335、app 44、CLI 6、CLI integration 22），1 个既有微基准 ignored；
- Node：**41 passed**，含 7 个分块加载测试；
- Vitest：**170 passed**；
- lint、typecheck、doc-tree:check、git diff --check：通过；lint 仍有既有 unused/Fast Refresh 警告，不称为零警告；
- Rust 测试包括 11 类非法导出输入、非零 origin、接缝/部分块、重复导出、f64 位级往返及 CLI 导出不改权威文件；
- 全量最大图源对照由独立 Node 脚本执行，不用 Rust 导出器验证自身就宣布通过。

## 6. 许可证与算法来源

本轮渲染投影、缓存和核对代码为 GlyphWeave 自有实现，没有复制 Mapgen4、symbios-ground、WorldEngine 或其他自然地形源码，没有下载第三方资产，没有新增 crate/npm 包。

唯一依赖配置变化是既有 serde_json 1.0.150 的 `float_roundtrip` feature；本机锁定包的 Cargo.toml 声明 `MIT OR Apache-2.0`，feature 列表为 `float_roundtrip = []`，本轮 Cargo.lock 无变化。依据是已安装锁定包自身元数据，而不是借用别的项目许可证。发行时仍需按所选授权保留相应上游许可与版权信息；这一条不等于全依赖树/全部模型资源已经完成发布合规审计。

自然地形模块现有“自有实现、思想参考、近似过程”的来源边界没有因本轮变化。不得把本轮投影性能收益写成引入真实侵蚀算法的成果。

## 7. 必须继续完成与可选优化

### 必须继续，且不能被本轮性能收益掩盖

1. **人文用途真正实现**：r43 最大图约 50,535,332m² 仍无实际地表实现；城市已实现面积为 0，不能用规划颜色或道路/房屋数量替代城市用地完成度。
2. **开发/自然的耦合**：在保护域约束下实现地块整地、边坡/挡墙/排水、可达性与土方/坡度成本；保留自然高度和最终高度的可追溯差量。不能为了大田块降低阈值或自动缩小用户字段尺寸。
3. **自然核心**：可解释地貌骨架、可靠流向/洼地/平坦区处理、侵蚀近似与真正输沙过程明确命名，并在有来源/许可证/基准的条件下选择外部实现。
4. **生态缺额**：当前仍缺 3 株；调查空间容量和分层配额，不减少 plannedPlantCount 来换取通过。
5. **人文形态**：真实城区/乡村研究转化为有出处、尺度和约束的模板合同，关系图驱动地块/入口/公共设施，而非国家标签乘随机缩放。
6. **继续最大图实际验收**：完整地图生成、独立重建仍慢；多地貌/非零原点/高密度样例和全 240 块长时间漫游还需补齐。
7. **开源发布审计**：第三方代码、模型、纹理、字体、数据分别出版本锁定与通知清单。

### 可选，须在上述合同稳定后实施

- 二进制区块、压缩传输、跨块大 mesh 引用表；
- Web Worker 解码/校验与适度预取、带预算的 LRU；
- 将仍全量保留的 accessSurfaces/entities 继续投影；
- 地形/实体缓存统一预算与释放；
- 对孤立旧内容寻址文件设计离线 GC；
- 材质/资产升级、LOD 与视觉风格完善。

这些可选项不能成为推迟真实自然/人文生成的理由。下一阶段优先处理“土地已规划但没有真正建成”的生成缺口，并保持独立账本和失败证据。

## 8. 可复核证据

- `G:\difyllmwiki\GlyphWeave\bevy\work\r44-projection-parity.json`
- `G:\difyllmwiki\GlyphWeave\bevy\work\r44-authority-before.json`
- `G:\difyllmwiki\GlyphWeave\bevy\work\r44-authority-parity.json`
- `G:\difyllmwiki\GlyphWeave\bevy\work\r44-maximum-final-export-measurement\process.json`
- `G:\difyllmwiki\GlyphWeave\bevy\work\r44-maximum-browser.json` 及五张对应截图
- `G:\difyllmwiki\GlyphWeave\bevy\work\r44-small-projection-parity.json`
- `G:\difyllmwiki\GlyphWeave\bevy\work\r44-small-farm.png.json`
- `G:\difyllmwiki\GlyphWeave\bevy\work\r44-small-conservation.png.json`
- `G:\difyllmwiki\GlyphWeave\bevy\work\r44-workspace-final-tests.log`
- `G:\difyllmwiki\GlyphWeave\bevy\work\r44-node-tests.log`
- `G:\difyllmwiki\GlyphWeave\bevy\work\r44-js-checks.json`

最大图独立 gate 重跑的报告另见本文件末尾补记。导出前后 SHA 比较是在重跑 gate 之前完成；gate 会正常刷新审计报告，不能将审计报告的重写误报为导出修改世界。

## 9. 最终独立 gate 重跑（2026-09-07）

最终 release CLI（SHA256 `B8C16E22767D2053965BF9C6247A16E836CA8EFCC769DE2DE05A154B65CEEF55`）对原最大世界重新执行 world-quality-gate：

- 耗时 **166.353 秒**，退出 **1**，质量 **fail**；
- 采样专用内存峰值 **1,356,468,224 字节**；
- scale-audit 通过，规则实体 623/623 通过；
- 保留五类失败：用途面积实现不足、地表未实现、生态缺 3 株、用地规划/实现不足、road-first 开发不足；
- 自然路由数据完整性仍通过，但 1628 个闭合洼地、4 个未解决平坦终端仍不能称为真实水文求解。

因此 float_roundtrip 与分块导出没有掩盖原有 gate 失败，本轮也没有为获取通过而删实体、补虚构面积或降低要求。完整审计耗时仍约 166 秒，不能拿 3.951 秒的派生导出冒充它的优化结果。

证据：`G:\difyllmwiki\GlyphWeave\bevy\work\r44-maximum-gate-measurement\process.json`、该目录下 stdout/stderr，以及 `G:\difyllmwiki\GlyphWeave\bevy\work\r43-maximum-baseline\world-quality-gate.json`。重跑前的历史审计文件保存在 `G:\difyllmwiki\GlyphWeave\bevy\work\r44-prior-audit-reports`。

### 当前验证脚本的复现方式

r43 的最大图浏览器脚本已升级为分块模式验证，现在需要第三个参数指定全量投影保真报告；历史 r43 结果保留，不再把旧的仅两次 reload 测试当作当前验收。

```powershell
node --max-old-space-size=4096 'G:\difyllmwiki\GlyphWeave\scripts\check-render-stream.mjs' 'G:\difyllmwiki\GlyphWeave\bevy\work\r43-maximum-baseline' 'G:\difyllmwiki\GlyphWeave\bevy\work\r44-projection-parity.json'
node 'G:\difyllmwiki\GlyphWeave\scripts\check-maximum-preview.mjs' 'http://127.0.0.1:8864/preview/?view=services' 'G:\difyllmwiki\GlyphWeave\bevy\work\r44-maximum-browser.json' 'G:\difyllmwiki\GlyphWeave\bevy\work\r44-projection-parity.json'
```

第二条需要先启动该世界的 preview 服务。读取几何/矩阵的小图测试会由测试代码获取完整 scene 作独立参考，这是测试行为；最大图测试明确禁止浏览器请求完整 scene。
