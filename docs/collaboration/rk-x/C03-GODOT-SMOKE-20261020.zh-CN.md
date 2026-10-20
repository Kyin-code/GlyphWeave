# C-V1 Godot 4.7 runtime smoke (2026-10-20)

状态：**Godot 4.7.2 stable 已在最大图工程世界上真实启动并加载分块**。

## 1. 运行环境

- 官方下载页：`https://godotengine.org/download/windows/`
- 版本：`Godot Engine v4.7.2.stable.official.ed1daf0bf`
- 包：`Godot_v4.7.2-stable_win64.exe.zip`
- 大小：`86,013,866` bytes
- SHA-256：`731980F9608D61333E5BAF54A2EF17210ACC7A538446C0CB9969F002ACA1E953`
- 项目：
  `bevy/work/rk-x/c02-maximum-engineering-20261020/world/godot/`

本地 Godot 运行时位于工作目录，不纳入仓库：

`bevy/work/rk-x/godot-4.7.2/runtime/`

## 2. 启动检查

普通 headless 启动：

```powershell
Godot_v4.7.2-stable_win64_console.exe --headless --path <world>/godot --quit-after 5
```

结果：exit 0，无 SCRIPT ERROR / ERROR。

## 3. 数据加载断言

一次性 smoke 脚本实例化 `main.tscn`，等待两帧后读取 HUD：

```text
GLYPHWEAVE_SMOKE scene=scene-0  chunk=0,1  loaded=6  X=500 Z=850 Y=0.00
```

这证明：

- `world.json` 与 scene sidecar 成功读取；
- 当前 scene 为 `scene-0`；
- chunk 坐标 `(0,1)` 可解析；
- 6 个邻接 chunk 已建立 Mesh 节点；
- 世界坐标 HUD 为有效数值；
- 进程 exit 0，无脚本解析/运行错误。

## 4. 边界

- 这不是 Godot 发布包截图或视觉审美验收；用户当前不要求渲染优先。
- 也没有完成碰撞导航、发布版剧情传送 UI 或导出模板打包。
- C-V1 的“Godot 4.7 可执行文件从未运行”缺口已关闭。
