# G33 独立检查：needs_changes；不能用 38/38 与 23/23 替代语义正确性

日期：2026-09-28。本轮检查员只读产品/worker 证据，运行两个隔离 Node/VM 检查脚本，查看 960 截图；未启动 cargo、产品 CLI、浏览器，未修改产品、旧包、旧证据或 Git。

## 1. 裁定与保留成果

G33 = needs_changes；g33-overview-minimap-1 暂不采用；G32 推荐包仍作为已接受回退基线。A 冻结/B 总体 partial/C 不推进；不进入 G34。

保留成果：小地图点击/拖拽基本功能、紧凑布局、100 项测试的已有报告、两份 audit 报告、模型不变、viewer 源码与最终包一致。独立枚举当前 G33 清单 39 项，0 stale/0 missing/0 unlisted；receipt/delivery 绑定匹配。这只证明当前文件集自洽，不证明先前失败记录没有被覆盖。

不能把 38/38 称作 38 项真实浏览器：其中前 12 项是 preflight；另有 g33-rigor 23 项，混合 VM、浏览器内纯函数求值与交互。两者只证明各自实际断言，缺少的场景不能自动通过。

检查证据根：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g33-20260928/
- independent-check.cjs / independent-check.json：独立枚举、实际产品纯函数、抽取的真实事件处理器。
- footprint-check.cjs / footprint-check.json：冻结 viewer 在无浏览器 VM 中运行，独立矩形多边形裁剪比较。

## 2. P1：北向与既定坐标系相反（合同本身也写错了）

G:/difyllmwiki/GlyphWeave/bevy/crates/cli/src/g28_viewer3d.js:443–458

保存坐标语义是 X 东、Y 上、Z 南。SVG y 向下，北向上时应 py=(z-minZ)/depthM*size。
当前公式 size-(z-minZ)/depthM*size 把 minZ 北边放到底部，把 maxZ 南边放到顶部；HTML 却把顶部刻度命名为 minimap-north。
独立实测：window=[256,32,64,64]，北西角→(0,140)，南西角→(0,0)。现有 rigor/corner-orientation 甚至把这个结果称作 north up。
因此“与自定公式一致”不能验收世界方向正确。应修合同、正反映射、北向文字与独立测试，不能只改标签规避方向语义。

## 3. P1：最终画出的 footprint 不是正确窗口交集

同文件:544–563 的 drawMinimap 对每个顶点坐标独立 clamp，而不是裁剪多边形。原始角射线求交近似正确，不等于 SVG polygon 正确。

独立复算真实默认相机（target=[288,0,64], distance=89.6, pitch=0.7, yaw=0.6，canvas=1280×664.8125）：
- 当前逐顶点 clamp 对应世界面积：2693.646555481 m²；
- 同一原始 footprint 与保存窗口的正确裁剪交集：4092.594474649 m²。
俯视 pitch=1.5533、yaw=0.6：1323.099698991 vs 4092.684934693 m²。
窗口本身 4096m²。这是显著的覆盖显示错误，不是视觉偏好。

浅视/仰视分支又使用 60*max(window,distance) 的任意远点，并标记 clampedToWindow。finite-only 的断言不能证明真实视域。应对有限保存矩形做平面与视锥半空间的交集；无覆盖时画空，不得编造满窗/三角形。UI 必须说明“target.y 水平参考面上的视域，不是地形真实可见/无遮挡范围”。

## 4. P2：边界、退化输入与手势归属未闭合

同文件:443–514、1659–1690。

独立运行实际产品函数结果：
- widthM=0 被替换为1，仍返回 x=256.5；depthM=-1 仍返回 z=32.5，违反退化拒绝。
- 捕获后拖到用户单位 (280,-140)，target=[384,0,160]，超出 x[256,320]/z[32,96]，没有合同要求的 clamp。
- startTarget=[] 未拒绝，产生 y=undefined（JSON 序列化为 null）。
- 第1 pointer 活动时，第2 pointerdown 把 owner 改成2；foreign pointerup 会应用一次跳转并清理原手势。

这些是 Node/VM 中实际函数/闭包反例，不冒充物理触屏观察。修复时只改 G33 新手势入口；不要顺手重构 G32 主画布状态机。

## 5. 证据和口径问题

1. worker 已披露 run-id 硬编码，第二次覆盖第一次结果/截图。当前目录名 a02，记录内部仍 runId=a01；原始失败记录不可由后写文字替代。
2. g33-rigor 有独立 chromium.launch、23条结果与3张现存截图；budget/receipt 仍只列2次浏览器。需要核对实际启动，不能因脚本名叫 verify 就不计。
3. 主 driver 末尾只输出 summary，没有按 fail/notObserved 设置非零退出码，也没有启动前独立 required 集合；漏执行可被误报为全绿。
4. 38项与23项的名字中未发现完整坏包拒绝恢复、主图真实 wheel/侧栏隔离的闭环；rigor 的 footprint 是 page.evaluate 直接调用纯函数，不等于实际相机→render→SVG 在全部视角下正确更新。
5. CLI台账将第二次 export+audit 合计一条；按真实进程至少有一次被拒+两次export+两次audit=5。用户允许继续消耗资源，不等于可以把5写成4。
6. report/verification-note/receipt 中存在33 vs39文件、a01 vs a02、截图2 vs3等旧摘要，下一轮在新目录统一纠正，不回写旧记录。

## 6. 对以前检查结论的更正

此前本检查方在 G32-R2-C1 的回复中称“无额外 Chrome、无超支”，但其 budget.extraProcessesDisclosed 明确载明3次 scratch 探针含 Chrome。该陈述错误，现撤回“过程完全合规”评价；正式会话97项的功能证据不因资源口径而自动作废，G32限定产品接受暂保留。scratch/authoring 同样是真实运行。

此前 G32-R1 检查中直接采信“旧位置移动约9.4px”“1.4%精确解释全部误差”的数字也不足，后续不得复用为独立证明。以后以原始前后快照和独立计算为准。

## 7. 下一步

发布 G33-R1，集中完成方向、最终视域裁剪、边界/归属和证据工具修复；给予可迭代的软预算，不再一错就停再开C1/C2。范围与验收不放松。无需重跑 G30/G31 全线或修补不可恢复历史记录。
任务：G:/difyllmwiki/GlyphWeave/docs/collaboration/dsh-b01-g33-r1-minimap-correctness-task.zh-CN.md