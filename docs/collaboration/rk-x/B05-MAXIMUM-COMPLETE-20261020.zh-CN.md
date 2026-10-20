# B-05 maximum-world complete-ground run (2026-10-20)

状态：**当前源码下 6x10 km 最大图 `world-quality-gate` 首次 status=pass / exit=0**。
这不是 B/C 全签收；它关闭了 A 遗留的四条 land-use gate 失败，并给出 B-05
最大图集成证据。

## 1. 输入与命令

| 项 | 值 |
|---|---|
| manifest | `examples/maximum-6x10km-complete.manifest.json` |
| 基础输入 | `examples/maximum-6x10km.manifest.json` + `residualLandProgram.coverage="complete-ground-v1"` |
| release binary | `bevy/target/release/glyphweave.exe` |
| 输出 | `bevy/work/rk-x/b05-maximum-complete-20261020/world/` |

```powershell
glyphweave generate-world examples/maximum-6x10km-complete.manifest.json <out>
glyphweave quality-report <out>
glyphweave world-quality-gate <out>
```

## 2. 实测

| 命令 | exit | 耗时 | 结果 |
|---|---:|---:|---|
| generate-world | 0 | **255.693 s** | 240 chunks / 623 entities / worldRevision `0bc8ece5…` |
| quality-report | 0 | **27.93 s** | status `pass`；warnings 0；access 156 connected / **0 disconnected**；scale 240/240 |
| world-quality-gate | 0 | **30.61 s** | status `pass`；failures **0** |

## 3. land-use / landcover 闭合

| 指标 | 值 |
|---|---:|
| landcover planned | 59,790,000 m² |
| landcover realized | 50,760,000 m² |
| conservation | 9,030,000 m² |
| landcover unresolved | **0 m²** |
| ground cover | **50,538,180 m²** |
| patches | 97,652 |
| ecology | **342,380 / 342,380 / unfilled 0** |
| ledger | realized **60,000,000** / unresolved **0** / status **pass** |
| residual | candidate 59,790,000 / realized 59,790,000 / unresolved **0** / status **pass** |
| budget | entityCoverageStatus **realized**；actualUrban 2,400,720 / actualRural 47,839,820 / actualNature 9,030,000 |
| development | status **pass**；incompleteReasons **[]** |

`realizedAreaM2` 是非 conservation 的 yard/农业/ground 合计；
`conservationGroundAreaM2` 单独列；两者合计覆盖 59,790,000 m² 的非硬开发
landcover 计划。

## 4. 与 A-closure 的差异

A-closure（未开 coverage）的 gate failures：

1. purpose-area allocation or realization
2. landcover remains unrealized
3. land-use planning or realization
4. road-first land-use development

本次 gate failures：**无**。当前 manifest 上 A 例外 #2/#3/#5/#6 不再出现；
#4 已在 A-closure 关闭；#1（自动路目标）在这份无 frontage/auto-road 目标的
最大图输入上不出现，不能据此单独宣称历史 #1 关闭。

## 5. 未验证

- ~~第二次确定性重跑~~ **已通过**：`b05-maximum-complete-repeat-20261020`
  与首次输出的 `worldRevision` 相同（`0bc8ece5…`）；1,777 个可比文件
  逐字节相同（mismatch 0 / missing 0），唯一差异是
  `generation-report.json` 的输出路径字段。
- ~~240 chunk HTML 连续漫游与视觉空置检查~~ **已补 smoke**：12×20
  全战略图渲染完成且 console error/warn 为空；2.5D 对非港场景改为
  Rust 内容 focus，最大图 focus 帧无黑洞/错位块。
- 偶发 `0xC0000005` 仍未修复；单次成功不等于套件稳定。

## 6. 证据

- `bevy/work/rk-x/b05-maximum-complete-20261020/bake.log`
- `bevy/work/rk-x/b05-maximum-complete-20261020/quality-report.log`
- `bevy/work/rk-x/b05-maximum-complete-20261020/world-quality-gate.log`
- `world/scenes/scene-0/landcover-audit.json`
- `world/scenes/scene-0/land-use-ledger.json`
- `world/scenes/scene-0/land-use-budget.json`
- `world/world-quality-gate.json`
