# 五项可见问题收口（2026-10-21）

本记录只处理用户对现有预览的五项反馈，不把 FUT 项混入完成口径。验收控制：

- Manifest：`examples/modern-lakeside-city-512.manifest.json`
- 世界：`bevy/work/rk-x/modern-lakeside-city-512-v6-20261021/world`
- `worldRevision`：`78010e9b0cca2e9736bfc2a8f415e676ff1be4d51f2ace85d5293ffd2b366e96`
- 战略图：`preview-strategic.png`
- 总览/近景：`preview-street.png`
- 城市 focus：`preview-city-focus.png`

## 1. 不规则白色区域

**结论：是城市地表基底在线性色彩空间中被重复做 gamma，亮部被抬成奶白色；land-cover
面片缝隙只是让它显形。**

排查顺序：

1. `?nograss` 后白色仍在，排除草叶过曝。
2. `?hide=access` 后白色仍在，排除入口铺装三角形。
3. `?hide=terrain` 后白色消失，确认来自地形网格。
4. 根因是 `makeMCTerrain` 把 sRGB 调色板字节直接写入 Three.js 的线性
   `BufferAttribute`；输出阶段再次编码后，约 0.5 的通道被抬到约 0.73。

修复：

- `adapters/html/render/terrain.js` 新增 `srgbChannelToLinear`，所有地形顶点色先转线性。
- 城市地表基底使用不参与强光叠加的稳定顶点色；自然草原仍保留 PBR 材质。
- `adapters/html/app.js` 提供 `?hide=terrain,access,landcover` 图层隔离开关，便于后续视觉回归。
- 最大世界实时烟测 `?nograss` 已不再出现白色/奶白空洞。

## 2. 草场只有疏密变化且呈圆形

**结论：已移除相机焦点径向拒绝采样。**

- `makeGrassBlades` 新增 `uniformDensity`；世界适配器开启后，草密度只由连续的世界
  coverage / wetness / slope 决定，不再围绕 camera focus 形成圆环。
- 城市场景改用较低 mown-lawn 密度和较矮草型；草原场景保留高密度参数。
- 草叶 shader 显式接入 `tonemapping_fragment` 与 `colorspace_fragment`，避免自定义
  shader 绕过 ACES 后把高光裁成纯白。

## 3. 现代社会感不足

**结论：现代控制场景已能同时读出混合街区、不同建筑尺度、街/店铺和高层。**

- 修正 HTML 分派：Rust 规划的 `storefront`、`residential_block`、`residential_tower`
  不再全部退化成无窗 `BoxGeometry`。
- 高层采用基座、分段窗带、屋顶设备、露台与街角树；店铺恢复橱窗/雨棚/招牌 CGA。
- 现代控制实际产物：29 条道路、27 个临街地块；住宅为 16x12/24x16，
  公寓块为 24x20x32m 与 36x24x20m，店铺为 12x10x6m。
- 当前基础道路仍以 64m 网格为主，D9.8 的真实转向/路口几何仍是后续工作；
  本轮先消除“只有几个固定盒子”的可见内容错误，不宣称道路生成器已完整现代化。

## 4. 中心土地利用、建筑固定尺寸和道路密度

- 临街请求从单一尺寸扩展为多尺度组合；实际建筑尺寸逐项来自 Rust 合同，HTML 不缩放。
- `compact-mixed-block` 形态、`automaticRoadExtensions` 的 128/64/32m 候选和
  collector/local 分级进入同一控制。
- 道路审计：`plannedEdges 29 / realizedEdges 29`，
  `plannedSites 27 / realizedSites 27`，`accessGraph.disconnectedEntities 0`。
- 建筑/道路/地块仍有明确上限：该 512m 控制只实现 27 个临街地块，不把未满足需求
  静默复制成装饰体；剩余供需仍应按 frontage outcome 审计。

## 5. 树、水体与 C 里程碑

**结论：C 不是“还没开始”。工程、水文、挡墙/涵洞和 Godot runtime 已有实现；
之前失败的是现代控制场景没有把树、水和城市内容组织到同一近景。**

- 当前控制：68 个显式 `tree` 实体，生态层总计 1,157 株（其中 tree 698、
  bush 449、reed 10），湖面 21,419m²，水面占位与岸线公共带均通过审计。
- 桥面/审计修复：
  - 结构桥不再按中心格重新贴地，避免岸侧中心格改写整个桥面标高。
  - 跨水审计从包围盒两个角改为逐格检查所有湿单元，避免干岸角误报和湿区漏报。
- 该控制 `road-network-audit.json: pass`、`scale-audit: pass`；
  `quality-report` 为 `warn`，唯一实质性剩余是 966m² 庭院坡地未实现面，
  另有 structured-water 场景尚未接 settlement planner 的已知口径提示。

## 6. 验证

```text
node --test adapters/html/render/terrain-color.test.mjs \
  adapters/html/render/road-profile.test.mjs
=> 4 passed

cargo test -p glyphweave-core --lib \
  structure_deck_keeps_plan_level_when_centre_station_is_on_dry_bank
=> 1 passed

glyphweave scale-audit <v6-world>
=> pass, failures []

glyphweave quality-report <v6-world>
=> exit 0, status warn
```

`pnpm lint`、`pnpm typecheck`、`pnpm build` 均通过；lint 保留仓库原有 warning。

## 7. 不夸大的边界

- 入口铺装仍是规则化三角带；颜色和位置不再像白色破洞，但还不是最终街景资产。
- 64m 基础路网、完整交叉口转弯几何、立交和 12 种聚落原型不属于本轮已关闭项。
- 动态车辆、行人和船只按项目合同应由运行时生成，当前 HTML 静态预览不伪造它们。
- 最大图的历史烘焙目录未整体重烘焙；其实时预览复制层用于验证白色伪影根因。
