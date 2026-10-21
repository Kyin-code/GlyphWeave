# HTML 街景相机与立面可见性修复

日期：2026-10-21

状态：运行态不再黑屏或落入地形；战略图、街景、天际线均有可辨认内容。
视觉反馈为 `warn`，不冒充最终视觉通过。

## 1. 复现问题

香港中环 r3d 的 `?view=street` 出现两类问题：

1. 相机在部分载入中变成 `NaN`；
2. 即使相机有效，也经常落入地形、建筑内部或只看店铺背墙。

同时发现：

- `streetHotspot.road` 是 `{ road, distance }` 包装对象，旧相机代码却直接读取
  `streetHotspot.road.rotationYDeg/worldX`，造成 `NaN`；
- `routeFocus` 的优先级高于 `isStreet`，街景点被路网中点覆盖；
- 最近道路经常选中过短的 approach，而不是可供观察的主路线；
- `makeStorefront` 已按 `frontageRoadPoint` 算好朝向，却被通用
  `buildEntityCached` 用地块 `rotationYDeg` 再次覆盖；
- CGA 店铺上层窗带的 `count` 被写为 `0`，实际没有重复窗带；
- 办公塔没有进入专用体量渲染，落入空白或普通建筑 fallback。

## 2. 修复

### 街景候选

- 从 26 个店铺中按局部商业密度评分；
- 使用最终 `frontageRoadPoint` 找道路，而不是建筑中心；
- 排除 `.approach.` 短引道；
- 按路线 ID 的数字后缀重建真实道路链；
- 只选择路线内部、前后都有足够路段的候选；
- 相机采用道路链节点与道路轴线，不再使用路网全局中点。

### 相机

- 先修正 street 分支优先级，避免 `routeFocus` 覆盖街景；
- 用烘焙高度场重新约束相机高度；
- `OrbitControls.update()` 后再写一次相机位置，防止极角/阻尼把相机拉回
  旧状态；
- 使用局部斜俯视窗口，同时显示街道、店面、办公塔和地形。

### 建筑

- 店铺正面朝向来自真实 `frontageRoadPoint`，通用 yaw 不再覆盖；
- 店铺正、背、左、右四个立面都生成上层窗带；
- 底层增加两侧橱窗；
- `office_tower` 使用分段办公塔语法，带逐段退台、横带、屋顶和天线；
- `urban_building` 进入商业裙房语法；
- 城市光照提高 hemisphere/sun/ambient，避免立面全黑。

## 3. 新控制与验收

输出：

`bevy/work/rk-x/hk-central-visual-1km-r4/`

运行数据：

| 项 | 结果 |
|---|---:|
| entities | 3,011 |
| chunks | 4 |
| storefront | 26 |
| office_tower | 7 |
| market | 6 |
| stairs | 26 |
| AccessGraph | 41 connected / 0 disconnected |
| 最大跨 chunk 高度差 | 0.847m |
| scale-audit | pass |
| quality-report | warn |

运行态截图检查：

- `strategic-final.png`：非空，可见地形、道路、市场、普通建筑和塔楼；
- `street-final.png`：不再是 NaN/黑屏，画面有道路、店面、楼梯和办公塔；
- `skyline-final.png`：能辨认山体、水岸、道路和城市天际线。

`visual-feedback.json` 记录为 `warn`：

- `street-foreground-sparse`
- `facade-material-detail-low`
- `strategic-diorama-not-plan`

## 4. 仍存在的问题

1. 街景前景仍有较大铺装/地形留白；
2. 店铺和办公塔立面仍是低模程序几何，缺少完整橱窗材质和招牌纹理；
3. `view=strategic` 当前仍偏 3D 沙盘，不是纯平面规划图；
4. 城市核心区的连续街墙和公共空间密度仍不足；
5. 视觉 warn 已写入产物，不能把本轮描述为视觉 pass。

## 5. 验证命令

```powershell
node --check adapters/html/app.js
node --check adapters/html/render/buildings.js

glyphweave generate-world `
  examples/hong-kong-central-natural-evolution-1km.manifest.json `
  bevy/work/rk-x/hk-central-visual-1km-r4

glyphweave scale-audit bevy/work/rk-x/hk-central-visual-1km-r4
glyphweave quality-report bevy/work/rk-x/hk-central-visual-1km-r4
glyphweave preview bevy/work/rk-x/hk-central-visual-1km-r4 8097
```
