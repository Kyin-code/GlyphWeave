# G29-R1独立检查：产品版本与恢复通过，控件补证驱动仍有缺陷

日期：2026-09-24（Asia/Shanghai）。**G29仍needs_changes；接受生产者版本、新包审核、坏包恢复及记录器隔离的局部闭合，不把尚未重测的14项红色判为已通过。**

## 1. 独立核验

证据：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g29-r1-20260924/。

- manifest91条bytes/hash匹配、0未列；receipt关键文件绑定匹配；9张截图及browser-record内部引用匹配。
- Rust182414B/2018242f…，JS67878B/6812141c…，exe42323968B/99fd458d…，与交付一致。
- 新包build id为g29-map-first-view-1；viewer仍与上一轮G29字节一致，model仍与H1字节一致。真实产品JS没有为了过检查而改几何。
- 独立CLI单测94/94。worker新包audit和二次导出结果按已绑定原记录核对，检查员本轮不另跑CLI或构建。
- 旧G29 80/H1 43/G28-R1 60/G28初始102/G27B-R1 114条全部匹配。本轮未修改worker源码、包或证据。
- browser checklist确为176项：150pass、14fail、12notApplicable、0notObserved；不存在“176全过”。查看恢复截图符合干净browse首屏；坏包、几何和布局的浏览器结果沿worker原来源引用，本轮检查员没有新Chrome观察。

## 2. D1—D4是合理驱动归因，不等于最终控件验收

实际源码和红项记录支持：字符串代spec导致DOM未找到、浏览模式试图点击隐藏检查面板、把屏外中心夹到工具栏而假判遮挡、快照缺selectionDetailsOpen。产品未必有缺陷，但在展开后的真实操作尚未通过之前，也不能保证没有产品可达性问题。

特别是报告“116项控件都绿”过宽：checklist仍有960/inspect的hud-toggle失败。这条可由驱动顺序解释，但应先按原fail保留，再用新记录覆盖其结论，不能预先删除。

## 3. 检查员运行r2预检，发现它尚不能直接使用

复制r2到检查员目录，仅去掉未使用的Playwright导入，指定--mode precheck和隔离run-root；未启动浏览器，未写worker目录。

实际结果：VM39/39，但预检**exit3**：
`ReferenceError: selfTest is not defined`

runPrecheck先把harnessSelfTests结果存入`harness`，末尾却返回`{vmRecord,selfTest,driverTests}`，后两名没有局部定义。应显式从harness取字段；不能把已写出的39项绿当整个预检已通过。

另外，r2还有两个静态/纯函数可重现判定问题：

1. enrich允许返回`reachable-after-scroll`，展开段汇总却只接受`visible-clickable`/`visible`。合法滚动后可达仍会被再次判红。须对同一语义使用一致枚举，并实际测试滚动前/后的测量记录。
2. 浏览器总exit仅看fail数量；runBrowser catch记录partError但不保证加入fail，notObserved和未到达必需项也未纳入exit。检查员按原判定式复现“异常中止/必需notObserved/缺检查项但fail=0”均可exit0的风险。这是**潜在假绿**，不是声称当前14红项已被隐藏。必须在最后一次浏览器机会前修好。

结论：不能直接执行“修正版r2”然后期待一次收口。先对驱动本身做完整预检，不再靠全浏览器试出这些变量/聚合错误。

## 4. a02描述纠正

a02的exit.json和host/console日志是**exit1、launched=false、自检marker定位失败**，不是顶层同run-id触发exit2。不可把两件事混写。

记录器的EEXIST/哨兵保留机制由隔离recorder-selftest提供，代码本身确实原子创建并拒绝已存在目录；所以这一实现可以接受，但证据来源应准确。新报告更正口径，旧a02/报告/receipt不回写，不要求另跑一次重复目录CLI凑证据。

## 5. 下一步和额度

G29累计已用cargo6/CLI9/browser3/aux7；当前上限6/9/4/7，已有浏览器余1，但aux余0。

“先执行precheck，再执行browser”是两个独立辅助调用，不是1次。本次正式只新增aux3（预检1、必要修正复测余量1、浏览器驱动1），不新增cargo/CLI/browser。新aux累计上限10，本次可用cargo0/CLI0/browser1/aux3。产品源/最终exe/新包冻结，不为这项测试收尾重新构建导出。

下一任务：G:/difyllmwiki/GlyphWeave/docs/collaboration/dsh-b01-g29-r1-controls-closeout-task.zh-CN.md。

检查员成本：隔离预检执行1（exit3、39项VMpass）、cargo1（94pass）、静态判定探针1；产品CLI0、browser0。失败记录保留。本轮真实产出接受，但G29整体仍待实际展开可达性补证，不推进G30或美化。
