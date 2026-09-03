# GlyphWeave HTML 渲染：全部方案、尝试记录与当前状态（移交文档）

- **整理**：小卡（opencode 助手）2026-09-03
- **目的**：把 GlyphWeave 2.5D 预览的全部渲染探索、验证过的开源方案、当前实现与调参记录，完整移交给下一位开发者（Agent 或人）
- **核心诉求（Kyin）**：让程序化世界的渲染达到陈星汉《花》/《Journey》级别，至少 Minecraft 质感；风格化、艺术化，而非写实

---

## 1. 项目背景与渲染目标

GlyphWeave 是一个程序化世界生成器（Rust 核心生成 `.gemap`/manifest + 多种适配器）。**HTML 预览适配器**（`adapters/html/`）用 Three.js 渲染 2.5D 世界，当前用于视觉验收（战略图 + 3D 近景）。

视觉目标演进：
1. 早期：能看（战略 2D canvas 俯瞰 + Three.js 近景）
2. 中期：**风格化**（对标陈星汉）——近景有草/树/楼/水的艺术化质感
3. 现在：**"地面=草色、草叶只做动态、季节调色板"**（Kyin 确立的原则）

### 两个视角模式（关键）
- **2.5D 近景 / skyline（正交俯瞰）**：看整体构图、地形色彩、城市布局。**但 2000m 世界的草在此视角下不可分辨**（天然限制，非 bug）。
- **第一人称 explore（透视）**：看草叶片/树/岩石的细节质感——**这才是展示草效果的正确视角**。MiMo（视觉评审模型）明确：草效果要在 explore/近景看，skyline 判断"草像贴图"是视角误判。

---

## 2. 渲染探索完整时间线

### 阶段 A：早期"仿陈星汉花"方向（深圳湾 → 草原）

| 时间 | 尝试 | 结果 | 教训 |
|------|------|------|------|
| 深圳湾城市场景 | 城市地表纹理 `makeSurfaceTexture`（CPU canvas 生成色块纹理 + 法线图） | 城市能看 | 纹理与几何脱离，精细物（树/路灯）与建筑冲突 |
| 城市 CGA 规则建筑（buildings.js） | 22 类建筑 + CGA 规则引擎 | 巨型建筑可 | 精细层面无效（门挡树、路穿房） |
| 视觉评审闭环 | MiMo 视觉模型评审截图 | 指导后续 | 评审需多视角，俯视会误判草 |
| 内蒙草原（纯自然） | `makeMCTerrain` 顶点色 + `MeshLambertMaterial` | 色彩可，**无质感** | Lambert 把绿草洗成灰黄 |
| 草 billboard（makeSteppeGrass v1） | InstancedMesh + 简单正弦风三角片 | 风动有 | 远处是"点"，无叶片细节 |

**关键教训（贯穿始终）**：
- **Lambert 顶点色不行** → 用 MeshStandardMaterial（色彩管理正确）
- **CPU canvas 纹理慢且糙** → 能 GPU 就 GPU
- **2.5D 俯视 = 草不可见** → 草评价必须 explore 视角
- **"调参"要针对参考工程**，不能凭空猜参数

### 阶段 B：调研 & 克隆开源方案验证（2026-08-31）

Kyin 指示克隆高质量开源渲染项目做对照评分。三个项目放 `G:\difyllmwiki\render-experiments\`（各自独立 Vite 工程，依赖已装）：

| 项目 | 来源 | 技术 | 评分（MiMo） | 结论 |
|------|------|------|------|------|
| `01-three-stylized` | github.com/Steve245270533 | 原生 Three.js GLSL | 质感6.25/10 | 确定性草地，可直接移植 |
| `02-grass-system` | github.com/achrefelouafi **GrassSystemThreeJS**（soil-system） | 原生 Three.js + 后处理 | 质感8/10 | **Kyin 选定为主方案**（土壤/草 PBR 管线 + DoF/Bloom/胶片） |
| `03-stylized-scene` | github.com/dedekpo | R3F + WebGPU + TSL | 6.25/10 | 吉卜力色，需 WebGPU（远期） |

**02 工程是核心参考**（Kyin 亲自在 `http://localhost:5174` 调参）。它的关键设计：
- 草：GPU instanced 叶片 + 圆弧 curl + 相干风 + backlight 透射 + coverage mask + 共享 groundHeightAt 粘地形
- 地面：`onBeforeCompile` 注入 PBR（置换/法线/裂缝/苔藓）
- 后处理：DoF + Bloom + 胶片调色（色差/暗角/颗粒），MSAA 在 composer RT
- 光照：ACES tone mapping + key/fill/rim 三灯 + RoomEnvironment

