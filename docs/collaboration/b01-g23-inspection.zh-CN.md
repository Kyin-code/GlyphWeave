# B01-G23独立复核：3D消费链成立，必需资产完整性审核未闭合

日期：2026-09-21。

## 裁定

**接受三个工程包的实际3D消费、生产几何映射与重新导出一致性；G23整体暂为partial，previewCompleteness存在明确错误通过。**

本次不要求推翻viewer、工程审核、楼梯或G22。唯一功能阻断是：审核器只检查用户声明的文件是否存在，却没有独立检查当前预览格式必须具备的入口和依赖。另保留超支与台账数值不一致的流程事实，不因技术成果通过追认额度。

## 1. 独立复核与基线

证据目录：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g23-20260921/。

- worker清单339/339 SHA256+bytes匹配；manifest51527B /54726559df9c38bb35d9ad53e3655c02c479defceb0f4c2047ea436f7db844c0绑定正确。
- final-source-hashes.txt的19项新增/修改/冻结源码与输入逐条一致。
- G22主包94/94、入口收尾包54/54及各回执绑定保持匹配；三个冻结G22源码基线未改。
- 独立cargo test工程过滤共40项通过：32项engineering_connections、6项engineering_preview、过滤名同时命中的2项现有engineering相关测试。没有运行整个auto_roads历史写证据套件，不称整个库全绿。
- 重新构建最终CLI；检查员10次CLI：3个worker刷新后包各audit通过；3个从最终源码全新导出的包各export+audit通过；1个完整性负例却audit错误通过。
- JS单测42项通过，涵盖3个新鲜实际render-model。
- 浏览器独立驱动1次，真实Chrome/WebGL2/RTX3080Ti，3个新鲜导出包加载，外网请求0、pageErrors0；该服务只绑定127.0.0.1，由检查员驱动启动并在结束时关闭。

检查员只创建独立目录及本检查文件/状态记录，未修改产品代码、worker交付或已冻结证据。

## 2. 已接受的可见成果与两项披露的补证

### 2.1 三个生产几何场景确实进入浏览器

- 直跑15部件/180三角；折返29/348；terrace125/1500。
- 浏览器使用新鲜导出，无任何导出后文件刷新；10个声明文件逐项校验，页内parity通过。
- 查看了直跑实际点击截图与折返侧视截图，踏步、平台与支持柱可见；界面明确“浏览器未运行Rust工程审核”。
- terrace的自然/基线/工程面切换记录其各自hash；不要求三者必然互异。
- 重载后的对象计数与初次相同，不以普通对象总数误称只有一个Three对象；route显示可能包含线与节点标记。

### 2.2 最终源码重新导出链已由检查员补证

worker在CLI额度耗尽后手工刷新3份viewer资产，不能把那一步说成生产导出命令运行。检查员在全新目录用最终源码的CLI重新导出三个同源包，随后独立audit并实际浏览。

结果：每包10个声明payload文件与worker刷新后的文件逐字节相同，描述JSON解析后的内容也相同。故接受最终源码能够生产相同预览这一技术结果，不再要求worker为相同证据无意义追加3次导出。

历史记录仍需按原事实保留：worker导出时点早于viewer修改；本次新鲜端到端证据属于检查员，不倒写旧audit/运行日志。

### 2.3 真实鼠标命中已由检查员补证

worker的selectById不是raycast命中。本次检查员只用page.mouse.click点击画布，不调用selectById或pickAt，真实选中flight::tread3：

- role=tread，levelId=flight:flight；
- topMm=640、bottomMm=440；
- 命中pointM约(10.9462,0.6400,10.0788)；distanceM约13.058；
- 页面选中信息和真实截图保留。

记录112次有界网格鼠标尝试后命中（同一浏览器运行），不冒称第一次中心点击就成功。结果证明当前选取逻辑实际可用，未对所有视角/对象命中率作全面断言。

