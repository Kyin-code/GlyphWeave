# 48 城自然地理与迭代历史完整性审计

日期：2026-10-21  
审计脚本：`scripts/audit-urban-case-completeness.mjs`

## 目的

把“48 个样本是否都具备自然地理、三阶段历史、城市结构、土地压力和证据入口”
从人工约定变成可重复运行的机器审计，并明确哪些字段是来源审计、哪些仍只是
机制先验。

运行：

```powershell
pnpm check:urban-cases
```

## 审计范围

每个 case 必须同时满足：

- `coreCut`、`focusFeature`、`communityProfile` 非空；
- `naturalGeography` 含 setting、regionalReliefM、climateBand、
  geomorphicProcesses、hydrography、hazards、evidenceTier；
- history 至少三阶段，order 严格连续，era/change 非空；
- urbanStructure 含 oldCore、renewal、newCity、edge 和至少两个 centres；
- landPressurePrior 有合法 settlementScale；
- specialFactors 非空；
- evidence coverage 至少有 2 个已登记来源，sourceId 可解析；
- measurementFile 实际存在；
- history-pattern 审计存在且至少三阶段。

## 当前结果

```json
{
  "status": "pass",
  "cases": 48,
  "sourceAuditedCases": 48,
  "measurementWindows": 48,
  "historyAudits": 48
}
```

规模分布：

| settlementScale | 数量 |
|---|---:|
| city | 18 |
| megacity | 19 |
| metropolis | 10 |
| town | 1 |

高发自然过程：

| process | case count |
|---|---:|
| fluvial deposition | 20 |
| fluvial incision | 16 |
| mass wasting | 16 |
| coastal wave erosion | 14 |
| tectonic uplift | 13 |
| tidal processes | 13 |
| tectonic subsidence | 12 |
| coastal deposition | 9 |
| reclamation | 8 |

高发 hazard：

| hazard | case count |
|---|---:|
| flood | 36 |
| storm surge | 24 |
| landslide | 21 |
| earthquake | 15 |
| typhoon | 12 |
| drought | 10 |

## 重要限制

`heuristicGeographyCases = 48` 是当前审计的明确结果：48 个 case 都已有完整的
自然地理字段，但其 `evidenceTier` 仍是 `mechanism-prior`，不是逐案的
`source-verified`。

这意味着：

- 可以进入生成器作为可审计的区域先验；
- 不能声称每座城市的造山、河流改道、喀斯特、风蚀、冰蚀等历史均已逐案完成
  史料/地质来源核对；
- 下一步应优先为地形机制差异最大的代表案例补足独立地理来源：
  重庆、昆明、西安、香港、东京湾、纽约、威尼斯/低地城市等；
- 来源核对完成后，逐步把 `evidenceTier` 从 `mechanism-prior` 升级为
  `source-verified`，不能让 Agent 直接改标记。

## 结论

规模、三阶段历史、旧城/更新/新城/边缘结构、土地压力和测量窗口已完整覆盖
48/48；自然地理字段也覆盖 48/48，但仍是机制先验层。该状态支持生成器的
“切面先验”使用，不支持“48 城历史地理研究已全部结案”的表述。

