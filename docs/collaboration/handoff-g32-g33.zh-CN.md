# GlyphWeave G32→G33 审核与项目计划交接包

交接对象：下一位独立审核/项目规划负责人
交接原则：只读继承已验基线；任何新功能必须先有范围合同，再有实现授权。

## 0. 一句话总状态

G32 的相机平移、手势生命周期、紧凑布局和真实 WebGL2 复测已经在声明范围内完成，当前裁定为：

`accepted_scoped_with_process_exceptions`

G33 尚未开始，仅完成范围闸门任务书；不得直接进入产品实现。

G32 不代表 ST18、默认世界、自然生成、工程安全、全城可达、route/bake/realize 或发布链已经完成。

## 1. 真实工作区与基线

实际 GlyphWeave 仓库：

`G:/difyllmwiki/GlyphWeave/`

注意：`F:/modenmapbox/` 是当前 Codex 工作区，但不是本批 G32 协作证据的实际根目录。后续审核必须先确认工作区，禁止把 F 盘当作 G32 产品根目录。

### 1.1 推荐冻结产品包

`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g32-r2-layout-closeout/cli-runs/c01-export/`

生产 ID：`g32-camera-pan-3-layout-1`

当前关键文件事实：

| 文件 | bytes | SHA-256 |
|---|---:|---|
| `bevy/crates/cli/src/g28_viewer3d.js` | 94,750 | `11a2a9544512d425590635136439d51907cc300625c23638472624ec6e27486e` |
| `bevy/crates/cli/src/context_preview3d.rs` | 205,123 | `4202ccc3d8a0a1eadb810339072b00f313be0506d8b85fa1d3a4c4f0b754789f` |
| package `viewer3d.js` | 94,750 | `11a2a9544512d425590635136439d51907cc300625c23638472624ec6e27486e` |
| package `render-model-3d.json` | 517,333 | `922d404cdbd413e567d5ab89094d69ef75f785ab73e469c5676a79dc025f23ff` |
| package `preview3d.html` | 527,911 | `9e4d2bdcd94bca9340158c861bb63c924012c9ab294c1c41f951553fd820fbc6` |
| `bevy/target/debug/glyphweave.exe` | 42,355,200 | `db5c9488f5abe0c9f86e94551ee6216ab0e653744ac0b736a44126fb991191cb` |

### 1.2 已验基线

G31-R1 基线包：

`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g31-r1-entity-layering/cli-runs/c01-export/`

G31-R1 model SHA 同样为：

`922d404cdbd413e567d5ab89094d69ef75f785ab73e469c5676a79dc025f23ff`

G32-R1 和 G32-R2 旧目录均只读保留，不得回写。

## 2. 推荐阅读顺序

先读交接与裁定：

1. `docs/collaboration/handoff-g32-g33.zh-CN.md`（本文件）
2. `docs/collaboration/b01-g32-final-verdict.zh-CN.md`
3. `docs/collaboration/dsh-b01-g33-scope-gate-task.zh-CN.md`

再读 G32 最终证据：

4. `bevy/work/collaboration/ds-b01-g32-r2-c1-final-browser/report.zh-CN.md`
5. `bevy/work/collaboration/ds-b01-g32-r2-c1-final-browser/c1-measurement-contract.zh-CN.md`
6. `bevy/work/collaboration/ds-b01-g32-r2-c1-final-browser/delivery.json`
7. `bevy/work/collaboration/ds-b01-g32-r2-c1-final-browser/facts-ledger.json`
8. `bevy/work/collaboration/ds-b01-g32-r2-c1-final-browser/manifest.json`
9. `bevy/work/collaboration/ds-b01-g32-r2-c1-final-browser/receipt.json`

然后读 R2 产品修复与独立检查：

10. `docs/collaboration/b01-g32-r2-independent-inspection.zh-CN.md`
11. `docs/collaboration/dsh-b01-g32-r2-layout-closeout-task.zh-CN.md`
12. `bevy/work/collaboration/ds-b01-g32-r2-layout-closeout/report.zh-CN.md`
13. `bevy/work/collaboration/ds-b01-g32-r2-layout-closeout/manifest.json`

最后读 R1 生命周期阻断与修复：

14. `docs/collaboration/b01-g32-r1-independent-inspection.zh-CN.md`
15. `docs/collaboration/dsh-b01-g32-r1-pan-lifecycle-repair-task.zh-CN.md`
16. `bevy/work/collaboration/ds-b01-g32-r1-pan-lifecycle-repair/report.zh-CN.md`
17. `bevy/work/collaboration/ds-b01-g32-r1-pan-lifecycle-repair/runs/g32-r1-a03/results.json`

