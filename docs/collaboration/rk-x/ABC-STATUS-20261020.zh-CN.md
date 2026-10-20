# A/B/C 当前状态总览（2026-10-20）

本文件是 `requirements-and-progress-master.zh-CN.md` 的快照式导读；数值均来自
本次实际烘焙/测试，不把历史值拼进新口径。

## A 稳定底座

| 证据 | 结果 |
|---|---|
| 最大图 gate | `examples/maximum-6x10km-complete.manifest.json`：240 chunks / 623 entities / **gate status pass, failures 0** |
| 确定性 | 第二次烘焙 worldRevision 相同；1,777 个可比文件逐字节相同，mismatch 0 |
| 生态 | 342,380 / 342,380 / unfilled 0 |
| 预览 | HTML palette 已覆盖全部 landcover 材质，未知材质有具名 fallback，不再直接抛错 |
| 套件 | `cargo test --workspace --no-fail-fast` **exit 0**，1613.77s；core lib **801 passed / 34 ignored**，所有 target 通过 |
| 仍缺 | 偶发 `0xC0000005` 未修；240 chunk HTML 连续漫游未做 |

## B 真实街区/聚落

| 批次 | 证据 | 状态 |
|---|---|---|
| B-01 地块/建筑/入口 | 512m joint-plains complete：18/18 住宅有入口、4/4 服务、gate pass | **完整切片** |
| B-02 道路容量 | 独立供应控制：3 条自动路、collector+local 实网、frontage **24/24 realized-with-access**、road/tier audit pass、disconnect 0 | **完整控制闭合** |
| B-03 用途/地表 | complete-ground-v1：512m unresolved 128,542→**0**；最大图 ground cover 50,538,180m²、ledger 60,000,000/0 | 当前控制闭合 |
| B-04 形态/服务 | heightM 已驱动默认高度；serviceRadiusM 在 village-field 800m 下 4 服务/18 户全覆；coverageRatio 由 `morphologyCoverage=audit` 逐地块审计并命名 out-of-range/unrealized | profile-driven 核心完成 |
| B-05 集成 | 最大图 complete-ground gate pass；1km/多 seed 旧证据保留 | 最大图通过，1km 临街供给仍缺 |

### B 的关键未闭合项

1. B-02 的旧失败控制保留为反例；新供应控制已 24/24 闭合。
2. 1km 探针的 120 户压力仍是输入/供给上限问题，不再作为 B-02 接口缺口。
3. `coverageRatio` 已接入 opt-in 只读审计；当前 compact/village 控制真实结果
   低于 profile 目标，审计如实报 `warn`。

## C 自然—工程—人文

| 能力 | 证据 | 状态 |
|---|---|---|
| C-N1 来源矩阵 | `docs/design/natural-source-matrix.zh-CN.md` 逐函数列出；Mapgen4/symbios 未复制 | 初版 |
| 地貌生态接口 | `HabitatContext`/`HabitatSuitability` 纯函数 + 5 测试 | 接口完成，默认不改旧世界 |
| C-E1 挡墙/涵洞 | 山城控制 232 挡墙 / 5 涵洞，status pass；gate 检查 retainingWorks | 首版 |
| C-E2 工程后水文 | 自然 vs 最终 D8：87 changed flows / 5 raised terminals，status pass | 首版（路由完整性，不是洪泛模型） |
| C-L1 许可 | workerd-windows 已按 Apache-2.0 记录；Quaternius 已定位 Ultimate Buildings Pack (Dec 2019)，本地为派生导出；精确历史下载日期未知 | 来源完成，历史日期留证 |
| C-V1/G1 | HTML 战略图/2.5D 已实测；**最大图工程版 gate pass**；Godot 未运行 | C-G1 完成，C-V1 仅 HTML |

## 结论

- **A 的当前最大图质量门已经 pass，且确定性重跑通过**；A 的偶发崩溃和浏览器长漫游仍开放。
- **B 已有 512m 24/24 沿街供给控制、完整 512m 联合切片和最大图 gate pass**。
- **C 核心已从“未实质开工”进入“自然/工程/水文闭环首批实现”**：最大图工程版
  1,734 挡墙、734 改向节点、gate pass；C-G1 已关闭。C-L1 的 workerd 与
  Quaternius 包来源均已解析，Quaternius 仅历史下载日期未知；C-V1 的 Godot 未运行。按用户当前“渲染不做要求”，
  HTML 预览已验收空置/loading 行为，Godot 留作发布前一次最小场景验收。
