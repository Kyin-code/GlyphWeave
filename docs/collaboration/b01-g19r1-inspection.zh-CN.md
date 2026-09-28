# B01-G19R1独立审查（2026-09-20）

## 裁定

**prior-counterexamples-fixed / new-counterexamples-found / acceptance-still-blocked**。

上一轮列出的13类构造/路由输入按worker预期复现，合法直跑/折返正例仍通过；清单与回执绑定有效。不能接受“六组问题完整修复”或推进ST18/SceneIndex/preview/audit：新的小型人工输入仍出现穿自身楼梯、窄梯误通、车辆涉水误通、引用错绑与预算内可行路被漏掉，而且独立verify重复接受。

不是要求立即实现新设施类型；这些仍属于原G19已承诺的几何/通行/审核不变量。禁止自动穿楼/穿墙/删障碍的用户规则不变。A冻结、B partial、C不推进。

## 核验方法

检查员目录：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g19r1-20260920/。

- integrity.cjs/integrity.json：223/223条SHA256+字节匹配；回执manifest hash/bytes都匹配；access/worldgen/main/auto_roads与上轮审查时hash相同，仅两份新engineering_connections生产模块变更。
- 重新cargo build CLI及cargo定向测试，共2次cargo。实际当前模块只有5项测试，5/5过（包含读取13case的一个矩阵测试）；不能声称原17项仍保留运行。没有执行历史自然测试或全库测试。
- probe.cjs/probes.json：13个worker v2 case全部重跑，另加6个独立变体。正例及接受状态逐一用独立CLI读回。thin-solid-wall与thick-wall-control均正确拒绝；不把接触容差误读为“所有薄墙都被忽略”。
- extra.cjs/extra-evidence.json：有/无短陡路的相同平坦绕路对照、预存report发布冲突、完整运行账本检查与输入变更差异。
- endpoint-probe.cjs/endpoint-evidence.json：1m端点豁免导致轮椅钻入楼梯实体。
- 合计检查员37次新CLI（全部人工小场景，最长进程超时限制15s）、2次cargo；自然/ST18重建/真实route/旧world bake/realize=0。旧worker证据与源码未改。
- 审查脚本首读UTF8 BOM回执发生解析错误，修正只读解析后重做完整hash核验；未修改回执或源文件，没有把失败当作有效核验。

## 阻断发现与复现

### P1-F1：端点豁免仍允许穿自身实体，不是合法接触

代码：core engineering_connections.rs CollisionContext::new/exempt_for约3043–3077、corridor_blocker约3098–3138。

exempt_by_node把端点所属结构的**全部parts**列为豁免；沿ground边离任一端点≤1m时直接continue该集合，不验证是否只有支承表面的接触，不区分哪一个端点真正拥有该part。

反例terminal-own-stair-solid：基于本轮合法positive-terrace，在stair-bottom=(15,0,24)与under-stair=(15.5,0,24)之间声明0.5m ground边，终点位于前两步楼梯下方，query=wheelchair。产物仍route=found、pathEdgeIds=[through-own-stair]、verify=verified；实际身体空间与踏步/支承相交。

修复要求：禁止“按所属对象整组豁免”。只允许明确证明的落脚表面接触，踏步/立板/支柱高于行走面进入身体空间仍必须拒绝；不能用扩大1m窗口、缩身体、删part修复。应让合法楼梯通过其stair边而非ground边钻入。

### P1-F2：楼梯净宽未按行人通道要求检验

代码：refresh_walkable_capabilities约3311–3405仅按每个member本身的宽度建立上方体积；CapabilityRules::decide约1380–1400对pedestrian只看blocked，不比较可用宽度；straight stair规格亦没有足够的净宽/端口宽度一致性校验。

反例narrow-stair：仅将合法直跑stair-terrace.widthM从1.6改为0.4m，端口仍1.6m，支柱可容纳，构造found且verified。当前项目pedestrian检查使用halfWidth=.75（1.5m），0.4m踏步没有承载这一通行体积。

修复要求：明确支持面可用宽度/长度、端口宽度、转向区和能力走行体积包含关系；不能沿窄网格生成一个更窄的检查体积就自动宣布行人可走。若要引入不同人群宽度必须显式版本化，不偷偷改当前参数。

### P1-F3：所谓“各自带宽”没有完整落实；车辆水域漏检，轮椅被域外坡度误拒

代码：build_ground_edge约2836–2859只计算pedestrian±.75和vehicle±1.5坡带，water统一half=.75；facts.grade_band约2876–2885使用两者最大值，轮椅没有独立±.6采样。

- vehicle-side-water：ground边z8、声明车辆净宽3m；water=[x5..9,z9..9.4]落在车辆范围内而在行人范围外，vehicle仍found/verified。
- wheelchair-outside-band：轮椅±.6与行人±.75内都平坦，只有车辆±1.5带的域外坡度=.2。轮椅被no_suitable_edge拒绝且verify仍verified。

