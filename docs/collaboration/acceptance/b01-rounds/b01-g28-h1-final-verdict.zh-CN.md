# G28-H1 独立裁定与后续推进

日期：2026-09-24（Asia/Shanghai，本机核对）。

## 1. 裁定分开记录

**产品：H1动态HUD同步修复通过；G28在固定保存场景的只读3D查看/选择/图层/相机基础体验范围内收口（accepted_scoped_with_process_exceptions）。**

**执行纪律：不通过。** H1超出浏览器/辅助执行限制，首个失败会话的四张截图被删除，累计aux还少算2次。这些记录永久保留，不因为产品有效而回写成预算合规或“失败证据完整”。

不再要求H2重复运行同一产品，也不要求做只有数字更改的勘误包。G27B限定验收不变；G28收口不表示地图美学、全城生成、路线安全或工程可施工已验收；B整体仍partial。

## 2. 本轮独立核验

证据：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g28-h1-20260924/。

1. H1 manifest 43/43 bytes/hash匹配、3项精确排除、无未列文件，嵌套包manifest正常列入；receipt关键报告绑定成立。
2. H1资源、源码和exe绑定：JS60097B/7eca3059…，Rust174147B/a4a3559e…，exe42313728B/646b4fb6…；新包viewer与源码相同，model922d404c…与冻结R1逐字节相同。
3. 独立重跑实际JS的VM Part A：26/26。使用worker已经修正的监听DOM/GL stub，移除全部浏览器部分，仅在检查员目录落盘；不把stub结果称为真实Chrome。
4. 独立CLI单测92/92。使用已绑定最终exe另跑一次真实audit：exit0、四段pass。本轮不重跑core和浏览器，不把前轮结果记为本轮新观察。
5. 比较R1冻结包JS与当前JS，terrainTopology、terrainIndicesFor、drawIndexedTriangles、drawSolidTriangles、surfacePickData、rayTriangle、pick函数片段逐字节相同；材质/光照/着色器块相同。变化限HUD刷新及其接线，无证据显示用重写地形算法掩盖HUD问题。
6. 旧G28 102条、R1 60条、G27B-R1 114条绑定仍匹配；15项保护文件未漂移。

## 3. 浏览器证据能否复用

可以接受本轮exec02的Part B作为浏览器证据，条件已满足：
- 最终hud-sync-record.partB与runs/h04-dynamic-driver/hud-sync-record-exec02.json的Part B逐项一致，非人工重写绿值；
- 两份subject记录绑定相同产品，7张截图仍逐项bytes/hash一致；截图索引sourceRecord hash成立；记录中35处截图引用全部匹配；
- 第3次执行只纠正VM测试夹具，未更改产品，也未产生第三次Chrome会话。

实际查看h1-natural和h1-layers-off截图：natural显示crop.naturalMm；关闭实体/道路/观测后HUD显示solid0、总7938、overlay hidden，保存事实计数仍为4/3/4/18。记录还包含恢复7/8022、三次重载、真实鼠标实体与地形命中、坏包当时截图与恢复。

因此不因同一次进程中的VM部分失败就自动抹掉有独立来源且正确绑定的Chrome部分；但该次进程失败、首次中止、超支与删图仍应原样记账。

## 4. 纪律与账务更正（本报告生效，旧文件不改）

H1本轮cargo2/CLI3/browser2/aux3，单轮browser超1、aux超2。

累计aux在H1预算中写carriedOver=8是错误：G28已核对6次＋R1已列4次＝10，再加H1 3次＝**13/11，超2**，不是11/11合规。R1日志还提到首次夹具失败未独立列调用，当前13是按已明确逐项记录的最低确定数，不推测额外次数；如worker可提供更多运行事实应据实补入后续账，而非减少13。

G28线已确认累计：
- cargo15/17；
- CLI20/20，但此前原G28发生的+1超支事件保留；
- browser13/12，+1；
- aux至少13/11，至少+2。

H1删除了首次中止会话四张截图：失效证据可以标记不可用于验收，不应删除。本报告不补造、不要求重拍历史状态、不声称失败截图已留存。

上一H1任务书也有协调方的问题：同时写“正式新增”和“若尚未确认”，并把cargo余量/新增可用量表述不一致。对此明确纠正：以后任务只保留一张当前可用额度表，不交给worker猜测；该文本缺陷不追溯授权browser/aux超支，也不取消已经观察到的产品修复。

## 5. 下一步

H1产品锁定，不再开HUD修复续包。发布G29“默认地图浏览模式与检查模式”任务，用同一场景、同一几何改用户首屏、信息层级与模式切换；不是新地图生成，也不直接开晕渲/等值线项目。

G:/difyllmwiki/GlyphWeave/docs/collaboration/dsh-b01-g29-map-first-view-task.zh-CN.md。

G29独立额度及严格停止规则由新任务明确，旧预算不借用、旧超支不抹平。从新任务开工即使用不可变run目录和失败留存，先VM预检驱动再启动浏览器；预算不足必须停，不以“为了交付”自行追加。

**A冻结；G27B限定验收保留；G28功能限定收口且执行纪律不合规永久留档；G29正式授权未实施；B整体partial，C不推进。**
