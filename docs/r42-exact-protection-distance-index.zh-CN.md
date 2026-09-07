# r42：保护区填土上限的精确距离索引

日期：2026-09-07。工作目录：G:\difyllmwiki\GlyphWeave。

## 1. 目标与现状

此前 ZoningMask.fill_capacity_at 每查询一个地形采样点就扫描全部zone，筛出所有非urban/rural矩形，计算到其受保护栅格的距离。规划预检和最终工程高度都调用该查询；重复扫描成本随zone数量增长。

本轮优化这个明确热点，不改变自然生成、土地保护条件、道路/建筑尺寸或农业参数。不能通过扩大可填土范围换取更容易生成或更快的结果。

## 2. 必须保持的数学合同

旧公式对每个保护矩形的**闭区间栅格索引**计算：

    dx = max(minX - x, x - (rightExclusive - 1), 0)
    dz = max(minZ - z, z - (bottomExclusive - 1), 0)
    capacity = 0.5 * min_over_protected_rectangles(max(dx, dz))

这是Chebyshev距离，不是欧氏距离，也不是一般连续几何边界距离。右/下边界减1的规则保持不变。

- 保护栅格内容量0；
- 没有保护区时正无穷；
- 未知用途与nature/unassigned一样仍保护；
- 保留场景外halo查询；
- 整数坐标计算使用i64，转换到f64并乘0.5的时机不变。

## 3. 实现与职责

新增 worldgen/zoning_distance.rs，作为zoning模块私有子模块。

- 构建时提取保护矩形一次，不改变原zones和hash。
- 以包围盒较长轴的矩形中心中位数分割，形成平衡二叉层次包围盒；最多4个矩形构成叶节点。
- 内部节点包围盒到查询点的距离，是全部子矩形距离的下界。
- 先遍历距离较近的子树，得到更小上界后剪枝；只有下界不小于当前最好结果时才剪枝，不以近似阈值省略检查。
- 查询不分配堆内存，用对数深度递归；额外常驻存储与保护矩形数线性相关，不分配整个60km²的距离栅格。
- 极端重叠或交错布局可能降低剪枝收益，最坏情况仍要检查所有叶子；不承诺每次O(logN)。

本轮保留 protected_at 的行索引、check 的全footprint所有权验证和 audit_protection 的全像素复查。后两者仍可能有性能瓶颈，不能把本轮索引称为完整大地图性能优化完成。

## 4. 等价性验证

1. 空集合、单栅格、精确边缘、i32极端坐标。
2. 1/4/5/16/127/1024个随机矩形，与独立旧扫描公式逐查询比较；反转输入顺序不改变答案。
3. 128×96m完整分区及halo，每一点核对旧公式；覆盖正负十亿origin、全建设/全保护/混合/未知用途。
4. 6000×10000m域上128m分区、尾块不满128m的布局；比较完整扫描与索引答案并统计实际叶子检查次数。
5. 最终fresh bake与r41同manifest输出比较，要求实体、生态、所有SurfacePatch、自然高度和最终高度不变。

语义保持等价，因此world generator revision继续r41；性能阶段名称r42不代表自然算法或文件格式变化。

## 5. 可复现的独立性能实验

显式执行：

    cargo test --release --manifest-path bevy/Cargo.toml -p glyphweave-core benchmark_maximum_domain_protection_distance -- --ignored --nocapture

测试域6000×10000m，使用每三块一块保护的128m布局和200000个确定性查询。分别测构建、旧扫描、新索引耗时，并逐结果assert相等。输出数组仅用于实验验证，不存在于生产查询热路径。

这是**保护距离查询微基准**，不是Rust生成完整60km²世界、完整保护审计耗时、峰值进程内存或最终HTML FPS证明。索引capacity字节数只包含矩形/节点Vec的预留存储，不是整个应用峰值内存。

## 6. 许可证与未完成项

自有空间索引与现有数学查询的重构，无新依赖/第三方代码/资产。Mapgen4与symbios-ground状态未改变。

总体目标仍缺自然地貌与水文深化、农业整地排水与城市剩余土地、全6km×10km生成/审计/内存测试以及视觉验收。r41的尺寸可控也不能替代这些任务。

## 7. 实测结果

### 7.1 三次release实测

| 运行 | 索引构建μs | 旧扫描μs | 新查询μs | 加速比 |
|---|---:|---:|---:|---:|
| 1 | 136 | 324653 | 30531 | 10.63× |
| 2 | 128 | 329564 | 30658 | 10.75× |
| 3 | 128 | 345229 | 40513 | 8.52× |

每次均为1238个保护矩形、200000次查询，并逐项assert新旧答案相等。索引Vec预留容量共96960字节。时间为本机实验测量，不保证所有输入布局均有相同比率；不是整个地图生成加速比。

### 7.2 最终回归与等价bake

- Rust workspace：Core331通过（1个默认ignored的微基准已另外显式release运行3次）；App44、CLI单元6、CLI集成20，共401项常规测试通过。
- Node34、Vitest170通过；lint退出0但仍有既有warning，typecheck/doc-tree/diff-check通过。
- 使用examples/multiscale-farms.manifest.json fresh bake，新目录bevy/work/farm-r42-equivalence-verified。
- 与r41同输入比较：scene.json、landcover-surfaces.json、road-network-plan.json、自然高度和最终高度bin全部逐字节相同。
- scale-audit、对象规则通过；quality-report保持warn；world-quality-gate实际退出1、fail，未改变其门槛或未实现面积。
- 浏览器农业核对18田块、6绕行、96农业mesh；生态核对192自然patch、1698植物、3批次并连续重绘，均无错位或重复且页面错误0。
- farm-preview.png已实际查看；仍有偏白地面、过密旧草及简陋模型，视觉尚未最终验收。

### 7.3 证据路径

权威输出：G:\difyllmwiki\GlyphWeave\bevy\work\farm-r42-equivalence-verified。

- r42-evidence.json保存微基准、等价比较和总体未完成状态；
- farm-preview.png.json与ecology-preview.png.json保存浏览器核对；
- bevy/work/r42-benchmark-final-{1,2,3}.log保存三个release微基准原始输出；
- bevy/work/r42-workspace-tests.log保存完整测试；
- bevy/work/r42-generation.log、r42-quality.log、r42-gate.log保存fresh CLI输出；
- 预览端口8862，来自独立bevy/work/r42-preview-runtime/glyphweave.exe。

总体路线图仍未完成。本轮只消除了一个重复扫描热点，不能替代最大60km²完整生成/审计与峰值内存证明。