修复要求：每项被宣称支持的模式必须使用自己的完整几何事实（坡度、水、实体、净宽、净空）；不能用一个最宽带值混授予，也不能用行人水域判据给车辆背书。unsupported模式保持明确拒绝。验证器应在保存工程栅格上重采各带，不把saved facts当独立证据。

### P1-F4：引用存在不等于端口实际落在该面上

代码：validate_references约4089–4188只核验结构id存在及levelId在字符串列表中；对未被生成器绑定使用的额外端口，没有几何包含/高度归属核验。

反例misbound-existing-surface：实际deck在(7,1,3)，端口a位于(2,0,8)却声明surfaceId=remote-deck、levelId=remote-deck:deck。该标签确实存在，ground路径仍found/verified。

修复要求：每个端口都必须解析到真实支持面/层，核对位置、高度、宽度、所有者与允许连接方式。不能仅靠字符串注册表消除悬空引用，不能把输入标签原样抄出后比较相同视为有效。

### P1-F5：搜索仍按净高差计爬升，最后拒绝短路后不搜索已有合法绕路

代码：dijkstra约3571–3725用edge.rise_m的有符号端点差累计climb，而不是有向折线的实际累计上升；previous仍按node而非label存储（非支配label的parent可能覆盖）。route约3820–3860末端重新计算真实折线，超限即budget_exhausted。

反例hump-with-valid-detour：从a到b有10m短路，中途上升1m再下降；另有a→c→d→b的18m平坦路线，maxRise=0、maxLength=100。全部ground边均构造成功，搜索却报告budget_exhausted并verified。只去掉超限短路（valid-detour-control），同一平坦绕路立即found、climb=0。

所以“超限不再报found”修好了，但“预算内可行路线选择”未修完整。修复需使用每条有向折线的真实升降资源，保留label身份/前驱一致性，目标处超限不得阻止其它可行label继续。不得只在最终报告新增一个拦截条件。

### P2-F6：回归迁入gitignored工作证据，原17项被整体替换

代码：tests::case_root约6075–6090硬依赖../../work/collaboration/.../cases。git check-ignore证实这些JSON被仓库.gitignore:43 work/忽略。干净checkout/CI没有该目录即测试失败。

当前仅5个测试；虽其中一项覆盖13case，但原17项中的大量几何/支持/篡改/单向等独立断言并未保持为可随源码交付的回归。本轮任务要求保留仍有效断言，不能以矩阵数量代替覆盖。

修复要求：把最小fixture放到受版本控制的测试资源或源码内生成；工作目录仅输出证据。逐条列原17测试断言的保留/替代路径，恢复受影响几何正负例、持久化篡改、单向/循环与支承检查。不得只把本轮新反例硬编码expected status再放弃其它不变量。

## 发布与成本：保留已修内容，区分未证部分

- 预存report的publish-collision已独立重测，正确output_conflict，原report不变、bundle/views均未生成，认可。
- v2 staged-state拒绝由定向测试通过。仍不能宣称完整crash一致性已证：CLI在staging内先commit_bundle写committed状态，之后才搬views/report/bundle；最终时间检查在report序列化和最终rename之前，验证端也没有最终发布门。未模拟崩溃/中断，不作实际残留已发生的断言；下一修复要把对外commit状态与成功发布对应，而不是只换一个字符串。
- receipt标16次CLI/1.111s是最后runs目录，不是全轮累计。目录含41份带exit与seconds的运行日志，38种不同字节hash；全部日志记录耗时合计5.323s。即使仅取38份不同日志，也已超本轮19次累计上限，不能仅报最终成功批次16/19。须准确列每次独立执行与复制日志的关系，不能未经证据去重成16。
- 本轮报告自己写cargo约20次，超过追加≤16；旧G19写约22，两项相加约42，不支持“未超合并40”声明。没有完整可复核命令账本时写unknown/估计，不自行倒填精确值。本轮不回溯扩大额度，也不把检查员成本混到worker内。
- step04中stair-inside-solid-obstacle标geometryIdenticalToProbe=true，但机器差异显示两梯support.columns由2→1、columnSizeM由.3→.2。正文披露确有修改；表的true应降为false。输入最初来自worker旧成功fixture，不能把它的既有问题归咎为检查员发明。修正旧fixture本身是允许的，不能静默移走待测墙或改变反例性质。

## 下一轮应如何收口

这是正确性修复，不扩展机场/建筑/桥型等新需求，不接入真实world。下一包以统一的几何与图约束为对象，而不是只使名单上的案例绿色：
1. 端口到真实面、能力到真实走行体积、路径到实际有向折线三者一一绑定；
2. 从相同权威输入计算每能力事实，验证侧独立重采/重算；
3. 构造器、验证器、CLI与版本控制内测试同时成立；
4. 以平移/旋转/反向/宽度边界/障碍位置/分支顺序等有限变体检查不变量，保留合法场景；
5. 新一轮必须单独明确人工运行累计上限，原剩余额度不能继续使用（已超）；自然额度仍0。

不要求单独确认或旧包勘误，不覆盖原证据，不为补日志重跑自然。要继续时一次授权并完成完整修复包，尚未签收前不进入真实ST18与默认世界消费者集成。