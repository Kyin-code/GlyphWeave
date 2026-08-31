# Rust 自动世界生成器

GlyphWeave 的世界生成真源是 `bevy/crates/core/src/worldgen.rs`，HTML 和
Godot 只读取已经烘焙的 sidecar，不负责补生成地图内容。

## 一键生成

在仓库根目录执行：

```powershell
cargo run --manifest-path bevy/Cargo.toml -p glyphweave-cli -- generate-demo-world generated-world
cargo run --manifest-path bevy/Cargo.toml -p glyphweave-cli -- scale-audit generated-world
cargo run --manifest-path bevy/Cargo.toml -p glyphweave-cli -- quality-report generated-world
```

`generate-demo-world` 使用 1000m x 1000m、固定 seed 的无地标 Manifest，自动
生成连续高度场、材质、道路、建筑变体、树木和灌木。需要剧情地标时，使用
`generate-world MANIFEST.json OUTPUT_DIR`；地标只是输入约束，不会改变 Rust
生成器的通用规则。

`generate-procedural-world` 额外支持选择城市形态主题和调整城市占比：

```powershell
# 默认 temperate-plain
cargo run --manifest-path bevy/Cargo.toml -p glyphweave-cli -- generate-procedural-world out 6000 10000 20260829
# 指定主题（dense-core / river-delta / coastal-bay / mountain-valley / low-density-suburban）
cargo run --manifest-path bevy/Cargo.toml -p glyphweave-cli -- generate-procedural-world out 6000 10000 20260829 river-delta
# 指定城市核心占比（0..1，suburban 自动取 1.2x）
cargo run --manifest-path bevy/Cargo.toml -p glyphweave-cli -- generate-procedural-world out 6000 10000 20260829 dense-core 0.30
```

## 城市形态 profile

`style.landUseProfile.theme` 决定城市形态。Rust 生成器只改变分布先验，
不改变硬约束（确定性、footprint 与水体、chunk 连续性、asset contract）：

| theme | 核心 | 路网 | 密度 | 典型特征 |
| --- | --- | --- | --- | --- |
| `dense-core` | 单核心 | 正交网格（260m） | 高 | 高密度建成区，商业/学校贴核心 |
| `river-delta` | 3 核心沿轴 | 河道主轴 + 支路 | 中高 | 多中心、双水渠、农田占比高 |
| `coastal-bay` | 双核心 | 环状放射 | 高 | 环状路网，滨水商业倾向 |
| `mountain-valley` | 双核心沿谷 | 谷轴 + 稀疏支路 | 中 | 山谷轴向聚落、林场占比高 |
| `temperate-plain` | 单核心 | 正交网格（360m） | 中 | 规则路网、农田/郊区组团 |
| `low-density-suburban` | 单核心 | 宽间距网格（460m） | 低 | 大住宅块、绿地多、路网疏 |

## 全局城市化场与区域系统

**城市化强度是全局连续的场**（`urbanization_field`），不再是每个 chunk
各自做一个小型城市。它以场景中心为峰值，向外平滑衰减到野外地带：

```
城市核心 → 城区 → 郊区 → 乡村 → 林地/山地（中心城市化 → 边缘荒地）
```

每个世界位置由城市化场 + 地貌共同分类为 `RegionType`，同一字段被所有
chunk 采样，所以：

- **跨 chunk 无缝**：chunk 边界两侧不会跳到不相关的区域类型。
- **全局比例**：城市占场景的比例由 `urbanCoreRatio + suburbanRatio`
  决定（城市半径），中心 chunk 是满满的城市，边界 chunk 才是大片荒地。
- **支持非城市化场景**：把 `urbanCoreRatio` 调低即可得到乡村/荒漠为主的
  大陆；荒漠绿洲可通过 theme + 低城市占比实现（城市只在中心一小块）。

### 区域类型 → 生成内容

| RegionType | 生成内容 |
| --- | --- |
| `UrbanCore` | 高密度围合住宅（中国式小区高层塔楼）、停车场、绿地、路灯 |
| `Urban` | 中高层住宅块、停车场、绿地 |
| `Suburban` | 美式独栋社区（低矮住宅+前院+行道树）、大绿地 |
| `Rural` | 农田、牧场、水井、零星树 |
| `Forest` | 林场、牧场、零散树 |
| `Mountain` | 山地林场为主 |

### 两种住宅形态（小区 vs 社区）

参考中国小区与美国社区的实际规划差异：

- **中国式围合小区**（`residential_tower`，UrbanCore）：高层塔楼群围绕共享
  庭院，建筑退线、内部集中绿化与停车，密度高、楼间距受日照约束。
- **美式开放社区**（`residential_home`，Suburban）：独栋住宅 + 前后院草坪，
  贴线建造、低密度、绿地分散到每户，路网更疏。
- `residential_block`（Urban）：中高层住宅块。

三种形态通过 `core_density` / `suburban_density` 调节填充强度，
由城市形态 profile 决定侧重点。

### 占用网格与坡度约束（统一冲突检测）

Rust 生成器内置一个**确定性占用网格**（`OccupancyGrid`，10m 单元），
所有地块按「道路 → 建筑 → 植被」顺序生成并自动互斥：