### 阶段 C：移植落地（2026-08-31 ~ 09-02）

把 02 的思路移植进 `GlyphWeave/adapters/html/`。**已提交 commits**（GitHub main）：
- `e95d3c4`/`6f5283d`/`ad9cf08`/`9fef1c7`/`e96fc58`：地形骨架/水文/biome/模块化
- `1796c5a`：wgen 草原 + naturalOnly + MC 顶点色渲染
- `4785d76`：**风格化管线一期**（草叶片 + PBR 地面 + 后处理 + 本地 three）
- `03d276e`/`93cdf8e`：草绿地面 + 光照调优 + 三档草 + 季节调色板
- 后续：规则引擎系列（另见第 7 节）

---

## 3. 当前渲染架构（文件级）

入口 `adapters/html/index.html` → `app.js`（1334 行，前置编排）。

### 模块化渲染层 `adapters/html/render/`

| 文件 | 行数 | 职责 | 关键点 |
|------|------|------|--------|
| `terrain.js` | 500 | 地面几何/材质 | `makeMCTerrain`（CPU 烘焙草绿顶点色 + tonal/moss/warm）、`makeSurfaceTexture`（城市）、`makeArtisticGroundTexture`、`makeSteppeGroundMaterial`（曾用 onBeforeCompile，因 vColor 丢失弃用） |
| `grass.js` | 305 | **GPU instanced 草** | 三档参数（见 §5），`InstancedBufferGeometry` + 圆弧 curl + 相干风 + backlight + 相机焦点密度衰减 |
| `vegetation.js` | 276 | 树/岩石/灌丛 | `buildTreeInstances`（风动树冠 GLSL）、`buildRockInstances`（instanced 低多边形） |
| `water.js` | 142 | 水面 | `makeWaterMaterial`（Fresnel 透明度 + 顶点涟漪）|
| `sky.js` | 17 | 天空 | 黄昏暖色渐变 CanvasTexture |
| `postfx.js` | 124 | 后处理 | EffectComposer：Render → Bokeh(DoF) → Bloom → OutputPass → FilmGrade（色差/暗角/颗粒） |
| `buildings.js` | 998 | CGA 建筑 | 22 类 + CGA 规则引擎（深圳湾/城市用） |
| `props.js` | 222 | 路灯/水井/摊位等 | 城市细节 |
| `presets/materials.js` | 70 | **材质参数唯一来源** | Kyin 调参回填处（草三档 + 地面季节调色板） |

### 支撑 `adapters/html/core/`
`shared.js`（噪声/颜色/random）、`world.js`（scene/height 读取）、`state.js`、`camera.js`、`three.js`。

### 关键：本地 three（vendor）
`adapters/html/vendor/three/` 存放 three@0.178 全套（含 **three.core.js**——必须存在，否则模块加载失败）。`index.html` 的 importmap 指向本地 vendor，**不依赖 CDN**（无代理可运行）。这是踩过的坑：three.module.js 内部 `from './three.core.js'`，只下 module 会 404。

---

## 4. 视觉评审方法（重要工作流）

- **工具**：`render-experiments/review-shot.mjs`（调 MiMo API）+ `shot-preview.mjs`（headless Chrome 截图 + CDP 诊断）
- 截图文件在 `render-experiments/shots/`（77 张）
- **关键**：评审用 1280 宽 PNG（太大传不上，太小抹细节），**explore 近景**截图才能评草
- MiMo API：`https://token-plan-cn.xiaomimimo.com/v1`，model `mimo-v2.5`（key 在脚本里）

