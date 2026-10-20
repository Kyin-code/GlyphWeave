# 自然算法来源矩阵（C-N1）

状态：2026-10-20 初版。只登记仓库中实际存在的函数；不把研究文档里的未来方案写成已实现能力。

图例：`self` = GlyphWeave 自写；`reference` = 只参考设计/论文，未复制代码；`approx` = 明确的领域近似，不是完整物理模型。

| 函数 / 接口 | 位置 | 来源 | 输入 → 输出 | 确定性 / 近似边界 | 决策 |
|---|---|---|---|---|---|
| `value_noise2d` | `worldgen.rs:4490` | self | seed, world x/z → 0..1 | 纯哈希 + 插值；按世界坐标确定；不是侵蚀 | 保留 |
| `fbm2d` | `worldgen.rs:4525` | self | seed, x/z → 分形噪声 | 固定 octave/频率；不是水文 | 保留为骨架基元 |
| `mountain_skeleton` | `worldgen.rs`（`approximate_valley_carve` 之后） | self | seed, x/z → 山脊/谷地骨架 | `|fbm|` 反相 ridge noise；非 Voronoi、非真实褶皱 | 保留 |
| `steppe_hills_field` | `worldgen.rs`（`mountain_skeleton` 之后） | self | seed, x/z → 缓丘高度 | 240m 网格抛物面丘累加；非侵蚀 | 保留为 steppe 风格 |
| `landform_field` | `worldgen.rs:4675` | self | seed, x/z, water → 连续地貌标量 | 径向裙边 + 水-aware 平滑带；无真实岩性/迎风背风 | 保留；可拆分气候/岩性 |
| `approximate_valley_carve` | `worldgen.rs`（`landform_field` 之前） | self / **approx** | seed, x/z, water → 下切高度 | 邻域噪声零线近似河谷，**不是**水流/泥沙模拟；城市范围内置零 | 保留但必须继续标注近似边界 |
| `terrain_height_metres_core` | `worldgen.rs:5582` | self | seed, x/z, water, landform → 米高度 | 组合 value/fbm/山脊/丘陵/近似河谷；只依赖 world 坐标，分块连续 | 唯一高度采样入口 |
| `water_geometry` | `worldgen.rs:1607` | self | water kind, landmarks, scene, style → 解析水域 | lake/river 解析式；不是 GIS 水体；岸线/宽度审计共享同一几何 | 保留；未来由 GIS/水文图替换输入 |
| `lake_radius_at` | `worldgen.rs:6312` | self | x/z → 归一化湖半径 | 椭圆/旋转近似；不含水深、岸坡或沉积 | 保留 |
| `river_half_width_at` | `worldgen.rs:4477` | self | z, scene depth, base width → 半宽 | 沿 z 的平滑变化；不是汇流演算 | 保留；接口可换为流域宽度 |
| `derive_drainage_graph` | `natural.rs:167` | self / **approx** | terrain, scene → 32m 采样流向图 | 8 邻域最陡下降；平地和闭合洼地显式命名；不是水量/洪泛模型 | 保留为 D8 路由完整性层 |
| `audit_drainage_graph` | `natural.rs:300` | self | DrainageGraph → 完整性审计 | 只证明无环、边合法、面积守恒；不证明防洪安全 | 保留；C-E2 加工程后反馈 |
| 自然保留地表分类 | `settlement/landcover.rs`（conservation pass） | self | 最终 terrain, water, 16m 网格 → ground/forest_floor/wetland | 坡度/低洼/水相交近似；不是群落演替 | 保留；可接 habitat 接口 |
| 生态密度 `profile` | `settlement/ecology.rs:99` | self / empirical constants | Material → 层密度/权重 | forest 420/ha、wetland 280/ha、shrubland 190/ha；经验值，不是冠层覆盖率 | 保留；来源等级标 heuristic |
| `HabitatContext` / `classify` | `settlement/ecology_habitat.rs` | self | slope, elevation, water distance, landform → forest/shrub/wetland/meadow/rock suitability | 一阶确定性规则；陡坡禁止湿地；近水平地偏湿地；高山偏灌/岩；默认不改变旧世界 | **新增 C-N1 接口**；后续可替换为物种/气候模型 |

## 1. 外部参考与许可

| 参考 | 状态 | 说明 |
|---|---|---|
| Red Blob Games Mapgen4 | 仅参考，**未复制代码** | `docs/mapgen4-hydrology-plan.zh-CN.md` 记录其 `map.ts` 为 Apache-2.0；当前 value/fbm/ridge/D8 均为自写 |
| symbios-ground | 仅研究参考，未复制 | 自然地形研究 §2；没有 vendor 源码 |
| 真实 GIS / DEM | 未接入 | 需要 CRS、轴序、高程基准、NoData、精度、来源年份和许可；C-N1 不冒充观测 |

## 2. C-L1 未决项

| 项 | 当前证据 | 仍未知 |
|---|---|---|
| Quaternius 建筑包 | `assets/glyphweave.registry.json` 的 `prop.building*` 写明 “CC0-1.0 declared by Quaternius building pack pages; exact local pack provenance unresolved”；`assets/third_party/quaternius/buildings/NOTICE.txt` 是本地说明 | 本地 `2Story_GableRoof.obj` 等文件的精确上游包/下载批次/校验和 |
| `@cloudflare/workerd-windows-64@1.20260701.1` | package lock / 许可库存中登记；A-09 一直带着该未决项 | 该版本包的精确许可文本与再分发义务 |

在两项清零前，C-L1 不能宣称完成。
