# r45：自然地形上的道路纵断面工程核

日期：2026-09-07。权威仓库：`G:\difyllmwiki\GlyphWeave`。

## 1. 为什么本轮先做这个，而不是再改渲染

r44 已解决单块近景读取巨型 scene.json 的问题，但没有让城乡开发自动成立。本轮重新读取最大图的真实输出，得到以下拒绝原因：

- 道路候选 28,767；因局部地形限制拒绝 7,191；合法候选中未选中 16,765；最终选中 246 条道路边。
- 当前整张连通路网被约束到同一个 **5m** 标高。
- 农田仅实现 270 块；**51,635 个农田候选因附近没有道路被拒绝**。
- 另有 447 个候选超过田面 12% 坡度，还有道路通道坡度/地块冲突等失败。
- 城市 future_development 仍有 2,400,720m²，农业候选 47,754,792m²。

因此，现阶段主要阻塞不是“缺少更多随机田块”，也不是全部候选都因为自然山体太陡，而是路网无法跨越正常地形起伏。扩大农田搜索、拉长无路通道上限或填平自然高度来迁就一个公共标高，都会偏离目标。

证据：`G:\difyllmwiki\GlyphWeave\bevy\work\r45-generation-bottleneck.json`。

**范围声明：本轮实现的是可验证的道路纵断面求解与设计网格，还没有接入自动路网和最终地形写入。** 不能声称已经多生成一条道路、实现更多城乡面积或让质量门禁通过。这里明确记录这项未完成集成，而不是重新定义最终交付目标。

## 2. 新工程核的职责

新增 `G:\difyllmwiki\GlyphWeave\bevy\crates\core\src\worldgen\road_alignment.rs`：

- 输入固定水平走廊、自然高度栅格、保护分区和自然水体；
- 每 1m 一个纵向站点，读取包含路肩的**完整横断面**，而非中心线/端点探测；
- 计算随自然起伏变化的、横向平整的路面纵断面；
- 支持明确的起点/终点接驳平台标高和平台长度，不能暗自修改这些约束；
- 输出道路 mesh、支撑走廊 mesh、源栅格哈希、分区哈希及预计填土指标；
- 独立复核每一个站点的自然下界、填土上界、坡度和接驳约束；
- 全部结果标为 `design_only_not_applied`，不触碰原始/最终高度、实体、土地账本或 gate。

Agent 不计算路面逐点标高；未来 Agent 只表达道路用途、起讫关系和场景要求，由 Rust 决定几何与失败原因。

### 2.1 当前不可放宽的约束

- 道路宽度：偶数，4–32m；单侧路肩：0–8m。
- 本研究输入为 8m 道路 + 两侧各 3m 路肩，共 14m 支撑走廊。
- 填土上限最多 6000mm；请求可更严格，但不能超过现有 6m 工程上限。
- 每 1m 站点高差最多 80mm，即纵坡 8%；请求可更严格但不能增大。
- 8% 是本项目的生成合同，不声称等同某国道路规范；现有 asset 规则中的坡度角度、农业 8% 通道合同也不能混淆。
- 不挖低自然地形；保护/未知用途不能改成建设用地；分区缺口拒绝。
- 保护边缘继续使用现有每远离 1m 最多增加 0.5m 填土的 envelope。
- 连续支撑矩形与自然水体相交即拒绝，不依赖整数站点恰好命中水面，也不自动虚构桥梁。
- 当前仅支持整数坐标的轴向直走廊，长度不超过 10km；非轴向输入明确拒绝。

这些是必要基础，并非足够的道路施工或通行安全认证。

## 3. 算法与精度合同

### 3.1 每站上下界

设第 i 站全宽自然高度为 `h(i,j)`，以整数毫米读取：

- 下界 `L(i) = max_j h(i,j)`，保证横向平整后不切入自然地面；
- 上界 `U(i) = min_j [h(i,j) + min(请求填土上限, 保护边界容量)]`；
- 如果该站属于显式接驳平台，则同时把上下界与指定标高相交。

新增 `G:\difyllmwiki\GlyphWeave\bevy\crates\core\src\baked_terrain.rs` 中的 `height_millimetres()`，直接返回存储整数；旧四分之一米编码精确乘 250，新毫米编码直接返回原值。没有经由 f32 读数再 ceil，避免 `3.861000061...` 被保守上取整成错误的 3862mm。既有 height() 查询路径没有被替换。

### 3.2 给定走廊上的最小可行纵断面

对于每米最大上升/下降量 g，求：

`Y(i) = max_j [L(j) - g * |i-j|]`

Rust 使用两次整数扫描求解：先从左传播高点的坡度影响，再从右传播。时间 O(N)，内存 O(N)。这不是侵蚀模拟，也不是复制某个开源地图生成器的实现。

性质：

