# 地貌与植被视觉基线

> 基线日期：2026-09-03
> 目的：为**通用地图生成器**建立可重复的地形、生态语义与 2.5D 预览对照；主题仅是测试样本，不是任何主题的专用规则。

## 1. 固定生成矩阵

所有样本均使用 `1600m × 1600m`、`16` 个 chunk，并通过同一命令入口生成：

```powershell
cargo run -p glyphweave-cli -- generate-procedural-world <OUT> 1600 1600 <SEED> <THEME> <DENSITY>
```

| 主题 | seed | density | entities | scale-audit | quality-report |
|---|---:|---:|---:|---|---|
| `river-delta` | 20260903 | 0.22 | 130 | pass，边界最大 1 quarter-metre | pass |
| `temperate-plain` | 20260904 | 0.22 | 131 | pass，边界最大 1 quarter-metre | pass |
| `mountain-valley` | 20260905 | 0.18 | 150 | pass，边界最大 1 quarter-metre | warn：rural share 超出 profile 容差 |
| `low-density-suburban` | 20260906 | 0.12 | 150 | pass，边界最大 2 quarter-metre | pass |

本轮最终生成物根目录（不入 Git）：

```text
G:\difyllmwiki\GlyphWeave\bevy\work\visual-baseline-20260903-r5
```

`mountain-valley` 的 warning 是土地利用比例质量提醒，不是 heightfield、sidecar、实体边界或渲染资源错误；因此保留为生成器调参基线，而不掩盖为 pass。

## 2. 渲染数据契约

所有主题共享同一个权威地形数据来源：

1. `*.height.bin`：烘焙后的唯一几何高度；道路、建筑、植被根部都以它为准。
2. `*.surface.bin`：地表类别。
3. `*.terrain.bin`：每格四个语义通道：坡度、曲率、湿润度、扰动。

HTML 不允许使用 shader displacement 重新抬高或下沉地面。PBR 只从上述 raster 构建法线与粗糙度贴图，因此不会制造第二套几何地表，更不会再次破坏道路、房屋、树木的接地。

## 3. 视觉检查项目

| 检查 | 通过条件 | 当前基线要求 |
|---|---|---|
| PBR normal | 从 baked heightfield 中心差分生成切线空间 normal，不能修改 vertex position | 每个高精度 chunk 绑定 `normalMap` |
| PBR roughness | 使用湿润度、扰动与 surface 推导，不以随机噪声覆盖道路/地块语义 | 每个高精度 chunk 绑定 `roughnessMap` |
| 草 | 读取湿润度、坡度、曲率、扰动和 surface；扰动/水面/湿泥/岩地不生草 | 与地形语义一致 |
| 树 | 与草使用同一份 `terrainAt()`；扰动、水面、过陡坡和过干处过滤 | 与生成器的自然物 clearance 一致 |
| 灌木/草丛实体 | 进入渲染前先经过同一生态过滤 | 不以 renderer 绕过地形语义 |
| 风 | 草 shader 和树冠 shader 引用同一组 world wind uniform 对象 | 同一时钟、方向、强度、尺度和 gust |
| 旋转建筑 | 90°/270° 的世界 footprint 同时用于接地、pad carve 和规则 audit | `80×20m @ 90°` 等价于世界 `20×80m` |

## 4. 浏览器基线说明

已使用本地 2.5D 预览与 Chrome-style DevTools 检查流程验证静态资源和控制台。当前环境只暴露 Codex 内嵌浏览器；OpenCode skill 所述的本机 Chrome DevTools 连接在本会话中不可用。因此浏览器检查采用：

- 确认导出的 `preview/render/wind.js`、`app.js`、terrain sidecar 与 Three.js 模块均可 HTTP 200 获取；
- `river-delta` 在完整高精度构建后可得到非黑帧；
- 浏览器控制台 `error/warn` 列表为空；
- 2.5D 切换的高精度 chunk 构建需要约 **30 秒或更久**，因此早于构建完成的截图是黑帧，不能作为渲染失败结论。

保存的非黑帧：

```text
G:\difyllmwiki\GlyphWeave\bevy\work\visual-baseline-20260903-r5\river-delta-browser-settled.avif
```

## 5. 后续使用方式

后续修改 worldgen、surface/terrain sidecar、PBR、草、树或 camera 时，至少重新生成这四个主题并比较：

- `validate-world`；
- `scale-audit`；
- `quality-report`；
- 高精度 2.5D 完整构建后的截图；
- 道路/建筑区不出现草、树、灌木；建筑底部不出现由 renderer 造成的下陷或浮空。