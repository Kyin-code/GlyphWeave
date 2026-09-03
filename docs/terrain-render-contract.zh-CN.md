# 地貌与地表渲染契约（Terrain Render Contract）

- 状态：第一、二阶段已实现基础链路
- 更新：2026-09-03
- 适用：GlyphWeave 通用地图生成器、HTML 预览、后续 Godot/Bevy 适配器

## 1. 目标

本契约把“地形几何真相”从视觉参数中分离出来。生成器、规则、道路、建筑、植被和渲染器必须围绕同一份 baked 地表工作；渲染器可以增加颜色、法线、草叶和雾，但不得自行制造会影响接地关系的第二套几何高度。

```text
Rust worldgen
  -> authoritative heightfield + land cover + terrain semantics
  -> scene.json chunk descriptor
  -> HTML / Godot / Bevy adapter
  -> material, vegetation and LOD
```

## 2. 唯一几何真相

### 2.1 高程

每个 chunk 的 `*.height.bin` 是唯一权威地形几何：

- 格式：little-endian `i16`；
- 精度：0.25m；
- 栅格：1m；
- 道路、建筑、规则审计和 adapter 均以其为最终接地依据。

普通生成式地块只能填高低侧，不能为了建筑中心高度或水面 datum 挖低自然高侧地面。台地、切坡、挡土墙等未来工程必须以显式 terrain edit 表达。

### 2.2 土地覆盖

每个 chunk 的 `*.surface.bin` 是每格 `u8` 覆盖分类。它表达水体、岸线、草地、湿泥、裸土、岩地等基础 land cover，不应由 HTML 端的随机颜色替代。

## 3. Terrain Semantics Sidecar v1

每个新 baked chunk 额外输出：

```text
chunk-X-Z.terrain.bin
```

文件与 height/surface 同尺寸、同世界坐标、同 1m 栅格。每格 4 个字节：

| 字节 | 名称 | 含义 |
| --- | --- | --- |
| 0 | `slope_degrees_u8` | baked 高程中心差分得到的 0–90° 坡度，映射到 0–255 |
| 1 | `curvature_signed_u8` | 局部曲率，128 约等于平地；小于 128 为脊，大于 128 为谷 |
| 2 | `wetness_u8` | 气候/临水湿润度，0–255 |
| 3 | `disturbance_u8` | 人工扰动：0 自然、160 建筑或地块填方、255 路床 |

`sidecar.json.contract.terrainSemantics` 声明该格式，`scene.json` 的每个 chunk 用 `terrainFile` 指向 payload。CLI `validate-world` 与 `scale-audit` 会校验文件存在、字节长度和 chunk hash。

## 4. 三个空间频率

### 4.1 宏观地貌（约 300m–公里）

由 `landform_field()`、水文和大陆/河谷/山地结构决定：

- 山脊、谷地、海岸、河流与湖盆；
- 城市、农田、自然区的可建设性；
- 战略视图与天际线轮廓。

### 4.2 中尺度地貌（约 80–360m）

由 `mesoscale_relief()` 写入 baked heightfield：

- domain-warped swells；
- 浅沟、柔和坡肩、可读起伏；
- 草原/平原不会只剩一张低频色板。

该层仍是权威几何，因此道路、建筑和树木的接地不会因渲染而漂移。

### 4.3 微观外观（厘米–数米）

由 renderer 的法线、粗糙度、色彩 breakup、草叶、碎石和雾完成：

- 不修改 baked 高程；
- 不影响规则、碰撞和地表接地；
- 可随季节和镜头 LOD 改变。

## 5. HTML 适配器职责

HTML `makeMCTerrain()` 现在读取：

```text
heightView + surfaceView + terrainView
```

它使用绝对海拔而非每 chunk 独立 min/max 调色，并叠加湿润度、曲率、坡度、land cover 与扰动。季节调色板继续由：

```text
adapters/html/render/presets/materials.js
```

提供，避免“调参文件”和实际地面 RGB 分离。

当前实现故意没有在 shader 中再做几何 displacement；这是一条接地安全边界，而不是视觉功能缺失。

## 6. 后续阶段

第一、二阶段不等于完整风格化渲染。后续按顺序完成：

1. 将规则引擎与 worldgen 统一使用旋转后的真实 footprint；
2. 用 terrain semantics 驱动草、树、裸土、湿地和远景 LOD；
3. 接入安全的 PBR normal/roughness 层，不能改变几何高度；
4. 草、树、灌丛共享世界空间风场；
5. 为 river-delta、steppe、mountain-valley、coastal-bay 等建立截图/高度统计基线。
