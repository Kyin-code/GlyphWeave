# B-03 complete-ground closure (2026-10-20)

状态：`land-use/landcover 闭合已在 512m 控制世界实现`；B 整体仍未签收。

## 1. 能力

新增 opt-in：

```json
"residualLandProgram": {
  "version": 1,
  "scenes": {
    "scene-0": {
      "coverage": "complete-ground-v1"
    }
  }
}
```

旧世界不带该字段时逐字节保持原样。实现见
[`docs/design/complete-ground.md`](../../design/complete-ground.md)。

## 2. 控制世界

| 项 | 值 |
|---|---|
| manifest | `examples/b03-complete-ground-512.manifest.json` |
| 输出 | `bevy/work/rk-x/b03-complete-20261020b/world/` |
| generate | exit 0，约 3.7s |
| gate | exit 1；失败 2 条（frontage、road audit） |
| landcover | `status=pass`；planned 229,568 / realized 180,416 + conservation 49,152 / unresolved **0** |
| ground cover | **128,542 m²** 新真实网格 |
| residual | `candidate 229,568 / realized 229,568 / unresolved 0 / status pass` |
| ledger | `status=pass`；`realized+unresolved=262,144` |
| budget | `entityCoverageStatus=realized`；`assigned=262,144`；`unassigned=0` |
| development | `status=pass`；`incompleteReasons=[]` |

对照旧控制（无 coverage）：`realized 133,602 / unresolved 128,542`，
residual backlog 恰好 128,542 m²。新控制把这 128,542 m² 变成
`rural_meadow / rural_woodland_floor / rural_rough_ground /
urban_vacant_ground` 等真实地形网格，而不是改状态或重标用途。

## 3. 独立审计

- `landcover::verify` 从最终地形重建整份 surface，逐网格比对；
- 每个 baseline quad 都在占用位图中避开已有 mesh，防双重计量；
- `residual_audit` 从已发布 patch 几何复算 realized，`unresolved=0`；
- `landUseBudget.actualNatureM2` 改为从 surface 实测，修复了之前硬编码 0
  导致 nature 49,152m² 永远算作 underfilled 的缺陷；
- 新增 CLI 回归 `bevy/crates/cli/tests/land_use_closure.rs`。

## 4. 仍未完成

1. 512m 控制仍因 `frontage demand 0/24` 和
   `road generated.road-auto.scene-0.0 ... fill 6.4m` 失败；前者属于 B-01/B-02
   供给，后者属于 B-02 final audit。不能据此签 B。
2. `world-quality-gate` 仍读取已发布的 `landcover-audit.json`，没有在 CLI 中
   重新执行 `landcover::verify`；core 测试能抓住伪造 surface，CLI gate 不能。
   已记录到 [`ISSUE-cli-landcover-gate-trust.zh-CN.md`](ISSUE-cli-landcover-gate-trust.zh-CN.md)。
3. 最大图尚未用 complete-ground 重烘焙；这是 B-05/A-closure 的下一步。
