# G27B-R1独立裁定与G28开工（2026-09-23）

## 裁定

**R1证据闭集修复通过。G27B按“保存场景上下文的只读真实3D/WebGL消费者”限定范围验收收口（accepted_scoped_with_known_ui_issues）。** 这不是工程安全、真实城市、ST18、默认世界或路线可达验收，也不是整体B阶段完成；A冻结、B整体partial、C不推进。

两个用户可见缺陷（HUD计数标签、选择面板未填充）保留为已知问题，转入G28的首批必修，不写成G27B已修复。G27B红测试原始输出缺失、曾短暂写源根并复原、continuation-2封存后改写等历史披露保留，不因R1通过而清零。

## 本轮独立核验

- 按manifest的scope + 精确相对路径解析，不按文件名排除。114/114条bytes/hash匹配，无重复、无列入/排除冲突、无非法路径、无漏列。
- continuation-3磁盘110文件，正常列入110、排除0；嵌套packages/clean-3d/manifest.json已正常列入。
- R1磁盘7文件，列入4；根manifest/receipt/review按精确scope/path排除3。
- receipt绑定manifest、四份报告及两份独立检查/任务引用成立。
- 原continuation-3的106条绑定仍匹配，原manifest hash与上次检查相同，原receipt/review hash与R1引用相同；产品源码/最终exe与上次独立检查绑定相同；15项保护文件与上次检查相同。
- 源码context_preview3d.rs：181865 B / 1df9286d9cbaddf9d216fc7d2851daef2359486f8a018e891c7b15b70f31ee6e。
- exe：42252800 B / 64f72697841fc10de01ce56685625541ac438593190b075dd08121f7ad59bbca。

本轮未运行cargo、生产CLI或浏览器。前轮检查员86/82项测试与worker21项CLI/五类WebGL观察继续按原来源引用，不说成本轮新运行。文件绑定一致证明可核对内容未漂移，不把事后hash检查扩大为可证明历史上从未有过任何瞬时写入。

本检查初次临时脚本误把带scope条目全部按R1目录解析，遇ENOENT即停止，未产生通过判定、未改worker文件。随后正确按两个已声明根分别解析，持久化verify.mjs与verification.json保留方法和结果。不得静默跳过读不到的条目。

## 旧G24失配

独立复核G24 continuation-2 command-log.txt：清单1207 B/eaddfe61…，实际1351 B/442f553e…，与本轮披露及此前历史记录一致。该历史不一致不归责R1，不回改旧清单，也不宣称全部历史证据均0 stale。worker“1395项扫描”是其观察，本检查没有把它重新标为检查员全量扫描；检查员本轮核验的是R1闭集、既有保护绑定和该具体旧失配。

## 推进

前置闸门已满足，G28正式开工，不再要求第二次确认。任务书原草案已更新为生效版本：
G:/difyllmwiki/GlyphWeave/docs/collaboration/dsh-b01-g28-map-presentation-interaction-task.zh-CN.md。

G28独立上限：cargo8、CLI/服务进程8、浏览器6、辅助6、归档64MiB；不借用G27B余额。G27B余cargo1/CLI2/browser3/aux1只保留历史账，不转入G28。G28改产品后必须新建包并绑定最终源码/exe/资源，既有G27B包和证据冻结。

**G27B限定验收收口；G28授权执行，尚未实施/验收。**