## 3. 当前完成情况

### 3.1 G31

状态：`accepted_scoped_with_process_exceptions`

已确认：

- 实体顶/侧层次在独立几何遮罩后达到限定阈值；
- 俯视 C3/C2 有真实补证；
- 默认渲染基线可用；
- 历史 aux 超支、旧归档缺项等流程例外保留。

G31-R1 只作为保存窗口和比较基线，不把它解释成工程城市生产能力。

### 3.2 G32-R1

状态：`partial`，之后被 R2 产品修复取代，不单独采用。

修复内容：

- 删除未清理的闭包 `pointerStart`；
- 统一使用 `state.pointerGesture`；
- `panCount` 按完成 pan 手势计数；
- `panMoves` 按实际应用的 pan step 计数；
- 非有限手势拒绝最多计一次。

R1 的 6 项浏览器红项中，布局是真实缺口，其余主要是驱动 rect、负对照和快照顺序问题。R1 aux `2/1` 的超支仍是历史事实。

### 3.3 G32-R2

状态：`partial`，随后由 C1 收口。

产品变化：

- 只修改 `context_preview3d.rs` 的 CSS 间距/字号；
- viewer 字节不变；
- 默认布局面积达到：
  - 960×640：`0.7877`；
  - 1280×800：`0.8310`；
- 摘要 43px、1 行、无页面溢出。

R2 初次浏览器会话的 3 项红灯均为驱动测量问题：布局重排中途读控件、terrain 点在 canvas 外、展开组未收起即测面积。

### 3.4 G32-R2-C1

状态：`delivered_for_inspection`，经独立检查后转为 G32 限定接受。

一次 aux 调用内完成：

```text
seal → preflight → offline checks → 唯一 Chrome → report
```

结果：

- 97/97 全通过；
- 96 项 required；
- 0 失败；
- 0 未观测；
- browser 真实启动 1 次；
- aux 1/1；
- cargo/CLI 0 使用；
- 输入包、R1、R2 目录漂移 0。

三项关键收口：

1. browse 12/12、inspect 13/13、展开相机组 5/5；
2. 当前 canvas 内部真实 terrain 点击返回 `terrainTriangle`，独立射线投射最近命中一致，pickCount 精确 +1；
3. 默认布局面积保持 `0.7877 / 0.8310`。

## 4. 必须检查什么

### 4.1 每次接手任务先检查

- 工作区是否为 `G:/difyllmwiki/GlyphWeave/`；
- 当前产品源码是否仍匹配冻结 hash；
- G32 推荐包的 6 个文件是否匹配 seal；
- G32-R2-C1 的 manifest/receipt/delivery 是否绑定；
- G32-R1、G32-R2 是否零漂移；
- Git 未被新任务误写入产品或旧证据。

只读收尾核验命令：

```powershell
node G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g32-r2-c1-final-browser/verify-delivery.mjs G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g32-r2-c1-final-browser
```

预期：`ok: true`、`problems: []`。

### 4.2 产品正确性检查

任何 G33 产品修改都要检查：

1. 生成模型是否仍为闭集 v1，字段完整、来源绑定、有限数值、拓扑合法；
2. viewer 与 Rust `include_str!` 绑定是否一致；
3. 41 个受保护函数是否逐字节不变，除非新合同明确允许；
4. shader 结构、材质、光照、雾、颜色常量是否发生非声明变化；
5. terrain topology：4096 vertices、7938 triangles、23814 indices；
6. 总绘制：8022 triangles、solid draw 7；
7. terrain/entity/road picking 是否仍使用真实索引三角和保存几何；
8. pan 是否保持角度、距离、Y 不变，只改变 target；
9. pointer lifecycle：pointerdown 锁 mode，pointerup/cancel/lostcapture/blur 清理；
10. 第二 pointer、UI/侧栏、普通旋转、滚轮、选择都不被 pan 劫持；
11. 默认/俯视/yaw≈90°两尺寸都要覆盖；
12. WebGL2、GL error、console error、page error 全部分类记录；
13. 坏包必须拒绝、清空残留，并在同页恢复干净包；
14. G32 布局门槛：960×640 ≥0.75，1280×800 ≥0.80，摘要 ≤48px且 ≤2 行，无溢出；
15. 所有声明为可达的控件必须通过真实矩形和 `elementsFromPoint`，不能只看 DOM 存在。

### 4.3 证据质量检查

必须区分：

