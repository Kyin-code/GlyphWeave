# r37：生态审计正向闭环与矩形净空修复

日期：2026-09-07。权威仓库：G:\difyllmwiki\GlyphWeave。
状态：实现已落地；验证记录见末节。不是总体项目交付完成。

## 1. 复查发现与历史说明校正

1. r36 的 densityPerHa 只是固定规划参数，尚未参与实例数量计算。420/280/190 个每公顷与每个 256m² patch 的固定 3/2/2 个实例并不等价：实际实例密度分别为 117.1875、78.125、78.125 个/ha。因此不能称为“面积驱动的密度预算已实现”。
2. 本轮开始时的 community_by_patch 在填充前被用于数量检查，导致任意非空合法生态计划也被拒绝。
3. 原 conservation 测试只要求若干错误版本返回 Err；这样的测试无法区分“正确拒绝错误数据”和“拒绝所有数据”。新增合法原件与 JSON roundtrip 的正向断言，先实际复现了失败，再修复验证顺序。
4. r35 所谓“完整 footprint 净空”实际上是中心与四角各自所在的 1m 方格探测；细长道路在探测点之间穿过、或小障碍位于植物占地内部时仍可能漏检。五点探测不是完整矩形相交测试。
5. r35/r36 目录名不代表生成器语义版本已变更，历史输出仍记录 r34。本轮确实修改 WORLD_GENERATOR_REVISION 为 worldgen-2026-09-07-r37-ecology-exact-clearance-audit，并要求重新 build 与 fresh bake。

## 2. 实现与重构边界

### 2.1 验证顺序

先验证并填充群落索引（存在性、唯一性、分类、合法数值与计划数量），再核对植物层集合与群落 plannedPlantCount。

- 不降低 required layer 数；不删除植物来让检查通过。
- 单位仍区分：覆盖率属于 [0,1]；densityPerHa 为有限正数，不属于覆盖率数组。
- 不以 scene 与发布文件“彼此相等”代替独立重建。
- generation/verify 都继续使用 final BakedTerrain，不由 HTML 修改坐标或高度。

### 2.2 精确矩形净空

将已有 1m obstacle_intersects 数学计算抽成通用 footprint_intersects_entity：

- 查询体为以植物坐标为中心、widthM 为边长的轴对齐占地矩形；
- 障碍体为含 scale 与 rotationYDeg 的旋转矩形；
- 依次测试世界 X/Z 轴和障碍体两个局部轴上的投影重叠；
- 仅边界接触不算正面积重叠；保留 1e-9m 的既有数值容差；
- 非法/非有限几何保守视为冲突；
- 旧庭院/农业的 1m 检测通过该 helper 调用，保持原有数学含义；
- 生态障碍集合从建筑/道路/铁路/运河扩展到已有 solid 集合，包括岩石、巨石、墙、围栏、灯杆、倒木及已有树木等；显式水体实体也检查。

这保证的是已声明矩形占地的重叠检查，不是任意三维网格碰撞、树冠/根系生物学模型或物种生态适宜性。

### 2.3 生成端与审计端统一准入

全 workspace 首轮测试查出 unavailable_joint_housing_fails_gate_without_lowering_the_request 场景中的旧固定植物锚点会与现有硬实体重叠，fresh bake 报 ecology plant intersects hard entity。没有修改测试预期或放宽障碍集合。

新增 choose_ecology_position：优先原锚点；不合法时在不超过16m的来源 patch 内按确定性1m步长搜索合法候选。候选使用与最终验证相同的完整矩形、水体与障碍集合；最终高度仍由 terrain_height 采样。实例宽高、来源和必需层均不变，不会删除障碍或植物。289个备用格点均不可用时返回含 patch/layer 的明确错误。

这是有限候选搜索，不声称能找到连续空间中每个狭小可行位置；尚未实现可继续生成其他区域的生态 shortfall 账本。新增测试验证原点保留、替代点确定性、整块被占用时失败、超尺寸不得缩小。

### 2.4 不改变的合同

自然/最终高度、保留地所有权、学校原始尺寸、设施需求、最低住宅数、道路连接、旧庭院/农业面积统计、完整 quality gate 门槛均不因本轮修复而放宽。

## 3. 可重复的验证

### 核心测试

