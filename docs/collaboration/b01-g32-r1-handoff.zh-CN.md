# G32 → G32-R1 交接单（给下一个 worker）

裁定（本轮结束时的正式状态）

| 项 | 状态 |
|---|---|
| G32 | **partial_blocked**（实现存在、首手势成立，手势生命周期缺陷阻断） |
| G32-R1 | **已授权、尚未开工**（任务书已发布，见下） |
| `g32-camera-pan-1` | **继续拒绝采用**，不得作为基线；包目录只作失败证据 |
| G31 | 限定验收**不受影响**，`g31-r1-entity-layering-1` 仍是已验候选与基线 |
| G33 | **不启动** |

本交接单只写"未完成 / 必读 / 必须覆盖"；所有数字以冻结的 G32 记录为准（见第 5 节）。

---

## 1. 必读（按顺序，全部读完再动手）

1. `docs/collaboration/dsh-b01-g32-r1-pan-lifecycle-repair-task.zh-CN.md` —— **本轮任务书（唯一授权范围）**。
2. `docs/collaboration/b01-g32-independent-inspection.zh-CN.md` —— 检查员对 G32 的独立结论与执行质量批评。
3. `bevy/work/collaboration/ds-b01-g32-camera-pan/report.zh-CN.md` —— 上一 worker 的自查报告（缺陷、覆盖真相、预算自耗）。
4. `bevy/work/collaboration/ds-b01-g32-camera-pan/navigation-contract.zh-CN.md` —— **已写定的导航合同**（符号/单位/尺度/阈值/锁定/取消/恢复）；修复**不得**改其数学。
5. `bevy/work/collaboration/ds-b01-g32-camera-pan/runs/g32-a03/results.json` —— 诊断记录：27 条 VM fixture = 15 通过 / 12 失败。
6. `bevy/work/collaboration/ds-b01-g32-camera-pan/facts-ledger.json` + `budget.json` + `command-log.txt` —— 事实、额度、进程台账（含被拒进程）。
7. `bevy/work/collaboration/ds-b01-g32-camera-pan/modules/pan-math.mjs` —— **独立**平移数学（不许调用产品函数生成 expected）。
8. `bevy/work/collaboration/ds-b01-g32-camera-pan/driver/g32-driver.mjs` —— 可直接复制的 driver（注意第 3 节要改的地方）。

G32 目录与其中失败记录**已冻结**：只读引用，不要往里面写新文件、不要改旧 manifest/receipt。

---

## 2. 未完成的产品修复（最小差异，两项）

先写 `navigation-repair-contract.zh-CN.md`（新目录，**先写后改源码**），再编辑源码。

当前源码事实（`bevy/crates/cli/src/g28_viewer3d.js`，viewer 当前 sha `52045e33…` / 93673 B；动手前先做**字节备份 + hash 校验**）：

| 位置 | 现状 | 问题 |
|---|---|---|
| L1443 `var pointerStart = null;` | 闭包变量 | 除声明与 pointerdown 赋值外，**没有任何地方把它复位** |
| L1448 `if (pointerStart \|\| event.button !== 0) { return; }` | pointerdown 守卫 | 第一次手势后永久拒绝新手势 |
| L1462–1473 | pointermove 读 `pointerStart` | 旧手势继续吃 move（旋转/平移跑到旧快照上） |
| L1476–1485 | pointerup 读 `pointerStart`，只调 `endPointerGesture` | 出口只清 `state.pointerGesture`，闭包仍脏 |
| L387–399 `endPointerGesture` | 清 `state.pointerGesture`、写 `gestureLog` | 不能访问闭包变量（模块作用域） |
| L401–425 `applyPanStep` | `state.panCount++` **每个 move 一次**；`state.panRejections++` 每个被拒 step 一次 | 合同要求：`panCount` 只在**手势完成**时 +1，另有 `panMoves` 计步；`panRejections` 每次手势**只 +1** |
| L161 state | `panCount: 0, lastPan: null, panRejections: 0, lastPanRejection: null` | 缺 `panMoves` |

**修复选项（二选一，并在单测里具名）：**
- **A（推荐，唯一真源）**：删掉 `pointerStart`，pointerdown 写 `state.pointerGesture`，pointermove/up/cancel/lostpointercapture/blur 全部读同一对象；`endPointerGesture` 之后 pointerdown 可建新手势。
- **B**：保留闭包，但在同一出口函数里显式 `pointerStart = null`（需要把出口移到能访问闭包的作用域），不得留下第二个未清状态。