1. `Y(i) >= L(i)`；
2. 邻站高差不超过 g；
3. 任意满足上述约束的纵断面，都不能在某个站点低于 Y(i)；
4. 若某站 `Y(i) > U(i)`，则在固定走廊、给定平台和上述约束下不存在可行纵断面。

因此“不可行”不是随机搜索没找到，而是这组一维约束冲突的证据。但它不表示换一条水平路线、加合法桥隧、重新规划路口后仍不可行。

Rust 验证器不重跑两次扫描来相信自己，而是检查全宽源上下界和局部最优性等式；测试还用 O(N²) 全枚举公式作为不同实现的 oracle。独立 Node 脚本同样使用 O(N²) 公式直接读取原始高度文件复核。

### 3.3 几何与数量

- 每个纵向站点在左右边界各生成一个顶点；邻站之间两片三角形。
- 道路 mesh 和含路肩支撑 mesh 分开输出，两个 mesh 不是两份土地面积。
- 支持正/反 X 和正/反 Z，三角形法线朝上。
- 标高毫米转米只发生在输出顶点时；不对源顶点重新生成噪声。
- `supportAreaM2` 是设计走廊水平投影面积，**不是世界的 realized area**。
- `proposedFillVolumeM3` 是走廊内逐 1×1m 单元积分的预计填土量：自然源用四角双线性插值，路面沿站点线性插值。这个模型与后续可能采用的自然三角剖分积分必须明确区分。
- 该量不含走廊外边坡、挡墙、土源/压实、挖填平衡或排水设施，不能充当真实工程量清单；尚未将任何土方写入最终地形。

## 4. CLI：只能设计/核对，不能覆盖世界

新增命令的实现拆入：`G:\difyllmwiki\GlyphWeave\bevy\crates\cli\src\road_corridor.rs`，避免继续把主 CLI 文件堆成一个更大的函数。

```powershell
& 'G:\difyllmwiki\GlyphWeave\bevy\target\release\glyphweave.exe' design-road-corridor 'G:\difyllmwiki\GlyphWeave\bevy\work\r43-maximum-baseline' 'G:\difyllmwiki\GlyphWeave\examples\urban-road-corridor-study.json' 'G:\difyllmwiki\GlyphWeave\bevy\work\r45-urban-road-corridor.json'

& 'G:\difyllmwiki\GlyphWeave\bevy\target\release\glyphweave.exe' verify-road-corridor 'G:\difyllmwiki\GlyphWeave\bevy\work\r43-maximum-baseline' 'G:\difyllmwiki\GlyphWeave\examples\urban-road-corridor-study.json' 'G:\difyllmwiki\GlyphWeave\bevy\work\r45-urban-road-corridor.json'
```

第一条要求新输出路径。如果示例文件已经存在，命令必须报错，请使用新的研究输出名，不能覆盖旧证据。

- 验证请求 scene 与 manifest 的范围一致；
- 只读取自然高度引用，不误用已经有人类整地的最终高度；
- 验证整个自然栅格的 content hash，而非只检查走廊内数据；
- canonicalize 场景/高度路径并阻止源路径逃出 world；
- 输出必须在权威 world 外，且必须为新文件；
- 对已存在输出、试图写入 world 的路径明确拒绝；
- verify 命令不修复损坏文件，不将设计状态改成 built。

这个命令可针对已有 baked world 的自然层做下一轮规划研究，不代表已经完整验证该 world 里所有现有人造障碍、路线相互关系或最终可达性。

## 5. 真实最大图上的结果，不替换 seed 或阈值

输入取 r43 原最大图中的城市横向走廊：

- 起点 `(2496, 5000)`；终点 `(3520, 5000)`；长度 1024m。
- 道宽 8m，含路肩支撑宽 14m。
- 读取跨 3 个真实 chunk 的 **15,375 个原始自然样本**。
- 自然高程范围 **−8.652 到 6.722m**。
- 若要求原来的整数等高平台，最低平台为 7m，至少需要 **15.652m** 填土，超过 6m 上限。
- 新纵断面最大设计填土 **2.578m**，最大纵坡 **8%**，预计走廊内填土 **9350.1415m³**。
- 道路和支撑网格各 **2048 个三角形**；支撑投影 14,336m²。

同一自然源、同一水平走廊，既不填平整张地图，也不放宽 6m 上限，已能形成可核对的非等高纵断面。这是替换公共标高瓶颈的实际算法证据，而不是只新增 schema。

但是，该段没有被写回 world，也没有成为道路实体/农业可达图的一部分。因此最大图道路仍是 246 条、农田仍为 270 块、完整 gate 仍保留原有失败；这些数字不能被本研究的投影面积抵扣。

与此前保存的源快照比较，当前 scene 源、自然/最终 payload、world/manifest 共 **1463 个文件 SHA256 不变**。证据：`G:\difyllmwiki\GlyphWeave\bevy\work\r45-authority-parity.json`。

## 6. HTML 研究图与复核

