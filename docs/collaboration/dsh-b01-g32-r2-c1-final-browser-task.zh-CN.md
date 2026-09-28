# DSH B01-G32-R2-C1：最终浏览器复测收口

日期：2026-09-28。依据 `b01-g32-r2-independent-inspection.zh-CN.md`。

**这是只读交付包上的一次最终复测，不是产品重做。**
不进入 G33，不修改 G32-R2 产品源码，不重建，不重新 export/audit。

## 0. 冻结输入

固定使用：

`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g32-r2-layout-closeout/cli-runs/c01-export/`

冻结：

- build id `g32-camera-pan-3-layout-1`；
- viewer、model、HTML、Rust/exe 绑定；
- G32-R2 原始 90 项记录、截图、manifest、receipt；
- G32-R1、G31-R1 全部旧目录。

新证据目录：

`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g32-r2-c1-final-browser/`

## 1. 允许修改的内容

只允许新目录中的：

- driver；
- 离线 fixture、preflight、记录器、截图和报告；
- manifest、receipt、budget、command-log。

禁止修改产品源码、交付包和任何旧目录。

## 2. 修正版 driver 硬闸门

一次 aux 调用内完成：

`preflight → offline checks → 唯一 Chrome 会话 → report`

不得另开独立探针 Chrome，不得拆成第二次 aux。preflight/fixture 失败时不得启动 Chrome。

必须先完成以下离线断言：

1. 视口切换后等待两帧，再读取 topbar、canvas 和控件矩形；
2. 默认布局测量前真实收起 `#more-camera` 和其他会改变顶栏高度的 details，并再次等待两帧；
3. browse/inspect 控件集合分开定义，browse 不把按设计隐藏的 `#hud-toggle` 判为必需可达；inspect 仍检查它；
4. 每个相机按钮在真实展开后检查自身矩形和 `elementsFromPoint`，不得仅凭初始重排中间的矩形；
5. terrain pick 点必须同时满足：位于当前 canvas rect 内、离边界至少 2px、独立/产品 pick 探针返回 terrain triangle；随后才执行真实 `page.mouse.click`；
6. layout-floors 只对默认布局判定，另行记录 expanded-controls 观察，不混用两个状态；
7. run-id 原子创建、截图键唯一、`wx` 写入、余额扁平且启动前打印。

## 3. 唯一浏览器会话

必须复测：

- `controls-still-reachable`：默认和 inspect 模式分别按适用控件检查；
- `terrain-pick-after-pan`：使用 canvas 内部且真实命中 terrain 的点；
- `layout-floors-hold-after-the-session`：收起展开组后测 960×640 与 1280×800；
- 其余 R2 的 87 项通过项保持回归，至少检查：
  - 23814/7938/4096/8022/solid7；
  - GL/console/page error = 0；
  - 坏包拒绝与同页恢复；
  - 两尺寸面积、摘要 43px/1 行、无溢出；
  - 真实 pan、选择、terrain/entity pick。

## 4. 新增额度与资源原则

新增独立额度：

- aux：**1**；
- browser：**1 次启动**；
- archive：**8 MiB**；
- cargo：0；
- CLI：0。

这不是对 R2 aux 2/1 的追认，也不把历史超支合法化。它只是允许一次必要的、低风险的证据收口。由于产品包已完成 build/export/audit，不应为了 driver-only 复测再次消耗 cargo/CLI。

## 5. 停止与交付

- 三项全绿且既有回归保持：交 `delivered_for_inspection`，不自行宣布 G32 通过；
- 任一项仍失败：交准确 partial，并区分产品失败、驱动失败和未观测；
- 不追加第二次浏览器；
- 不以只读 DOM 探针、离线 fixture 或 R2 原记录替代本轮真实观察；
- 若发现真实产品布局/控件/拾取失败，停止扩展范围，推荐回到 G31-R1，不进入 G33。
