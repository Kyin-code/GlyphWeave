# B Review R2 结果：512m 小闭环诊断

状态：`partial`。R2 目标 24/24/24 未完成；没有用提高 fill、改平自然地形或换 seed 绕过。

## 保留的 R1 修复

- F1 residual 来源绑定保留：CLI 从冻结 manifest/scene 独立取策略，scene 与发布 plan 同步篡改、删除、错 version、未知 scene 均拒绝。
- F2 priority/purpose/tier 原子选择保留；无授权 tier 不 fallback；宽路结点半径为 `max(junctionLength, roadWidth/2+3)`，8m 旧行为保持。
- F3 morphology 错类型拒绝和悬空 caseId 修复保留。

## station45 诊断

只读 `design-road-corridor` 使用同 seed 已成功产物的自然地形，不应用道路、不生成世界。失败走廊：

```text
start=[258,224], end=[258,160], width=16m
startLevelMm=8000, endLevelMm=8000
station=45: required 7120mm > allowed cap 7040mm
```

因此 station45 是真实自然/fill envelope 拒绝，不是共享结点、parcel、zone 或 purpose 错误。同一走廊在 7000/7100/7200/7400mm 的端口组合下可设计通过。

进一步有界搜索：

- 上半城市带 z=136/144/152 的横向 12m 路可设计，但其 26m parcel 行互相重叠，只能保留一行。
- 下半城市带 z=216/224/232/240 的横向 12m 路可设计，同样只能保留一行。
- 中间带 z=168..208、start x=256..264、end x=124..132、7000..7400mm 的有界搜索没有找到可行走廊，说明原自然高脊阻断中间带。
- 可行竖向走廊只在 x=256/264 等位置产生少量城内侧 parcel；没有找到能容纳 24 个 20×26 parcel 的路线组合。

完整表见 `bevy/work/collaboration/ds-b-r2/diagnostic-table-r2.json`。这不是全球数学不可行性证明，但足以说明当前 R1 候选网络与冻结工程合同冲突，不能继续用同一输入盲目生成。

## 运行与验证

- R2 新 512m 生成：0/2，未运行；上一步诊断先证明未找到 24 个完整 parcel 的合法候选网络。
- 因没有新生成，未运行 gate、maximum-world、release 或浏览器。
- core：405 passed / 3 ignored / 0 failed。
- residual CLI 来源负例：1 passed。
- R1 的 A frontage 与 automatic-road 定向回归继续通过。

## 未解决

- 16 住宅 + 8 店铺真实 CLI 24/24/24 未证明。
- 24 完整 parcel 与 yard surface 闭合未证明。
- collector+local 两级实际拓扑未在生成世界形成。
- 在保持自然高地、fill、水体和保护约束的前提下，需要新的**合法道路选线/地块分配**方案；不能靠增加填方或删需求通过。

本文件与 `ds-b-r2` 证据不替代 R1 原始失败证据，也不签收 B 或推进 C。