新增脚本：`G:\difyllmwiki\GlyphWeave\scripts\check-road-corridor.mjs`。

它独立读取原始高度文件，检查所有站点、全宽分区与填土限制、纵断面全枚举 oracle、mesh 顶点/三角形/朝向/面积、土方指标，再输出只读 HTML 纵横断面对照图。

- `G:\difyllmwiki\GlyphWeave\bevy\work\r45-urban-road-proof.json`
- `G:\difyllmwiki\GlyphWeave\bevy\work\r45-road-study.html`
- `G:\difyllmwiki\GlyphWeave\bevy\work\r45-road-study.png`
- `G:\difyllmwiki\GlyphWeave\bevy\work\r45-road-study-browser.json`

HTML 已用实际 Edge 渲染、截图并检查，两个图表可见、无横向溢出、无 pageerror。页面明确写“尚未应用到地图”，不能把工程研究图当作现代社会整图最终效果。

## 7. 测试与边界

新增 Rust 测试覆盖：

- 两次扫描与多组 O(N²) oracle 一致；紧一毫米的上界必须拒绝；
- 长上坡的非等高道路成立，而 6m 内单平台不可能成立；
- 路肩隐蔽高点不能被中心线采样漏掉；
- 显式接驳平台保持标高/长度，冲突请求拒绝；
- 四个方向、非零原点、整数毫米读取、已知体积和朝上三角形；
- 保护区、越界/非法尺寸、超限请求、损坏 profile/mesh/土方/状态/来源；
- 水域不得静默桥接，缺失栅格不得按零高度使用；
- CLI 设计/验证、禁止覆盖/写入 world、源文件不变、伪造数据拒绝；
- 改动走廊外的自然栅格同样因全源哈希不符被拒绝。

完整测试结果以本文件末尾补记和实际日志为准，不将还在执行的测试算作通过。

## 8. 下一步必须接入的地方

纵断面工程核只是替换错误前提的第一部分，下一轮的优先事项不是继续增加旁路诊断：

1. **共享路口约束**：同一拓扑节点不能被相邻路段各自赋不同高度；显式平台应从道路图统一规划。
2. **非等高道路合同**：RoadNetworkPlan/RoadEdge 不能再只靠一个 levelM；纵断面要有版本、来源和独立重建验证。
3. **最终地形工程应用**：平整路面与两侧过渡必须进入最终 raster，保留自然层不动；旧矩形常数标高 carve 需要拆分。走廊外边坡/保护边缘/邻路叠加仍要验证，不能直接贴一个浮空 mesh。
4. **沿道路标高的入口/设施**：服务选址目前也取全局 levelM，需要按真实连接点标高规划入口与平台；学校、住宅最低数量仍须满足。
5. **农业接入实际路网**：农用通道只能连已经实现并通过最终地形验证的道路，不能连本轮研究 JSON。
6. **HTML 和独立审计同步**：只显示真实已应用的 profile/mesh；外部核对要重建最终路面、连通性、坡度、土方和保护域。不能仅验证 profile JSON 自洽就授予世界通过。
7. **重新运行同输入大地图**：保留需求，核对道路覆盖、城市/农村实际面积、农田和生态缺额；若仍失败，继续暴露拒绝原因。

竖曲线舒适性、排水/路拱、桥隧、水平曲线、完整土方供需与物理水文可分层实施，但在缺少它们时必须明确限制，不得把算法称为完整道路工程或真实侵蚀。

## 9. 来源和许可证

该求解、精度 accessor、CLI 和诊断图均为 GlyphWeave 自有代码。本轮没有新增 Rust/npm 依赖，没有复制 Mapgen4、symbios-ground 或其他自然地形仓库源码，没有下载模型或纹理。

数学合同和最小性证明在本文自洽给出，测试含独立全枚举公式。当前第三方授权关系没有变化；本模块不是一次“已集成外部侵蚀库”的更新。原定开源发布时对现有依赖、模型、纹理、字体和数据的独立清单/通知审计仍然必须完成。

## 10. 最终验证补记

本轮最终代码的完整 Rust workspace 测试：**415 passed**（core 342、app 44、CLI 6、CLI integration 23），1 个既有微基准 ignored。日志：`G:\difyllmwiki\GlyphWeave\bevy\work\r45-workspace-tests.log`。

Node 41 项、Vitest 170 项通过；lint、typecheck、doc-tree:check、git diff --check 通过。lint 保留既有警告，未声称零警告。最终 release CLI 已对真实 1024m 研究文件执行 verify-road-corridor，退出 0；证据：`G:\difyllmwiki\GlyphWeave\bevy\work\r45-urban-road-verify.log`。

本轮没有重新运行最大图 generate-world，也没有声称 world-quality-gate 转绿。因为尚未改变已实现世界，r44 的完整门禁失败仍是当前质量状态。整体开发目标仍在进行中。
