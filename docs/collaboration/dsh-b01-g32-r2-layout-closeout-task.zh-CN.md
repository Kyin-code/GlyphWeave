# DSH B01-G32-R2：布局门槛修复与一次证据收口

日期：2026-09-27。依据独立检查：`b01-g32-r1-independent-inspection.zh-CN.md`。
**正式授权前置准备，但本任务需要新的独立额度；不采用 G32-R1 包，不进入 G33。**

## 0. 冻结与新目录

冻结以下内容，不改写：

- G32 原目录及 R1 目录、manifest、receipt、报告、截图；
- G31-R1 基线包及其证据；
- model、几何、mesh、拾取求交、pan 数学、pointer 生命周期修复、材质、光照、shader、读图语义。

新证据目录：

`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g32-r2-layout-closeout/`

建议新生产 ID：`g32-camera-pan-3-layout-1`。

产品白名单只允许：

- `bevy/crates/cli/src/context_preview3d.rs`：压缩 G32 导航界面的正常流布局，使 960×640 与 1280×800 达到面积门槛；同步更新生产 ID 与对应具名静态测试；
- 新目录下的 driver/modules/fixtures/evidence。

`g28_viewer3d.js` 不得改动；若必须改 viewer 行为或 pan 数学，立即停止并交回新任务，不得在本轮扩大范围。

## 1. 布局修复合同

在任何源码修改前写：`layout-closeout-contract.zh-CN.md`，明确：

1. summary 保持 ≤48px、≤2 行；
2. 960×640 visible canvas area fraction ≥0.75；
3. 1280×800 visible canvas area fraction ≥0.80；
4. 页面横向/纵向溢出均 ≤1px；
5. 所有导航控件、显示模式、地形层、图层、视角、复位、读图入口仍可达；
6. 不能用隐藏控件、删除信息、`notApplicable` 或浮层遮挡地图换取面积；
7. pointer 生命周期、pan 计数、拾取、几何必须逐函数冻结。

允许使用紧凑 padding/gap/字号或重新分组，但必须用真实 DOM 矩形证明控件仍可见可点击。

## 2. 驱动修正合同

在启动 cargo/Chrome 前，离线修正必须通过：

- 每一次 projection/pick 前重新读取当前 canvas `getBoundingClientRect()`，禁止复用初始 rect；
- old-position 负对照使用足够大的 pan 或计算投影分离度，若旧点仍在实体投影内则硬闸门拒绝该场景；
- selection-survives-pan 只比较 reselect 之后的 pickCount，不能拿点击前快照比较；
- 记录修正前后 expected/actual，不删除原 R1 红项；
- 截图 key 唯一、`wx` 写入、run-id 原子创建；
- 产品 preflight 不能把当前源码读成自身 expected。

**一次 aux 调用内**完成 preflight → offline fixtures → 唯一 Chrome 会话；preflight 或 fixture 失败不得启动 Chrome，也不得开启第二次 aux。

## 3. 生产与浏览器验证

新额度建议：cargo **1** / CLI **2** / browser **1** / aux **1** / archive **16 MiB**。不继承、不抵消历史超支；失败照计。

顺序固定：

`contract → source backup → static driver gate → cargo test/build → export → audit → one aux full run → final manifest`

浏览器一次会话必须覆盖：

- 960×640、1280×800，真实布局矩形与面积；
- 读图摘要、选择卡、导航按钮、更多视角展开；
- 斜视/俯视/yaw≈90° Shift 平移与真实点击；
- rect 更新后的独立 pan 数学一致；
- 旧位置负对照确实离开实体；
- 已选实体平移后 selection key 保持，pickCount 不因 pan 增加；
- terrain pick after pan；
- 普通旋转、点击、滚轮、取消/失焦/第二 pointer；
- 两面、图层、模式往返、坏包拒绝/同页恢复；
- 几何 23814/7938/4096/8022/solid7；
- GL/console/page errors = 0。

## 4. 停止和交付

- 任何一项真实浏览器失败，交准确 partial；
- 不追加第二次 cargo 或浏览器，不用离线 fixture 替代真实浏览器；
- 不把 R1 的 74/80 复写为本轮通过；
- 成功也只交 `delivered_for_inspection`，不自行宣布 G32 通过；
- 若布局无法在白名单内满足门槛，保持 G32 partial，推荐回到 G31-R1，不启动 G33。
