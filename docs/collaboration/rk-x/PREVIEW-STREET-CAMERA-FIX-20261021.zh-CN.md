# 2.5D 街景空屏与相机焦点修复

日期：2026-10-21

状态：运行态修复并重新烘焙；战略图、street 2.5D、scale audit 和
quality report 均通过。

## 1. 现象

`?view=street` 运行时出现大面积黑色空屏，只有左侧不足 1/5 画面能看到少量
悬空建筑和草地。问题稳定复现，不是加载瞬时状态。

## 2. 根因

修复前 street 焦点取自“离地图中心最近的单个 storefront”，当前切面最近店
在 `(508,676)`，已经靠近商业街区东南边缘。相机再固定向东南偏移，导致视锥
右侧大半落在无地形的世界外侧。

同时，`resize()` 只更新战略图 canvas 和后期处理尺寸，没有同步
`WebGLRenderer`、透视相机 aspect 和 viewport 尺寸。窗口/全屏变化后可能继续
使用旧画布比例。

## 3. 修复

`adapters/html/app.js`：

- street 焦点改为住宅、公寓、店面、市场、办公和公共广场的整体质心；
- 保留单店/路灯/道路作为没有建筑场景的回退；
- street 相机使用 105m / 44m / 150m 的稳定斜俯视偏移，直接看向城区中心；
- 不再把 street 焦点额外推向地图南侧 55m；
- `resize()` 同步 `nearRenderer`、透视相机 aspect、像素尺寸和 postFX；
- 保留鼠标旋转、缩放、平移和 chunk 导航。

这不是重做渲染风格，只修复“看不到已烘焙内容”的相机与 viewport 错误。

## 4. 运行态证据

输出：

`bevy/work/rk-x/v3-visual-focus-1km-r6/`

检查：

| 项 | 结果 |
|---|---|
| 战略图 | 城市、湖、道路、树石可见 |
| street 2.5D | 画面覆盖城区、道路、湖岸、草地和建筑，无大面积黑屏 |
| 相机 | 以 15 个城市实体质心 `(464,555)` 附近为焦点 |
| AccessGraph | 13 个实体连通，断点 0 |
| scale audit | pass |
| quality report | pass |
| `visual-feedback.json` | `pass` |

当前建筑仍使用简化体块；这是渲染层暂不提高要求的已知状态，不属于空屏或
错位。后续街区立面、临街家具和市场入口细节仍需继续实现。

## 5. 验证命令

```powershell
node --check adapters/html/app.js

$env:CARGO_TARGET_DIR='bevy/target-main'
cargo run --manifest-path bevy/Cargo.toml `
  -p glyphweave-cli -- generate-world `
  examples/natural-evolution-core-city-1km.manifest.json `
  bevy/work/rk-x/v3-visual-focus-1km-r6

bevy/target-main/debug/glyphweave.exe scale-audit `
  bevy/work/rk-x/v3-visual-focus-1km-r6

bevy/target-main/debug/glyphweave.exe quality-report `
  bevy/work/rk-x/v3-visual-focus-1km-r6
```
