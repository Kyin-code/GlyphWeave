# G28-R1 独立复核：地形拓扑已修复，HUD同步与证据引用仍需补齐

日期：2026-09-23。裁定：**接受地形运行期回归修复；G28整体继续 `needs_changes`，不进入G29，不开晕渲任务。** 本轮不能再归类为“地形算法未修”，但也不能把R1写成完全收口，因为动态HUD与一处浏览器记录内部绑定仍有问题。

## 1. 已独立确认的修复

- R1清单60项逐项bytes/SHA-256匹配，无重复、无漏列；receipt关键绑定匹配；嵌套包manifest已列入。
- G28初始包、G27B-R1旧证据和固定来源未漂移。
- 当前源码：`g28_viewer3d.js` 56945 B / `eb8eba8ed295f4e1e01b1dc74b42d125da208d7481cb01c35dc63d15a18b1531`；`context_preview3d.rs` 173539 B / `1cb9f3af4c4f18096896d0e8b7c308236d1be56257b1426efcf8d1fba559919e`；最终exe 42310656 B / `882993c6c6da37fd030b60fb387e578048d54cd430a8267d48eafba929f9e538`。
- 检查员独立运行CLI单测92/92和core `engineering_` 82/82；最终包audit exit0、四段pass。此处不把检查员运行记为worker运行。
- 实际执行R1 viewer JS的38项回归全部通过：两面4096顶点、23814索引、7938三角，地形走drawElements，实体/道路走显式非索引路径，总三角8022；非法拓扑和负原点2×2斜面均拒绝/命中正确。
- R1保存的真实浏览器证据显示final/natural地形点击与独立oracle一致，地形单显、实体选择后背景清除、隐藏层拖拽、滚轮、窄屏和坏包恢复均有记录。此段沿worker来源引用，检查员本轮未新增成功浏览器会话。

## 2. 尚未闭合的真实缺陷：HUD只在boot填充

源码 `g28_viewer3d.js` 的 `fillHud()` 只在 `boot()` 中调用。surface切换、实体/道路/观测层切换的监听只调用`render()`，没有刷新HUD文本。

我在实际viewer JS的Node VM回放中观察到：

- boot final、solid draws=7、total=8022、overlay shown：HUD正确；
- 切到natural后，运行状态是natural/crop.naturalMm，但HUD仍写`from crop.finalMm`；
- 关闭实体、道路和观测层后，运行状态是solidDrawCalls=0、drawnTriangleCount=7938、overlayRecordsDrawn=0，但HUD仍显示`solid draws 7`、总8022和overlay shown；
- 恢复final/layers后HUD再次与初始文本一致。

因此报告中“渲染派生行只会陈旧”说法正确，但影响不止drawn行：**layer来源与图层状态文字也会陈旧**。`__terrainRuntime3d()`新鲜不够，用户看到的是HUD文本，体验合同要求页面事实可读。

## 3. 证据内部绑定问题

R1 `browser-record.json` 自身仍引用了两张已被聚焦复跑替换的截图哈希：

- `browser/r1-terrain-only-final.png`：记录期望 `b124c5dd…`，磁盘实际 `781322d1…`；
- `browser/r1-terrain-only-natural.png`：记录期望 `b124c5dd…`，磁盘实际 `2b0266a1…`。

`browser-terrain-only.json` 是新的正确记录，且其图片哈希一致；但旧 `browser-record.json` 仍是R1 manifest的正常条目，故不能以“manifest自洽”掩盖报告内部引用失配。不要回改已封存R1；新收尾目录需统一生成browser record、截图索引、manifest和receipt，消除这种失配。

本轮检查员曾因一版内联验证器把带scope路径按单根解析而提前报错，随后修正；修正后的验证器没有再写worker文件。另一次独立校验脚本因路径对象错误曾在worker目录临时生成`independent-verification.json`，检查员立即移出到本检查目录并披露；worker当前清单未新增该文件。该检查员操作错误不归责worker，但不写“本轮检查员零写入worker”这句绝对话。

## 4. 预算与下一步

R1当前累计：cargo 3/5（余2）、CLI 6/6（余0）、browser 3/3（余0）、aux 4/4（余0）。已有G28历史CLI+1超支继续保留。

发布一个窄范围的G28-R1-H1任务，不改变地图事实、不改地形算法、不做hillshade：建议新增cargo2、CLI3、browser1、aux1；用于动态HUD修复、最终构建/导出/audit、一次真实浏览器全状态复验和新证据闭集。新上限为G28累计cargo17、CLI20、browser12、aux11；历史超支仍单独可见。若用户不批准新增额度，至少可先在剩余cargo2内实现并测试，但不能把未重导/未复验的新产品写成完成。

**G28当前仍needs_changes；R1地形问题已修复；下一阻断是HUD动态同步和新证据引用一致性。**
