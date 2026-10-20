# B-01/B-04 512m complete slice (2026-10-20)

状态：**512m 联合平原控制 `world-quality-gate` pass**；这是 B 的一个完整纵向
切片，不代表 1km/最大图所有内容都已闭合。

## 1. 输入

- manifest：`examples/b01-joint-plains-complete-512.manifest.json`
- 基础输入：`examples/service-program-joint-plains.manifest.json`
- 唯一新增：`residualLandProgram.coverage="complete-ground-v1"`
- 输出：`bevy/work/rk-x/b01-joint-plains-complete-20261020/world/`

该控制没有 `frontageProgram`；它走的是 settlement/joint programme 的
独立地块—建筑—入口链路，因此不是 B-02 沿路临街供给的证据。

## 2. 实测

| 命令 | exit | 耗时 | 结果 |
|---|---:|---:|---|
| generate-world (debug) | 0 | 15.05s | 1 chunk / 55 entities |
| world-quality-gate | **0** | — | status **pass**, failures **[]** |

关键审计：

| 项 | 值 |
|---|---|
| joint housing minimum | 18 |
| planned residential | 18 |
| realized residential with access | **18** |
| missing residential | **0** |
| service requests / realized with access | 4 / **4** |
| access connected / disconnected | **22 / 0** |
| landcover | status pass / unresolved **0** |
| land-use ledger | status pass / unresolved **0** |
| budget entityCoverageStatus | **realized** |
| development | status pass |

## 3. 意义

这是 B 的第一个同时满足以下条件的真实 512m 世界：

- 住宅最低数 18/18；
- 服务 4/4；
- 每个住宅有已验证入口；
- 用途账本与 landcover 闭合；
- 独立 world-quality-gate 通过。

它证明 B-01 的地块/建筑/入口链路和 B-03 的地表闭合可以在同一控制中同时成立。

## 4. 边界

- 不是 1km 沿路临街供给（那仍是 B-02 的 0/24 问题）。
- 不是最大图；最大图 B-05 另有 complete-ground gate pass。
- 没有形态 profile 的 coverageRatio 消费者；B-04 serviceRadiusM 另有
  village-field 控制。

## 5. 证据

- `bevy/work/rk-x/b01-joint-plains-complete-20261020/world/world-quality-gate.json`
- `.../scenes/scene-0/joint-layout-audit.json`
- `.../scenes/scene-0/service-program-audit.json`
- `.../scenes/scene-0/landcover-audit.json`
- `.../scenes/scene-0/land-use-ledger.json`