### MiMo 评分轨迹
| 版本 | 阶段 | 质感 | 氛围/色彩 | 植被/密度 |
|------|------|------|-----------|-----------|
| 早期草原 | Lambert+顶点色 | 4/10 | — | 3/10 |
| 一期移植 | 草+后处理 | 4-6/10（俯视误判） | 8/10(光照) | 8.5/10 |
| 三档草 160k | focus 密度 | 8/10(质感) | 8.5/10(色彩) | **9/10(植被)** |
| 500k-1.2M | 近景密度修正 | 密度 8、立体感 9 | — | 层次 7 |

**结论**：画面已到"陈星汉氛围 + 大量植被"的水准，MiMo 认可 8/10 级。剩余差距是细节精致度（低多边形风 vs Journey 手工感）。

---

## 5. Kyin 调参记录（最宝贵资产）

Kyin 在 02 工程（Soil Studio :5174）亲自调出草参数，原则"**地面染成草色，立体草叶只做动态，颜色看季节**"。

### 草三档（grass.js 内联 + presets/materials.js 同步）
| 档 | bladeHeight | bladeWidth | curl | 用途 |
|----|------------|-----------|------|------|
| 浅草 low | 0.2 | 0.02 | 0.25 | 洼地铺底 |
| 中草 mid | 0.78 | 0.042 | 0.48 | 草原主体 |
| 高草 high | 1.54 | 0.05 | 0.73 | 高处/点缀 |

分层逻辑：CPU 按地形高度分区（elev + jitter <0.3 low / <0.78 mid / else high）。

### 草渲染关键参数（grassLook）
- `colorBase #33421b`（底部暗绿）/ `colorTip #9bc24a`（尖端亮绿）
- `colorVarAmt 0.5`（每丛色差）、`translucency 1.1`（逆光透光）
- 密度：explore 900k / skyline 1.2M 根，相机焦点 55m 核心超高密，外缘平方衰减
- **数量启示**：200m 内想茂密需要 >50 万根；02 是 20m 放 2.6 万根 = 每 0.014m² 一根，我们早期每 1.6m² 一根必然稀疏

### 地面季节调色板（groundPresets/seasonPalettes）
```
summer(当前): low[66,118,52] mid[88,132,60] high[132,148,70]
spring/autumn/winter 已备好，改 season 字段即可
```
地面 tonal 烘焙进 makeMCTerrain 顶点色（CPU），**不要**用 onBeforeCompile 的 color_fragment 覆盖（会丢 vColor）。

### 光照与雾（app.js steppe 分支）
- 自然日光：白太阳 1.3-1.8 + 绿半球光 + 白 ambient 0.28-0.5
- 雾 `new THREE.Fog(0xa8c48a /*草绿*/, 3500, 8000)`——**雾色必须是草绿不是暖黄**，否则远景绿地变黄褐
- 太阳位置(-300,600,200)、ACES tonemapping

---

## 6. 运行方式（给下一位开发）

```bash
# 预览服务（静态 serve G:\difyllmwiki 根，已修复目录/MIME 问题）
cd G:\difyllmwiki && node serve.mjs        # :8083

# 草原近景（看草/树/岩石/黄昏天空）
http://localhost:8083/artifacts/grassland/preview/?view=explore
# 2.5D 全景（看构图/色彩；草在此视角不可分辨）
http://localhost:8083/artifacts/grassland/preview/?view=skyline

# URL 参数：?view=explore|skyline|street|near；?nograss 隐藏草；?nofx 关后处理
```

**重要**：`artifacts/grassland/preview/` 是 html 应用副本（每改 `adapters/html/` 需同步：
`Copy-Item adapters/html/* artifacts/grassland/preview/ -Recurse -Force`）。新生成的世界场景同理。

---

## 7. 我对当前渲染的观点（供下一位参考）

