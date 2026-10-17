# G34-R1 独立检查：单位修复成立，状态与域绘制仍阻断

日期：2026-09-28。结论：needs_changes / accurate partial；不接受 G34 整体通过，不启动 G35。
本检查只读取产品与旧证据、抽出实际函数做 VM 反例、运行已审阅的只读核验器、评读既有截图；没有启动 Chrome、cargo 或产品 CLI，没有改旧目录。VM 受控依赖测试不冒充真实浏览器。

## 1. 已独立确认可保留的进展

- 当前源码 viewer 与交付 c08 一致：a33a2b51d76486d45cafb9adaa95acd6f981de642fcfc38a0a6b981ce19d21aa。
- c07/c08 六文件逐字节一致；model 保持 922d404cdbd413e567d5ab89094d69ef75f785ab73e469c5676a79dc025f23ff。
- 抽取当前产品 occlusionSegmentHitMetres，直接调用当前 oracle segmentTriangleMetres，按独立解析期望复测七例全过：5m墙、0.75m墙在0.5m线段之后、两端epsilon两侧等。oracle 原始距离与调用者端点筛选分开核对。
- R1 manifest 所列663项全部存在、bytes/hash匹配。不能据此推导没有漏列。
- 合成模型可以承担正常观察者遮挡正例，真实数据扫描无正例不应逼迫制造噪声。但252个配置的结果只是采样结论，不是不存在遮挡的定理。
- 本次未重新独立跑完整grid山脊/生产测试/41函数白名单；这些仍是交付证据，不能写成检查方亲自重跑。

## 2. 必须修复的实质问题

### P1-A：outside 被重新合并成 unknown

源码 G:/difyllmwiki/GlyphWeave/bevy/crates/cli/src/g28_viewer3d.js 的922、942行附近：outsideDomain进入undefined，knowledgeComplete又取undefined===0。
独立VM直接调用该文件的完整 minimapVisibilityGrid，受控平坦有效地形、半窗footprint、128个eligible全部visible、128个outside-domain、unknownData=0、numericalDegenerate=0：实际返回partialKnowledge/knowledgeComplete=false，并把128个域外点描述为无法判断。
这违反先前合同“outside不是unknown”。必须将判断域、视锥外、真正缺失/数值退化分离；仅宣称有效采样域内通视，不宣称完整世界无遮挡。

### P1-B：partialKnowledge 浏览器项是假绿

G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g34-r1-occlusion-correctness/runs/g34r1-a12/results.json 中 partial-knowledge-state-standalone：verdict=pass，但actual.status=occluded，hidden=14，unknownData=0，numericalDegenerate=0，outsideDomain=217，outsideView=6。
驱动约305行使用“如果是partialKnowledge才检查”的蕴含条件，而没有要求进入partialKnowledge。实际截图也写“已遮挡”。这不能证明该分支已观测；21/21不能作为全覆盖验收。

### P1-C：clipPath存在，但不是 footprint∩window

当前drawMinimap只生成内缩矩形(4.375,4.375,131.25,131.25)，不会随非矩形footprint裁剪。独立执行实际drawMinimap（DOM替身，仅控制输入）：footprint为(0,0),(136,0),(0,136)，hidden中心(65.625,65.625)在内；实际rect为(61.25,61.25,8.75,8.75)。点(69,69)在rect和真实clip矩形内，却在footprint外。故会画出域外颜色。
驱动只检查clip节点存在和url，不检查裁剪几何。内缩半格也会无理由丢掉窗口边缘合法绘制面积。必须按真实域几何裁剪，不以“存在clipPath”代替几何正确。

### P1-D：短状态不在140×140小地图框内

人工查看 G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g34-r1-occlusion-correctness/runs/g34r1-a12/browser/g34r1-partial.png：状态栏横跨主画布底边，文字从画布左侧开始，而小地图在右下角。
模板的span是svg兄弟，absolute left:0/right:0/bottom:0相对外层画布定位；CSS注释所称“框内”与截图不符。不得只凭字符串包含文案验收，应测DOM矩形、实际遮挡与点击命中。

## 3. 性能风险：测到了，尚未控制

a12 performance.json实际median83.2ms/p90 96.9ms/max98.5ms；同文件另有90ms候选摘要和G33 10.5ms，不能混用为本次严格比值。触发方式不等价，不能说精确慢了多少倍，也不是FPS。
原任务要求计算次数/缓存命中，本记录未提供。下一任务先受控计数和剖析，再以不降低16×16语义、不跳过相机状态为底线优化。旧任务没有明确数值性能地板，不追溯捏造不通过阈值；新阈值前瞻写入R2。

## 4. 当前证据闭环与纪律

- 重新运行原只读verifier：exit1、ok=false，problems=[enumerated closure size differs from the manifest]。实际闭集664项，漏列confirm-delivery.mjs；663已列项无漂移；核验前后全目录无变化。不能继续称“当前ok=true”。不删除漏列物、不重写R1 manifest，新轮只读绑定并更正。
- 两个被误报“没有逐文件清单”的旧目录实际为：
  G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g33-r2-c1-evidence-closure/manifest.json（16/16匹配）；
  G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g33-r2-c1-r1-doc-correction/manifest.json（39/39匹配）。
  是核验路径错误，不是这两个旧轮缺证。
- 首轮台账误写后又加指针注释是已承认的旧输入修改，不能因最终“解释过了”称零漂移。今后只在新目录登记。
- mtime只能作为时序线索，不能单独证明修改归属。G33-R2旧核验输出优先和既有C1快照hash/更正文档交叉核对；缺证不猜写入者。
- cargo3与执行者披露实际6不一致。下一轮分列可证调用、披露调用、无法恢复的明细；不得以四份文档复制同一个低估值当账务正确。

## 5. 对执行积极性的判断

没有证据判断主观懈怠或故意造假；有实质修复与反例校准工作。但确有可证明的执行质量问题：必测状态以空条件通过、几何验收降为节点存在、视觉位置未核对、旧目录误写、路径与封存后漏列。这些不能用“资源便宜”或“已披露”自动豁免。本轮不再拆纯文档C1，集中修复和验收。

## 6. 独立证据

目录 G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g34-r1-20260928/：
- independent-check.mjs / independent-check.stdout.json：产品原函数、七例单位、outside状态反例、实际renderer裁剪反例、a12原记录。
- closure-check.mjs / closure-check.stdout.json：独立枚举、只读verifier、c07/c08与正确旧路径清单。
以上是本次检查证据，不混入执行者R1目录。后续任务见 G:/difyllmwiki/GlyphWeave/docs/collaboration/dsh-b01-g34-r2-domain-state-render-task.zh-CN.md。