- **道路/水渠**先标记为硬占用走廊，可互相交叉。
- **建筑与地块**（住宅、商业、农田、林场）避开道路走廊。
- **树木/灌木/岩石**避开道路和建筑，只填充软地面。
- **坡度约束**（`local_slope`）：建筑限 30% 坡度、道路限 30%、山体植被
  容忍 70%，防止房屋撞进山体。

这从机制上消除了「树长在道路上」「房屋穿进山体」「绿化盖住建筑」等
问题，而不是逐个 kind 硬编码规避。

无水体现代大陆场景不再叠加 legacy 通用网格（旧 `generated.tree/building`
散点），全部由现代 profile 生成，实体数从 ~6 万降到 ~3 千且零道路冲突。

### 穿模防护（山城式地基）

HTML 端对大型建筑（住宅、学校、商业、市政、工业）应用 `applySlopeFoundation`：
采样建筑 footprint 四角 + 中心的地形高度，从最低地表到建筑底边加一段
混凝土基座/吊脚，使建筑像重庆那样"嵌入"坡地，避免悬空或被草地埋没。

## 面积统计与区间审计

`quality-report` 不再只统计实体数量。每个场景输出 `landUseArea`：

- `urbanM2 / ruralM2 / natureM2`：城市 / 乡村 / 自然三大区的实体 footprint 面积。
- `urbanRatio / ruralRatio / natureRatio`：相对场景面积的比例。
- `byKindM2`：每种地块类别的面积明细。

`landUseProfile` 提供 `urbanTarget / ruralTarget / natureTarget`。审计把实际
三大区占比归一化后与目标 share 对比（容差 0.22），超出区间时在 `warnings`
中提示。城市、乡村、自然的归类：

- urban：commercial_center, entertainment_center, school, residential_block,
  parking_lot, temple, church, road, building, building_tower, storefront
- rural：farmland, pasture, canal, building_cluster
- nature：green_space, mountain_forest, nature_reserve, tree, bush, rock,
  grass_clump, reed, fallen_log

## 地貌先行（Landform）

地形不再用处处相等的正弦噪声叠加。Rust 先生成一个**连续的世界空间地貌场**
（`landform_field`），把场景划分为河谷 / 平原 / 丘陵 / 山地四个地貌带：

- 地貌带只依赖世界坐标 + seed 的平滑分形值噪声，天然跨 chunk 连续。
- 高程是地貌场的连续函数：河谷低平、平原缓坡、丘陵起伏、山地高峻，
  没有跨 chunk 的台阶。
- `surface`（地表材质）跟随地貌带：河谷土壤、平原草地、丘陵草+石、
  山地岩石+高寒草。
- 地块生成也是地貌感知的：农田优先河谷/平原，森林向丘陵/山地聚集，
  避免在陡坡上糊荒地。

`value_noise2d` / `fbm2d` 是平滑插值的分形噪声，取代了原来的逐格哈希
噪声，保证同一世界坐标永远得到同一地形，且相邻格高程差被限制在约 4m
以内（水面与 chunk 接缝都连续）。

## 侵蚀雕刻（Erosion Carve）

为了让自然谷地更真实，Rust 在 `terrain_height_with_geometry_carved` 里加了
一个**确定性的侵蚀雕刻项**（`erosion_carve`）——它不是粒子模拟（那会破坏
纯函数高度场），而是模拟水蚀的**结果**：

- 用三级噪声（主河 / 支流 / 细流）生成谷地网络，噪声低洼处下切加深。
- 只在**真正低洼的自然地面**雕刻（`low_ground` 掩码），山脊保持高峻。
- **城市区被 `wild` 掩码归零**（`urbanization_field > 0.55` 即城区）——
  湾区、商业街、住宅区保持平坦，不会被侵蚀拉出坑。
- 蚀量是 `(x, z)` 的平滑函数，天然跨 chunk 连续，与实体 world_y 采样
  严格一致。

这借鉴了 symbios-ground-lab / redblob mapgen4 的"水位是固定参照面、
岸线在 0、陆地 > 0"原则，以及轻度水文侵蚀让谷地自然、城区平坦的思路。

## 建筑规则化（CGA Rules）

`adapters/html/app.js` 里的商铺（storefront）和滨海度假建筑（resort_lodge）
改成了**声明式 CGA 风格规则**驱动，而不是硬编码的 mesh 堆叠：

```js
const CGA_STOREFRONT = [
  { kind: 'box',   w: '~1w', d: '~1d', h: '~1h', y: '~0.5h', mat: 'wall' },
  { kind: 'repeatY', start: 4.6, step: 3,
    part: { kind: 'box', w: '~0.78w', d: .12, h: .9, mat: 'glass' } },
  { kind: 'box',   w: '~0.84w', d: .8, h: .5, y: 2.6, mat: 'trim' },
  ...
]
```