### 已达成
1. **风格化定位清晰**：低多边形 + 艺术化色 + 黄昏光，远离写实；这条方向 Kyin 认可。
2. **草达到有生命**：GPU instanced 三档、风动、逆光、密度衰减正确。explore 近景 MiMo 认可（密度 8 / 立体 9）。
3. **地面=草色原则落地**：makeMCTerrain 顶点色 + 季节调色板，参数在 presets 一处可改。
4. **后处理管线就绪**：DoF/Bloom/胶片，帧率可接受。
5. **本地化 three**：无 CDN 可跑，CI/离线友好。

### 尚未解决（明确知道的问题）
1. **"森林/远景植被密度" vs 性能**：近景 900k 草是焦点区衰减，skyline 看整体时植被稀疏。方案：LOD 草（近景精细叶 + 远景 billboard/色块）未做。
2. **树叶是低多边形几何**（dodecahedron），不是 Journey 那种单片透光叶。要做 alpha 面片叶需换树模型。
3. **相机运动时草与树的 shader 风相不共享**（grass 用自带 wind，tree 用 window.__treeWind），风方向要统一。
4. **DoF 默认关闭**（bokeh.enabled=false），开启需调 focus/aperture 到大场景值。
5. **规则引擎与渲染未联动**：Rust 生成逻辑（规则引擎）已验证，但 HTML 渲染端（buildings/props）仍是旧直接放置——规则产出的实体几何要在渲染端重新对齐。

### 我认为下一步最值得做的（优先级排序）
1. **统一风系统**（草/树/灌木共享世界空间风场）→ 视觉一致性收益最大
2. **树冠升级**：多层 alpha 叶片（带透光），替代低多边形 dodecahedron（对标 03 的叶子做法，但用 GLSL 而非 WebGPU）
3. **LOD 植被**：explore 精细 + 远处 billboard/雾化，解决"大场景 vs 性能"
4. **季节切换实装**：presets 已有调色板，加 URL ?season=autumn 驱动地面/草/树变色
5. **体积雾/光晕**（可选远期）：已有 FilmGrade/Bloom，加 God ray 需额外 pass

### 给下一位的警告（踩过的坑）
- **不要用 MeshLambertMaterial 做草绿地面**（洗色）
- **不要用 onBeforeCompile 覆盖 #include <color_fragment>** 丢 vColor；地面变化烘焙进 CPU 顶点色
- **雾色=草绿**，别用暖黄
- **three 本地化必须含 three.core.js**
- **评审草必须 explore 视角**，skyline 俯视草天然不可见
- **改 adapters/html 后同步 preview 副本**

---

## 8. 相关但独立的系统

### 规则引擎（Rust，已在 GlyphWeave bevy）
- 现状：完成 MVP（审计 + rulesMode 主导生成），见 `docs/rules-engine-final-notes.zh-CN.md`
- 与渲染关系：规则引擎约束实体**是否放/放哪**，渲染引擎负责**长什么样**。两者通过 manifest/scene.json 的实体（kind/worldX/worldY/width/depth）衔接。
- **下一位注意**：渲染端 `app.js` 的实体 builder 按 kind 分支渲染，新 descriptor（TOML 规则）若新增 kind，渲染端要加对应 builder 或 fallback。

### 生成器（Rust worldgen.rs，~4800 行）
地形/水文/biome/道路/地块/实体生成。与渲染用 `.gemap` 或 manifest+bake 产物衔接。

---

## 9. 附：参考资源

| 资源 | 用途 | 位置 |
|------|------|------|
| Soil Studio（主参考） | 草/地面/后处理参数来源 | `render-experiments/02-grass-system/`（:5174） |
| three-stylized | 确定性草地/光照 | `render-experiments/01-three-stylized/` |
| stylized-scene | WebGPU TSL 吉卜力风（远期） | `render-experiments/03-stylized-scene/` |
| MiMo 评审脚本 | 视觉自评 | `render-experiments/review-shot.mjs` |
| 无头截图脚本 | 自动截图+诊断 | `render-experiments/shot-preview.mjs` |
| 材质参数 | Kyin 调参唯一来源 | `adapters/html/render/presets/materials.js` |
