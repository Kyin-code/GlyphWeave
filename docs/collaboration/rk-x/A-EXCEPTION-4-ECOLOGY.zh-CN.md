# A 里程碑例外 #4（生态配额缺 1 株）：已关闭

状态：**已关闭（2026-10-20）**。真实 6×10km 世界已达到
`planned 342,380 / realized 342,380 / unfilled 0`，且独立 gate 的失败清单不再包含生态配额项。

## 缺陷与修复

最大图曾出现 `planned 342,380 / realized 342,379 / unfilled 1`，拒绝记录为
`scene-0.landcover.4563.conservation.208.32.canopy.4`、层 `canopy`、第 5 株、原因
`no clear full-size ecology anchor in source patch`。

成因不是容量：锚点随机选取，早被接受的一个锚点可能把后面实例的位置关死，于是一块本可
放下全额的地报出 1 条拒绝，既不重试也不声明地不够。

修复（`fix(ecology)`，2026-10-18）：短少的 patch 在**确定性足迹晶格**上整体重排，只有实际
多放才采用；达标 patch 永不触碰，因此完整世界字节不变。唯一准入谓词 `anchor_is_clear` 由
**锚点扫描、重排、独立 verify** 三处共用；足迹宽于 patch 仍是具名拒绝而非静默短少。

## 关闭证据

| 项 | 值 |
|---|---|
| 世界 | `examples/maximum-6x10km.manifest.json` |
| manifest SHA-256 | `26647877A2F3D18D8D243F0463CBFA3024D93433D3D41C4DE2E391028AC189A9` |
| runtime SHA-256 | `03DD4D35BCC234B30529BC9423013AA67F5F81F4A35010D34B088E3C1A24E338` |
| 输出 | `bevy/work/rk-x/a-exception4-20261020/world/` |
| `generate-world` | exit 0；255.941 s；240 chunks；623 entities |
| 生态 | **planned 342,380 / realized 342,380 / unfilled 0** |
| quality-report | status `warn`；rules 623/623；access 156 connected / 0 disconnected；scale 240/240 chunks |
| world-quality-gate | exit 1；29.031 s；4 条失败，均不再是生态 |

完整证据与命令见
[`A-REBASELINE-20261020.zh-CN.md`](A-REBASELINE-20261020.zh-CN.md)。

## gate 仍然失败的四项

本次 A-closure 烘焙的 gate 失败项为：

1. `purpose-area allocation or realization remains incomplete`
2. `landcover remains unrealized`
3. `land-use planning or realization remains incomplete`
4. `road-first land-use development remains incomplete`

这四项属于 B-03/B-05 的用途与地表闭合，不再是 A 例外 #4。历史 r49 的
`50,230,050m²` 是旧源码快照；本次同口径重算为 **50,538,180m²**，两者不能混用。

## 仍未证明的事项

- 本次只做了一次 6×10km 烘焙；确定性重跑尚未做。
- 最大图独立 gate 仍 fail，A 不能因此被表述为 whole-world-quality-pass。
- 偶发 `0xC0000005` 仍未修复；本次单次烘焙成功不等于整套测试已稳定。
- 1km 探针的当前口径是 Round 53 的 **43 pad / 15 户 / 24 店 / 0 未验证入口 / 7 条 gate**，
  不是 Round 51 的 44/16/1/8。
