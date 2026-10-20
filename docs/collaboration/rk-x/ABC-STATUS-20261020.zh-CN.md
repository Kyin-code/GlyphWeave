# A/B/C 当前状态总览（2026-10-21 更新）

本文件是 `requirements-and-progress-master.zh-CN.md` 的快照式导读；数值均来自
本次实际烘焙/测试，不把历史值拼进新口径。

## A 稳定底座

| 证据 | 结果 |
|---|---|
| 最大图 gate | `examples/maximum-6x10km-complete.manifest.json`：240 chunks / 623 entities / **gate status pass, failures 0** |
| 确定性 | 第二次烘焙 worldRevision 相同；1,777 个可比文件逐字节相同，mismatch 0 |
| 生态 | 342,380 / 342,380 / unfilled 0 |
| 预览 | HTML palette 已覆盖全部 landcover 材质，未知材质有具名 fallback，不再直接抛错 |
| 套件 | 第二次 `cargo test --workspace --no-fail-fast --quiet` **exit 0**；core lib **805 passed / 34 ignored**，所有 target 通过 |
| 仍缺 | 已知偶发 `0xC0000005` 未复现但也未修复；最大图 HTML 已做全战略图与 2.5D focus smoke，未做逐块手工长漫游 |

## B 真实街区/聚落

| 批次 | 证据 | 状态 |
|---|---|---|
| B-01 地块/建筑/入口 | 512m joint-plains complete：18/18 住宅有入口、4/4 服务、gate pass | **完整切片** |
| B-02 道路容量 | 独立供应控制：3 条自动路、collector+local 实网、frontage **24/24 realized-with-access**、road/tier audit pass、disconnect 0 | **完整控制闭合** |
| B-03 用途/地表 | complete-ground-v1：512m unresolved 128,542→**0**；最大图 ground cover 50,538,180m²、ledger 60,000,000/0 | 当前控制闭合 |
| B-04 形态/服务 | heightM 已驱动默认高度；serviceRadiusM 在 village-field 800m 下 4 服务/18 户全覆；coverageRatio 由 `morphologyCoverage=audit` 逐地块审计并命名 out-of-range/unrealized | profile-driven 核心完成 |
| B-05 集成 | 最大图 complete-ground + 工程版 gate pass；第二次烘焙逐字节一致；HTML max strategic/2.5D 无 console error | **完整集成通过** |

### B 的关键未闭合项

1. B-02 的旧失败控制保留为反例；新供应控制已 24/24 闭合。
2. 1km 探针的 120 户压力仍是输入/供给上限问题，不再作为 B-02 接口缺口。
3. `coverageRatio` 已接入 opt-in 只读审计；compact 24/24 地块控制已落入
   profile 范围并 `pass`，village-field 服务覆盖 18/18 户。

## C 自然—工程—人文

| 能力 | 证据 | 状态 |
|---|---|---|
| C-N1 来源矩阵 | `docs/design/natural-source-matrix.zh-CN.md` 逐函数列出；Mapgen4/symbios 未复制 | 初版 |
| 地貌生态接口 | `HabitatContext`/`HabitatSuitability` 纯函数 + 5 测试 | 接口完成，默认不改旧世界 |
| C-E1 挡墙/涵洞 | 山城控制 232 挡墙 / 5 涵洞，status pass；gate 检查 retainingWorks | 首版 |
| C-E2 工程后水文 | 自然 vs 最终 D8：87 changed flows / 5 raised terminals，status pass | 首版（路由完整性，不是洪泛模型） |
| C-L1 许可 | workerd-windows 已按 Apache-2.0 记录；Quaternius 已定位 Ultimate Buildings Pack (Dec 2019)，本地为派生导出；精确历史下载日期未知 | 来源完成，历史日期留证 |
| C-V1/G1 | HTML 战略图/2.5D 已实测；最大图工程版 gate pass；Godot 4.7.2 最大图 headless 加载 `scene-0 / chunk 0,1 / loaded 6` | **C-V1/C-G1 完成** |

### 2026-10-21 可见性补验

- `modern-lakeside-city-512` 新增为现代混合街区控制：29 条道路、27 个临街地块、
  68 个显式树实体、1,157 株生态植物、21,419m² 湖面。
- 可见对象：住宅 16x12/24x16、公寓块 24x20x32m/36x24x20m、店铺 12x10x6m；
  HTML 不再把这些 Rust 实体统一渲染成固定尺寸无窗盒子。
- 白色伪影根因已确认是地形顶点色的 sRGB/线性空间重复编码，并已修复；
  草场焦点圆形由 `uniformDensity` 消除。
- 详细证据见 `FIVE-ISSUES-CLOSURE-20261021.zh-CN.md`。

## 结论

- **A 的当前最大图质量门已经 pass，且确定性重跑通过**；A 的偶发崩溃和浏览器长漫游仍开放。
- **B 已有 512m 24/24 沿街供给控制、完整 512m 联合切片和最大图 gate pass**。
- **C 核心里程碑已完成**：最大图工程版 1,734 挡墙、734 改向节点、gate pass；
  C-L1 两项来源已解析；C-V1 的 Godot 4.7.2 runtime smoke 已通过。
