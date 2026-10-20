# C-02 maximum-world engineering run (2026-10-20)

状态：**6x10 km 最大图的工程实体与工程后水文反馈已跑通，gate status=pass**。
这关闭了 C-G1 的“最大图工程版未跑”缺口；C-V1 的 Godot 运行与偶发崩溃仍开放。

## 1. 输入

| 项 | 值 |
|---|---|
| manifest | `examples/maximum-6x10km-engineering.manifest.json` |
| 基础 | `maximum-6x10km-complete.manifest.json` + `terrainWorks` + `engineeringDrainage="audit"` |
| 输出 | `bevy/work/rk-x/c02-maximum-engineering-20261020/world/` |
| worldRevision | `7ca6f8ce26084cf87677880ced97e1dfe3942e5689d63e3205cc31991b7feec5` |

## 2. 实测

| 项 | 值 |
|---|---:|
| generate-world | exit 0，**278.535 s** |
| chunks / entities | 240 / 623 |
| retaining walls | **1,734** |
| culverts | 0（该最大图没有 structured-water crossing；山城控制有 5 个） |
| works status | **pass** |
| engineered drainage status | **pass** |
| drainage nodes / edges | 58,844 / 57,122 |
| new sinks / lost sinks | 41 / 22 |
| changed flow nodes | **734** |
| raised terminals | **54** |
| world-quality-gate | exit 0，status **pass**，failures **0**，32.53 s |

挡墙由 carve 的自然高度与最终地形高差生成；不是手写实体。工程后水文图
对最终地形重新做 32m steepest-descent，保留 sink/outlet 分类，并逐节点比较
自然图。它是路由完整性反馈，不是水量/洪泛/泥沙模型。

## 3. 与 C-01 的关系

- 山城 512m：232 挡墙 + 5 涵洞，works/drainage pass；仍有 B frontage/landcover
  216m² 遗留，所以 gate 不 pass。
- 最大图：1,734 挡墙 + 0 涵洞，gate pass；证明工程 works 不会阻断最大图质量门。

## 4. C-L1 / C-V1

- `@cloudflare/workerd-windows-64`：已按 Apache-2.0 记录，见
  [`C-L1-WORKERD-20261020.zh-CN.md`](C-L1-WORKERD-20261020.zh-CN.md)。
- Quaternius 建筑资产：已定位到 **Ultimate Buildings Pack (Dec 2019)**，本地文件为
  改名/单材质派生；页面 CC0-1.0，当前 QAL v1.0 禁止单独再分发原包。精确历史
  下载日期仍未知，见 [`C-L1-QUATERNIUS-20261020.zh-CN.md`](C-L1-QUATERNIUS-20261020.zh-CN.md)。
- HTML preview：战略图与 2.5D 均实测；2.5D 在 chunks 加载期间显示 loading
  overlay，不再是无提示黑屏。最大图 2.5D 首屏约 40s（9 个 chunk），最终
  显示地形/建筑/植被，无 console error。
- Godot 4.7 未安装/未运行；按用户当前“渲染不做要求”，不把它当作 C 阻断，
  但发布前仍需一次最小场景验收。

## 5. 证据

- `bevy/work/rk-x/c02-maximum-engineering-20261020/bake.log`
- `bevy/work/rk-x/c02-maximum-engineering-20261020/world-quality-gate.log`
- `world/scenes/scene-0/terrain-works.json`
- `world/scenes/scene-0/engineered-drainage.json`
- `world/scenes/scene-0/engineered-terrain.json`