- 尺寸支持 `~0.8w`（实体宽的比例）、`[2, 4]`（确定性随机区间）、米数。
- 块支持 `repeatY`（楼层重复）、`mirrorX`（镜像）、`cond`（条件门）。
- 材质用 `CGA_MATS` 调色板（玻璃/白墙/木百叶/砖/金属…）。
- 同一解释器 `cgaBuild` 可套用任意风格规则集，换风格只需换一组数据——
  「深圳湾度假风」和「后海商务风」就是两套规则集，无需改代码。

这借鉴了 symbios-ground-lab 的 CGA（Split / Extrude / Comp / Repeat）
思想，但用纯 JSON 数据 + 轻量解释器，避免引入运行时解析器。

## 现实建筑类型

现代 profile 会生成以下地块，按真实城市逻辑布局：

| 类型 | 布局位置 |
| --- | --- |
| `town_hall` 市政厅 | 城市核心，带广场 |
| `market` 集市 | 核心旁，摊位+雨篷 |
| `industrial` 工业区 | 环城路外侧，远离住宅核心 |
| `water_well` 水井 | 核心/次级核心旁 |
| `school` 学校 | 住宅区邻近 |
| `commercial_center` 商业中心 | 核心 + 次级核心 |
| `entertainment_center` 娱乐中心 | 核心 |
| `parking_lot` 停车场 | 商业/学校/工业旁 |
| `sidewalk` 人行道 | 主干道两侧 |
| `street_lamp` 路灯 | 沿主干道排列 |
| `road_sign` 路牌 | 干道交叉口 |
| `temple` / `church` | 老城 / 广场节点 |

## 硬约束

- 水体几何从 Manifest 的 `water` 与 `lake`/`river` landmark 推导，不读取地名
  或字符串猜测。
- 树、建筑、道路和家具在确定性 jitter 后再次执行完整 footprint 水体检查。
- 任何普通世界的低洼地形不会被误标为水体；只有声明的水体才生成水面。
- 每个实体输出米制 `widthM/depthM/heightM`，并由 asset contract 审计。
- `scale-audit` 检查分块文件尺寸、hash、覆盖范围、实体契约、禁水 footprint、
  树/建筑尺寸、道路贴地和跨分块高度连续性。
- `quality-report` 会继承 `scale-audit` 的失败状态，不再把失败的审计标记为
  `pass`；若 `scale-audit.json` 缺失，报告会给出明确 warning 提示先运行
  `scale-audit`。

## 验证顺序

```powershell
cargo test --manifest-path bevy/Cargo.toml --workspace
cargo run --manifest-path bevy/Cargo.toml -p glyphweave-cli -- generate-demo-world generated-world
cargo run --manifest-path bevy/Cargo.toml -p glyphweave-cli -- scale-audit generated-world
cargo run --manifest-path bevy/Cargo.toml -p glyphweave-cli -- quality-report generated-world
```

视觉反馈仍需通过 `preview` 写入 `visual-feedback.json`；视觉 `warn/fail` 不得
被 Agent 直接解释成生成成功。

## GIS 真实数据管线（Overpass → Manifest）

程序化噪声生成器之外，GlyphWeave 支持把**真实地点的 GIS 数据**转成 manifest，
得到与真实城市一致的建筑 footprint / 道路 / 水体：

1. **拉数据**（`tools/gis-fetch-overpass.js`）——用 Overpass API 免费拉取
   目标 bbox 的 OSM 数据：
   - `way["building"]` 建筑 footprint（含 `building:levels` 楼层）
   - `way["highway"]` 道路、`natural=water`/`waterway` 水体、
     `landuse=forest`/`leisure=park` 绿地、`bridge=yes` 桥梁
2. **转 manifest**（`tools/gis-convert-to-manifest.js`）——把每个 way 的
   footprint 中心 + 宽深转成 landmark：
   - WGS84 → 本地米（等距圆柱，以 bbox 中心为基准）
   - 建筑按类型/高度分到 `building` / `building_tower` / `storefront`
   - 道路转 `road`，水体转 `river`/`lake`，桥梁转 `bridge`
   - 采样控制数量（真实城市有数千建筑，1km 场景取 1/3）
3. **生成**——manifest 走标准 `generate-world`。Rust 检测到 manifest 已含
   真实 `road`/`building` landmark 时，自动跳过 legacy 程序化城市叠加
   （`generate_entities_template` 的 `has_real_city` 分支），只保留真实数据。
4. **审计**——`scale-audit` 对 GIS 实体放宽尺寸/落水检查（真实 footprint
   权威），contract 范围优先于程序化默认。

示例：`examples/shenzhen-bay-gis.manifest.json` 是从 OpenStreetMap 拉取的
真实深圳湾后海数据（2000×2000m，132 建筑 + 145 道路 + 深圳湾水体 + 1 桥），
全部通过 `scale-audit` 和 `quality-report`。

```powershell
node tools/gis-fetch-overpass.js          # → artifacts/gis/szbay-*.json
node tools/gis-convert-to-manifest.js     # → examples/shenzhen-bay-gis.manifest.json
cargo run -p glyphweave-cli -- generate-world examples/shenzhen-bay-gis.manifest.json ..\artifacts\shenzhen-bay-gis
cargo run -p glyphweave-cli -- scale-audit ..\artifacts\shenzhen-bay-gis
```
