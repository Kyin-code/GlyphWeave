# A-rebaseline 2026-10-20：6×10km 最大图实测

状态：**A 例外 #4 已关闭；A 整体仍未签收为 whole-world-quality-pass。**

## 1. 目的

用当前 `refactor/authoritative-baked-audit` 源码重新烘焙 6×10km 最大图，关闭
A 例外 #4（生态缺 1 株），并为后续 B-03/B-05 提供同源码快照的最大图基线。

## 2. 源快照

| 项 | 值 |
|---|---|
| HEAD | `74cdc1ad533c2200b72339c02110f46f10771ab7` |
| manifest | `examples/maximum-6x10km.manifest.json` |
| manifest SHA-256 | `26647877A2F3D18D8D243F0463CBFA3024D93433D3D41C4DE2E391028AC189A9` |
| release binary | `bevy/target/release/glyphweave.exe` |
| binary SHA-256 | `03DD4D35BCC234B30529BC9423013AA67F5F81F4A35010D34B088E3C1A24E338` |
| 输出 | `bevy/work/rk-x/a-exception4-20261020/world/` |

## 3. 命令与结果

```powershell
cargo build --manifest-path bevy/Cargo.toml -p glyphweave-cli --release
bevy/target/release/glyphweave.exe generate-world examples/maximum-6x10km.manifest.json bevy/work/rk-x/a-exception4-20261020/world
bevy/target/release/glyphweave.exe quality-report bevy/work/rk-x/a-exception4-20261020/world
bevy/target/release/glyphweave.exe world-quality-gate bevy/work/rk-x/a-exception4-20261020/world
```

| 命令 | exit | 耗时 | 关键结果 |
|---|---:|---:|---|
| `generate-world` | 0 | 255.941 s | 240 chunks / 623 entities / worldRevision `115234ca…` |
| `quality-report` | 0 | 26.988 s | status `warn`；rules 623/623；access 156/0；scale 240/240 |
| `world-quality-gate` | 1 | 29.031 s | status `fail`；4 条 land-use 失败 |

## 4. 生态关闭

`landcover-audit.json`：

```json
{
  "ecologyPlannedPlants": 342380,
  "ecologyRealizedPlants": 342380,
  "ecologyUnfilledPlants": 0
}
```

这是 A 例外 #4 的直接关闭证据。`world-quality-gate.json` 的 failures 中已不再出现
`ecology density budget has 1 unfilled plants`。

## 5. 仍然失败的内容

`world-quality-gate.json` 本次 failures：

1. `"scene-0" purpose-area allocation or realization remains incomplete`
2. `"scene-0" landcover remains unrealized`
3. `"scene-0" land-use planning or realization remains incomplete`
4. `"scene-0" road-first land-use development remains incomplete`

同口径 ledger：

| 指标 | 值 |
|---|---:|
| scene area | 60,000,000 m² |
| realized | 9,461,820 m² |
| unresolved | 50,538,180 m² |
| nature realized | 9,030,000 / 9,030,000 m² |
| rural realized | 431,820 / 47,997,120 m² |
| urban realized | 0 / 2,400,720 m² |
| unassigned realized | 0 / 572,160 m² |

历史 r49 的 `50,230,050m²` 是不同源码快照，不能与本表混用。

## 6. 未验证项

- 未做第二次 6×10km 确定性重跑。
- 未做 240 chunk 浏览器连续漫游。
- 偶发 `0xC0000005` 未修复；本次成功只代表这一次烘焙成功。
- 本次没有把 gate fail 说成 A 通过；A 只关闭了生态例外 #4。

## 7. 证据位置

- 烘焙日志：`bevy/work/rk-x/a-exception4-20261020/bake.log`
- quality-report 日志：`bevy/work/rk-x/a-exception4-20261020/quality-report.log`
- gate 日志：`bevy/work/rk-x/a-exception4-20261020/world-quality-gate.log`
- 生态审计：`bevy/work/rk-x/a-exception4-20261020/world/scenes/scene-0/landcover-audit.json`
- 用途账本：`bevy/work/rk-x/a-exception4-20261020/world/scenes/scene-0/land-use-ledger.json`
- gate：`bevy/work/rk-x/a-exception4-20261020/world/world-quality-gate.json`