## 3. P1：同步删文件与声明后，缺入口预览仍四段全pass

### 复现步骤（仅检查员独立副本）

从最终生产CLI全新生成且audit通过的fresh-straight复制出一个负例：

1. 不复制preview/index.html；
2. 从engineering-preview.json的files数组中删去该文件条目；
3. 保留viewerEntry="preview/index.html"及complete状态，source/input、source/bundle、render-model和其它文件都原样；
4. 调用当前生产audit-engineering-preview。

完整负例路径：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g23-20260921/missing-entry-selfconsistent/。

实际audit结果（audit-missing-entry-selfconsistent.json）：

    status=pass，findingCount=0
    engineeringVerification=pass
    sourceBinding=pass
    renderParity=pass
    previewCompleteness=pass
    detail="9 declared file(s), 10 observed"

同一负例通过真实HTTP访问preview/：**404**，没有预览入口。

### 根因

G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/engineering_preview.rs约1213–1274行，完整性检查确认schema/exportStatus/viewerEntry字符串，并遍历description.files及observed差集，但未独立要求viewerEntry本身在声明/磁盘存在，也没有固定当前格式的必要依赖集合。

“描述文件自己不声明某个必需文件”使该文件缺失不再触发审核。自洽文件清单不是独立完整性规范。该问题影响本任务要求的previewCompleteness，不影响三个合法包已经真实显示的结论。

### 最小收尾方向

- 为当前支持的预览格式定义必须存在的入口、viewer模块、本地必要vendor依赖及源/模型文件/角色关系；不能仅信任待审包自报的files集合。
- viewerEntry/renderModel/source引用应在清单和实际文件中恰好一致；重复路径、角色错配、未知格式的处理明确。
- 至少加入两个负例：删入口+删声明；删关键模块/vendor依赖+删声明。必须在previewCompleteness拒绝；源工程和renderParity可继续通过，保持四段结论分离。
- 不必要求所有包的静态文件都永远等于当前二进制的资产hash；版本/资产集合规则应明确，不能把修复变成任意修改页面也算工程错误。当前格式无法识别的必要依赖关系就拒绝，不靠浏览器404后才发现。
- 保留现有合法三包、新鲜导出、工程篡改和渲染投影篡改证据。不要重写G22或整个viewer。

## 4. 流程与计数

worker披露前端驱动17/12、超支5，接受其透明披露，不批准事后越限。ledger-final.json仍写15/12、超支3；progress.budgets已写17/12、超支5，但knownIssues还有3-process旧口径。故ledger的PASS仅能说明它所收集33行记录一致，不能用来证明总预算合规。

33行分组为23条普通CLI、1条CLI preview服务、5次browser、4次local-service，合计CLI口径24成立；额外JS/probe进程需按最终披露17统一本包总账。下一次合法收尾报告引用该最终数字，不覆盖旧记录、不另开只改数字的确认包。

本次检查员新观察不能冲抵worker超支或给其恢复余额。检查员单列：cargo2、CLI10、JS驱动1、浏览器/本地服务一体驱动1；自然/ST18/bake/realize/默认世界0。3次export只是保存数据的显示mesh消费，不是新地形合成。

## 5. 验收边界与下一步

接受：三个人工包真实3D消费、最终源码可重复导出同一资产、真实鼠标命中、渲染数据数值测试、当前干净包四段audit通过、冻结G22不变。

暂不接受：previewCompleteness完整拒绝合同；worker所报“正分支已全闭合”；预算合规。G23整体partial，功能缺口集中在一处。

不再要求修相机、重做viewer、重新生成楼梯、扩大到SceneIndex/ST18。下一授权只应覆盖必要资产集合审核与成对回归、最终绑定；CLI24/24已尽且前端已超支，不在本次检查中自动追加运行。保持证据冻结，不发确认稿/勘误包。

A冻结、B partial、C不推进。G22原验收不撤回，长期真实城市接入仍需另批。
