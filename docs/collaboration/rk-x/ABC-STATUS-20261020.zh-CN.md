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
| 仍缺 | 偶发 `0xC0000005` 未修；240 chunk HTML 连续漫游未做 |

## B 真实街区/聚落

| 批次 | 证据 | 状态 |
|---|---|---|
| B-01 地块/建筑/入口 | 512m joint-plains complete：18/18 住宅有入口、4/4 服务、gate pass | **完整切片** |
| B-02 道路容量 | tier id/role 持久化到 edge/extension/entity；`roadTierAudit` 进 gate；512m 控制 accepted 2、unserved 0、road audit 33/33 pass | 接口完成，frontage 供给 0/24 未闭合 |
| B-03 用途/地表 | complete-ground-v1：512m unresolved 128,542→**0**；最大图 ground cover 50,538,180m²、ledger 60,000,000/0 | 当前控制闭合 |
| B-04 形态/服务 | heightM 已驱动默认高度；serviceRadiusM 在 village-field 800m 下 4 服务/18 户全覆；coverageRatio 仍 unsupported | 部分 |
| B-05 集成 | 最大图 complete-ground gate pass；1km/多 seed 旧证据保留 | 最大图通过，1km 临街供给仍缺 |

### B 的关键未闭合项

1. `b02-road-supply.manifest.json` 的 frontage 仍 0/24；B-02 不能整体签收。
2. 1km 探针仍是 43 pad / 15 homes / 24 shops；120 户需求是输入/供给上限问题。
3. coverageRatio 没有不破坏既有 parcel 合同的实际消费者。

## C 自然—工程—人文

| 能力 | 证据 | 状态 |
|---|---|---|
| C-N1 来源矩阵 | `docs/design/natural-source-matrix.zh-CN.md` 逐函数列出；Mapgen4/symbios 未复制 | 初版 |
| 地貌生态接口 | `HabitatContext`/`HabitatSuitability` 纯函数 + 5 测试 | 接口完成，默认不改旧世界 |
| C-E1 挡墙/涵洞 | 山城控制 232 挡墙 / 5 涵洞，status pass；gate 检查 retainingWorks | 首版 |
| C-E2 工程后水文 | 自然 vs 最终 D8：87 changed flows / 5 raised terminals，status pass | 首版（路由完整性，不是洪泛模型） |
| C-L1 许可 | Quaternius 建筑包、workerd-windows 两项仍未解析 | 未完成 |
| C-V1/G1 | Godot 未运行；最大图工程版未跑 | 未完成 |

## 结论

- **A 的当前最大图质量门已经 pass，且确定性重跑通过**；A 的偶发崩溃和浏览器长漫游仍开放。
- **B 有了完整 512m 纵向切片和最大图 gate pass**，但 B-02 的 1km 沿路临街供给仍不闭合，因此 B 不能整体签收。
- **C 已从“未实质开工”进入“首批自然/工程实现”**，但 C-L1/C-V1/C-G1 和最大图工程版仍未完成。
