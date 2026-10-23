# 相机输入修复 r245

## 问题

旧预览在不同视图采用了不一致的左键语义：

- 街景和探索模式：左键默认旋转；
- 天空/港湾视图：左键默认平移；
- 按下 `Alt+左键` 才临时旋转，但释放后无条件恢复平移。

因此 `Alt+左键` 在街景/探索模式没有可切换的状态，释放后的映射也不一定与
当前视图一致。

## 修复

所有 2.5D 视图统一使用同一输入合同：

| 输入 | 动作 |
|---|---|
| 左键拖动 | 平移 |
| `Alt+左键` 拖动 | 旋转 |
| 右键拖动 | 旋转 |
| 中键拖动 | 平移 |

映射在 `pointerdown` 捕获阶段写入 `OrbitControls.mouseButtons`，早于
OrbitControls 自身的 `pointerdown` 读取；释放、取消和窗口失焦时恢复默认
映射。WASD 第一人称移动不受影响。

## 验证

- `node --test adapters/html/core/camera.test.mjs`
- `node --check adapters/html/app.js`
- `node --check adapters/html/core/camera.js`

浏览器运行态证据在重新复制适配器并烘焙后补入。