- 离线纯函数/VM fixture；
- 真实 Chrome 鼠标/键盘操作；
- `page.evaluate` 受控事件；
- 只读 DOM 探针；
- 历史引用。

不能用：

- 离线 fixture 冒充浏览器；
- 只读 probe 冒充正式交付会话；
- 旧截图证明新产品修改；
- 报告文字替代原始记录；
- manifest/receipt 绑定替代产品行为证明；
- `pickMethod` 这个输入来源字段冒充求交算法证明。

## 5. 未来详细工作计划

### 阶段 A：G33 范围闸门（当前下一步）

状态：未完成、未授权实现。

必须先生成：

- `g33-scope.zh-CN.md`；
- `g33-baseline-binding.json`；
- `g33-budget-proposal.json`；
- `g33-stop-rules.zh-CN.md`。

内容必须明确：

1. 一个用户可见目标；
2. 与 G32 的冻结边界；
3. 不包含的能力；
4. 可离线证明的内容；
5. 必须真实浏览器观察的内容；
6. 成功/失败/未观测判定；
7. 预算和备用额度；
8. 驱动失败与产品失败如何分栏；
9. 是否需要视觉 intent 和基线截图；
10. 完成后交给独立审核方裁定，不自宣通过。

阶段 A 禁止运行 cargo、CLI、浏览器，禁止修改产品源码。

### 阶段 B：G33 合同与备份

获得实现授权后，先做：

1. 写产品合同；
2. 写 visual intent（如果目标涉及观感）；
3. 对所有源文件做原字节备份；
4. 记录 baseline package/source/exe hash；
5. 定义产品白名单；
6. 定义不变量、反例和停止规则；
7. 建立新证据目录，不复用旧目录。

### 阶段 C：离线实现与硬闸门

顺序：

1. 先写纯函数/独立期望；
2. 先做正例、负例和边界例；
3. 做 whitelist diff；
4. 做 model/package/source binding；
5. 做 fixture；
6. fixture 未全绿不得启动 Chrome；
7. 所有余额、run-id、截图 key、目录路径在启动前校验。

### 阶段 D：生产链

按任务书执行：

```text
unit tests
→ correct bin build
→ export
→ audit
→ optional determinism export
→ package seal
```

`include_str!` 相关 viewer 修改必须重新 build；不能用源码或旧 exe 替代最终构建证据。

### 阶段 E：一次完整真实浏览器会话

根据目标合并场景，但至少要有：

- 两种视口；
- 默认与目标状态；
- 真实鼠标/键盘交互；
- 关键正例与反例；
- 错误分类；
- 坏包拒绝与恢复；
- 性能定义和采样口径；
- 真实截图及元数据；
- 禁止用另一次探针替代正式记录。

### 阶段 F：收口与独立裁定

最后顺序：

```text
raw records
→ report/facts/budget/command log
→ manifest
→ receipt/delivery
→ independent read-only verification
→ final verdict
```

manifest 必须最后枚举绑定文件；不能用事后文档排除绑定范围。成功只交 `delivered_for_inspection`，最终接受由独立审核方完成。

## 6. 资源与预算原则

资源不应被过度压缩到无法完成必要证据。建议以后：

- cargo 至少包含一次正式构建链，另留一次修复余量；
- CLI 至少包含 export + audit，若确定性是硬要求再加一次；
- browser 至少保留一次正式复测余量；
- aux 必须包含夹具自检和正式会话的安全余量，避免因参数/时序错误耗尽；
- 预算超出必须单列，不用历史余额抵消，不事后合法化；
- 允许合理追加一次有明确目标的复测，但禁止无限微调到绿色；
- 一次失败后先区分产品问题、驱动问题、证据缺失，再决定是否申请资源。

V4.1 速度和成本允许更宽松的额度，但不能降低证据口径，也不能用更多资源替代清晰的合同和停止规则。

## 7. 不可自行改变的裁定

- 不把 G32 改称无条件通过；
- 不让 G33 自动开始；
- 不把 G32 证据扩展为 ST18/工程安全/全城可达；
- 不回写 G31、G32、R1、R2、C1 旧证据；
- 不把历史超支删除或吸收；
- 不用旧截图证明新版本；
- 不在没有新任务书时扩大产品白名单。

## 8. 交接完成条件

下一位负责人读完本文件后，应能做到：

1. 独立复核 G32 当前基线；
2. 解释 `pickMethod` 常量口径；
3. 区分真实产品失败和驱动失败；
4. 不重复 G32 的 cargo/CLI/browser 工作；
5. 先完成 G33 范围闸门，再请求实现授权；
6. 继续保留完整历史异常和证据链。