**计数器语义（合同第 3 条）**：`panMoves` 每应用一步 +1；`panCount` 仅当 `mode === "pan"` 且至少应用过一步时，在手势结束时 +1；`panRejections` 每次手势最多 +1（用 `gesture.rejected` 守卫），且相机保持不变。**不改** `panTargetFromDrag` 的数学、不改拾取求交。

---

## 3. 修复前必须通过的离线硬闸门（硬性，未过不得启动 cargo/浏览器）

G32-R1 任务书 §2 的要求 + 上一轮实际踩过的坑：

1. **展开更多视角**：driver 必须在**默认折叠**状态下真实点击 `#more-camera > summary`，断言 `open === true`，再点相机按钮；不得假设默认已展开、不得用列表选择代替点击。（上一轮就是这里 30 s 超时，导致 26 项计划只跑 10 项。）
2. **`expectedScreen` 统一签名 + 画布页面偏移**：所有调用用同一个 `(camera, canvasRect, world)` 形态，返回 `rect.left + p.x / rect.top + p.y`；对 1280 与 960 各做一个 fixture 校验（上一轮 3 条浏览器失败就是漏了偏移与签名不一致）。
3. **余额 JSON 扁平字段**：`{"browserAllowance":1,"auxAllowance":1,...}`，拒绝嵌套误读；启动前打印本次剩余额度与 run id。（上一轮有一次参数检查拒绝。）
4. **截图 key**：由唯一场景表生成、`wx` 写入；缺项/重复/超时都判非零。
5. **27 项 VM fixture 全通过**才可启动 Chrome；报告要像 G32 那样写清「N 条具名 fixture + 1 条汇总」的分类，不得把 VM 说成浏览器检查。
6. **preflight**：校验新 build/source/package 三方绑定；不得把当前文件读成自身 expected；包 hash 与源码 hash 必须相等（G32 时 `candidateViewerEqualsSource=true`）。
7. **不要 `--lib` 试探**；cargo 只用任务书允许的那一条链。

---

## 4. 一次浏览器会话必须覆盖（检查员清单 → 实现要点）

**硬底线（任一缺失即准确 partial）**

- [ ] 1280×800 **与** 960×640（`setViewportSize` 后重新取 canvas rect）。
- [ ] 斜视（`#cam-default`）、俯视（`#cam-top`）、**yaw ≈ 90°**（点 `#cam-rotate-right` 若干次后记录 `yaw` 角度，允许 ±4°）。
- [ ] Shift 平移（`keyboard.down('Shift')` + `mouse.move/down/move/up`）、**正反平移回位**（两次同幅反向手势后 target 回到起点，1e-6 内）。
- [ ] 普通旋转（不带 Shift 的拖拽：只改 yaw/pitch、target 不动）、普通点击（`pickCount` 恰好 +1）、滚轮（只改 distance）。
- [ ] **Shift 中途释放**：pointerdown 带 Shift、move 时 `shiftKey: false`，仍必须继续平移、不得切成旋转。
- [ ] `pointercancel`、`lostpointercapture`、`blur`：受控事件（`page.evaluate` 派发）**必须标注为受控事件**，随后用真实鼠标点击确认恢复正常。
- [ ] **第二 pointer**：手势进行中派发第二个 pointerdown/move，不得接管相机（`panCount`/`panMoves` 不因它变化）。
- [ ] **已选实体平移后仍保持选择**：先真实点击选中实体（记录 `selection.objectKey`），再平移，`selection` 与 `pickCount` 不变。
- [ ] **平移后真实实体与地形点击**：用**修好偏移**的期望屏幕点，分别点实体（`entitySolid`）与地形（`terrainTriangle`）；旧位置不再命中作为负对照。
- [ ] **默认折叠的“更多视角”展开后点击**：见第 3 节第 1 条；展开后真实点击旋转按钮并断言生效。
- [ ] 模式往返（browse↔inspect，target 保持）、两面（final/natural 各 4096 顶点、7938 三角）、图层开关、摘要 ≤48px 与可见面积 ≥80%/75%、详情图例 5 条不溢出。
- [ ] 坏包拒绝 + **同页恢复**（复制包、截断内嵌模型、拒绝后回到干净包，几何回到 8022）。
- [ ] 几何底线：**23814 / 7938 / 4096 / 8022 / solid 7**（全开），地形单显 7938 / solid 0。
- [ ] **GL 错误 = 0、console 错误 = 0、page 错误 = 0**（坏包段产生的错误要单列，不能污染前段判定）。

