# 南京长江大桥 scale-audit 审计（修复复盘）

本文记录南京长江大桥场景从“实体落水/竹竿树/分块棋盘格”到可通过
`scale-audit` 的修复过程与验收标准。后续 Agent 调度南京桥或类似大场景时
必须先读本文与 GlyphWeave skill 的「已排查硬性规则」章节。

## 1. 关键数据

| 项目 | 值 | 来源 |
|------|-----|------|
| 场景尺寸 | `5000m × 2400m` | 公路桥 4588m 横跨主轴约 91.8% |
| 公路桥长 | `4588m` | `examples/nanjing-yangtze-bridge.manifest.json` |
| 长江河道 | `1400m × 2400m`，水平水面 | `landmark.yangtze-river` |
| 树冠直径 | `2.6 ~ 4.8m` | 阔叶树真实冠幅，非树干直径 |
| 树高 | `5 ~ 8m` | 同上 |
| 单店/建筑 | `12 × 12 × 7m` | 店铺尺度 |
| 实体总数 | `11949`（修复前 13007） | 落水实体被剔除 |

## 2. 已修复的问题与根因

### 2.1 树/房落水
- **根因**：worldgen 硬编码河道半宽为 `0.16 * scene.width = 800m`，而
  Manifest 的 river landmark 是 `1400m` 宽。`800~1400m` 区间实体被判
  “岸上”，渲染时却泡在水里。另：`jitter` 之后未复检水体；细节实体
  （bench/lamp/grass_clump/fallen_log）在水体模式下无条件放行。
- **修复**：`river_half_width_m()` 从 river landmark 提取半宽，worldgen
  地形/材质/实体共用同一值；`jitter` 后复检 `in_river`；非 reed 细节
  实体显式排除水面格。
- **校验**：`worldgen::tests::river_landmark_keeps_entities_out_of_water`
  + `scale-audit` 水体 footprint 相交检查（`river`：`|dx| < halfW - margin`；
  `lake`：椭圆归一化半径）。

### 2.2 树木像竹竿
- **根因**：tree 的 `widthM` 被写成树干直径 `0.2~1m`，渲染按整棵树
  （树冠+树干）缩放到该宽度，得到细高棍。
- **修复**：`widthM/depthM` 改为树冠直径 `2.6~4.8m`；`scale-audit`
  默认范围改为冠幅 `2.0~5.5m`、高度 `4~9m`；Manifest `assetContracts.tree`
  同步为冠幅范围。

### 2.3 分块主题色不一致（棋盘格）
- **根因**：LOD/远区几何每 Chunk 取“中心格材质”作为整块单色，相邻 Chunk
  中心材质不同 → 大色块棋盘格。
- **修复**：新增 `makeLodTexture()`，每 8m 对 `surface` 取均值生成连续
  `CanvasTexture`，Chunk 边界颜色自然过渡；战略图按真实 surface 数据
  逐 64m 块绘制，并叠加 `road`/`bridge` footprint。

### 2.4 地面质感差、无主体
- 草地增加草叶笔触（高频 blade）+ 草簇（tuft）+ 低频明暗草斑（patch）+
  草丛团（clump）；土路/沙岸加石子斑点；水体保留波纹与高光。
- 新增两岸滨江公路实体（`generated.riverbank-road-*`，南北向 14m 宽，
  带标线与路缘）；建筑挂店铺招牌面。
- 废弃“SVG/缩略图替代真实渲染”验收，验收必须用运行态截图。

## 3. 验收命令

```bash
cargo test --manifest-path bevy/Cargo.toml -p glyphweave-core -p glyphweave-cli
cargo run --manifest-path bevy/Cargo.toml -p glyphweave-cli -- \
  generate-world examples/nanjing-yangtze-bridge.manifest.json <OUTPUT>
cargo run --manifest-path bevy/Cargo.toml -p glyphweave-cli -- scale-audit <OUTPUT>
cargo run --manifest-path bevy/Cargo.toml -p glyphweave-cli -- quality-report <OUTPUT>
cargo run --manifest-path bevy/Cargo.toml -p glyphweave-cli -- preview <OUTPUT> 18144
```

## 4. 最新验收结果

- `scale-audit`：**pass**（11949 实体，0 失败）
- `quality-report`：**pass**
- Rust 测试：`125 passed`（core）+ `4 passed`（cli）
- 运行态截图确认：
  - 战略图色块连续、无棋盘格；大桥横贯全图；两岸各一条南北向道路
  - 近景水面无树/房/家具；岸上树冠正常、带招牌店铺存在
  - 草地有颗粒质感；滨江道路带黄线

## 5. 后续硬性要求

见 GlyphWeave skill「已排查硬性规则（南京长江大桥复盘）」：
水体 footprint 一致性、尺寸契约同步、分块配色连续、禁止缩略图验收。

## 6. 街景复刻（第二轮修复）

针对“桥两侧应有什么建筑”的质疑，先搜索真实资料再布设：

| 地标 | 位置 | 规格 | 依据 |
|------|------|------|------|
| 狮子山阅江楼 | 南岸 x=860, z=760 | 山 78.4m + 楼 52m，7 层 | 百度百科/维基（红柱金瓦） |
| 南堡公园 | 南桥头堡下 x=250, z=1230 | 320×260m，对称园林+喷泉 | 攻略（宝塔桥东街 7 号） |
| 北堡公园 | 北桥头堡下 x=4740, z=1230 | 300×240m | 百科（2005 年新建 7 公顷） |
| 浦口火车站 | 北岸 x=4380, z=820 | 站房+钟塔+雨廊+铁轨 | 攻略（百年老站《背影》） |
| 弘阳之星摩天轮 | 北岸 x=4080, z=620 | 直径 46m 高 116m | 中国日报（桥北地标） |
| 长江货轮 ×2 | 江面 z=760 / 1720 | 70~90m 长 | 通航资料 |
| 街区塔楼 ×250 | 两岸 300~1700 / 3300~4700 | 20×18m，高 24~50m | 下关/桥北街景 |
| 桥上汽车 ×7 | 桥面 z=1200 | 4.6×1.9×1.4m | 参照物（玉兰路灯间） |

本轮同时修复：
- **道路被地皮覆盖**：道路 strip 抬升 0.45m + 深色路床，标线/路缘随路面抬升。
- **马赛克**：近景纹理 `textureScale 2→3`（≥1 texel/0.33m），LOD 纹理
  `step 8→4`（4m/texel）；低空截图确认草地有颗粒、桥面标线清晰。
- **战略图实体消失**：抽 `drawStrategicOverlays()`，在每次 chunk 异步绘制
  后重画 road/building/地标符号（否则被地形覆盖）。
- **岸上水潭**：低洼水坑仅限近岸湿地带（河宽外 60~320m），其余显示泥地。

验收结果：`scale-audit: pass`（12213 实体）、`quality-report: pass`、
`cargo test 125+4 passed`；低空近景可见桥上汽车与标线，两岸可见塔楼群、
站房、摩天轮，江面可见货轮。