- 合法 conservation 输出必须通过 verify；JSON 序列化后再次通过。
- 15 种伪造逐个独立修改原件：删除/重复植物，错误来源/类型/层，移动高度/平面位置，扩大 footprint，篡改计划数/密度/面积，NaN/Infinity，删除/重复群落。
- 几何回归：细条穿过五点之间、内部小障碍、仅接触、完全分离、旋转包围盒假阳性、scale、正负大 origin、非法尺寸。
- 直接调用生态净空验证，确认 road/railway/canal/wall/rock/fence/tree 各类交叉均被拒绝；不是仅依靠位置与生成器不一致导致失败。

### 浏览器合同

保留原有 patch/面积/数量验收，并改进 check-preview-conservation.mjs：逐个对比实际 Three.js 根节点 ID 与 Rust 实例 ID、来源 patch、XYZ 与几何高度/半径；不能用数量相同但身份错误的实例冒充。脚本当前明确用于整个单 chunk 场景，不是多 chunk 流式渲染性能证明。

### CLI 集成测试

以 service-program-joint-plains.manifest.json fresh bake：

1. 原件 quality-report 必须成功执行（其中整体质量可诚实为 warn）。
2. 确认真实生成多层实例，不接受空集合证据。
3. 同时伪造 scene.json 与 landcover-surfaces.json，并写入假的成功摘要。11 种伪造必须由 independent realization 拒绝。
4. 只恢复 scene 时，仍必须识别 published 文件伪造。
5. 恢复两份原件后 quality-report 再次成功。

## 4. 尚未完成，禁止混称通过

- 密度仍未驱动面积与实例数量闭环；当前固定层实例只验证数量和来源，不代表目标覆盖率已实现。
- forest/wetland/shrubland 分类与水文标签仍是启发式，缺少水位季节性和完整群落演替。
- 已支持原 patch 内有限替代选址；仍缺少净空完全不可满足时的生态 shortfall 账本。
- 同群落植物彼此、树冠垂直重叠、任意 mesh 与实例化 LOD 尚未覆盖。
- 仍须完成农村/城市剩余地块、自然水文与地貌深化、许可证发布审核、6km×10km 性能/内存和跨 chunk 证明。

本轮无新增第三方算法代码、素材或依赖；数学 helper 源于项目既有矩形检测的参数化重构。未引入 Mapgen4/symbios-ground 源码，未改变其许可证关系。

## 5. 实测证据

本轮最终代码实际验证结果：

| 验证 | 结果 |
|---|---|
| Rust workspace | Core 315、App 44、CLI单元6、CLI集成18，共383通过 |
| CLI双份伪造专项 | 原件与恢复件接受，11类双份伪造及独立发布文件伪造均拒绝 |
| Vitest | 170通过 |
| Node高度/住宅/设施合同 | 27通过 |
| lint / typecheck | 退出0；lint仍有既有warning，不能声称零警告 |
| doc-tree:check / git diff --check | 通过 |
| fresh scale-audit | pass |
| fresh rules audit | 55/55 pass |
| fresh quality-report | 正常重建执行，质量状态warn |
| fresh world-quality-gate | 实际执行退出1，status=fail |
| 浏览器合同 | 192 patch，544个植物逐ID/来源/坐标/尺寸匹配，页面错误0 |

权威输出：G:\difyllmwiki\GlyphWeave\bevy\work\ecology-r37-exact-clearance-verified。

- 生成器版本：worldgen-2026-09-07-r37-ecology-exact-clearance-audit。
- 世界revision：59686ce7c07322a5e68df2ac693afe251594151b5d42839f818f5b5e2d7efba8。
- 192群落、544植物、49,152m²自然地表，数量没有靠删除来通过。
- 设施4/4、最低住宅18/18、29条道路专项通过。
- 农村仍157,504m²未实现；城市仍9,048m²未实现。
- 完整gate实际失败项：purpose-area allocation/realization、landcover realization、land-use planning/realization、road-first development仍未完成。
- 原始日志位于 bevy/work/r37-workspace-final-tests.log、r37-cli-ecology-test.log、r37-relocation-cli-test.log、r37-generation.log、r37-quality.log、r37-gate.log、r37-browser.log。
- 证据摘要为输出目录的r37-evidence.json；浏览器证据为ecology-preview.png.json，截图ecology-preview.png已实际查看。

HTML服务使用独立的 bevy/work/r37-preview-runtime/glyphweave.exe，文件SHA256与build产物一致，避免锁住target/debug导致后续build失败；预览端口8857。

**视觉未验收**：截图仍有地面偏白、草过密和代理模型简陋的问题。浏览器合同通过仅证明Rust几何/身份被正确消费，不能当作现代城市与自然环境最终视觉完成。