**另外必须做对的三件事**

1. 结果分栏：真实鼠标操作 / 受控事件 / 离线 fixture **分开写**。
2. 页面真实失败就记录状态并停止；**不得改夹具把失败改成通过**。
3. 负对照建议保留：冻结 R1 包上 Shift 拖拽仍旋转、target 不动、无 `__panTarget3d`（上一轮此项通过）。

---

## 5. 额度、停止规则与当前余额

| 类别 | G32 已用 | G32-R1 新增 | 本轮可用 |
|---|---|---|---|
| cargo | 4/4（含 2 次自耗：`--lib` 拒绝、为干净退出码的重跑） | **1** | 1（建议一次链：`cargo test -p glyphweave-cli --bin glyphweave`；若必须 build，按台账写明同链） |
| 产品 CLI | 3/6 | **2** | 2（export、audit；第二次 export 只在确定性必需且台账说明时用，**不得在失败目录上 audit**） |
| 浏览器 | 1/3 | **1** | 1（推荐同一个 aux full：先硬闸门再启动） |
| aux | 4/4 | **1** | 1（fixture/preflight 阶段） |
| 归档 | 7.691 MiB | **16 MiB** | 新目录 ≤16 MiB |

- 启动前写 `balance-before`（扁平字段 + run id），任何类别为 0 不得启动对应进程；**参数拒绝也计数**，不得事后折扣。
- 不复用/不吸收 G31/G30 余额与历史超支；R1 的 aux 5/3(+2)、G32 的 cargo 自耗都保留为历史事实。
- 新目录：`bevy/work/collaboration/ds-b01-g32-r1-pan-lifecycle-repair/`；新 build ID：`g32-camera-pan-2`；model 必须仍为 `922d404c…`，HTML `44364c4a…`、材质、光照、拾取、几何、读图全部冻结。
- 启动顺序一次连续做完：**contract → 静态/fixture 硬闸门 → cargo → export/audit → 一次 browser → report/manifest/receipt**；报告/预算/command-log/facts-ledger 必须在**最终 manifest 之前**写完，最后独立枚举绑定；报告之后若不再有产品运行变化，只做**管理绑定**（管理归档不是额度项，不占 aux、不需要再开 driver），且**不得**把 `postManifestDocumentation` 当新的排除授权。
- 修复后若仍有真实产品失败：按额度准确 partial，**不追加第二次 cargo/browser**，建议停止该功能扩展。

---

## 6. 交付判定

- 只有**全部**第 4 节项目在**一次**会话中观测到、且几何/错误底线成立，才交 `delivered_for_inspection`（**仍不得自宣通过**）。
- 缺任何浏览器项 → 准确 `partial`，逐项列出未观测。
- A 冻结 / B 总体 partial / C 不推进；**不进入 G33**。

---

## 7. 需要继承的纪律（上一轮四项批评 + 事故教训）

1. **先报告后台账，最后绑定**：`report / budget / command-log / facts-ledger` 必须在 manifest 之前写完并进闭集（上一轮报告晚写导致漏列）。
2. **管理归档不是额度**：哈希/枚举/文档/manifest 编排不占 aux，也不必为此开验证 driver。
3. **日志按记录归属**：浏览器运行是哪个 run id 就写哪个（上一轮把浏览器会话写成 a02）；所有比例、文件数、run id 一律由脚本从记录计算，**禁止手填**（上一轮手填的 top-final C3 比 0.973314 实为 0.9734852534632594）。
4. **源码写入纪律**：先用显式严格 UTF-8 读取 → 原字节备份到本轮目录并确认 hash → 再局部补丁；**备份失败立即停止**；禁用依赖宿主默认编码的全文件读写链（这条来自 R1 的源码编码事故，修复已逐字节证明但过程不可重演）。
5. **历史异常不复制成已修**：R1 的 `residual_source` 异常退出、缺失的 cargo 原始日志、R1 aux +2 超支、G32 的 cargo 自耗，都按事实保留在新的预算/报告里，不追认、不抵消。
