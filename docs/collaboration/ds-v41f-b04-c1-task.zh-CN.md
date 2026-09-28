# V4.1 B04-C1：让现有形态profile真正驱动Rust生成

R6约定修复已独立验收。不要继续加固同一joint审核器或重跑原自然四档高程。回到已有B04任务的业务目标：目前morphologyProfile主要只resolve/validate，本轮把它接入实际生成。

## 1. 输入与基线

仅工作于G:/difyllmwiki/GlyphWeave，记录实际HEAD和R6源码快照，保留当前未提交改动。先读：
- docs/collaboration/b-review-r6-inspection-20260916.zh-CN.md
- docs/collaboration/ds-v41f-b04-task.zh-CN.md
- docs/collaboration/milestone-b-acceptance.zh-CN.md
- bevy/crates/core/src/settlement/morphology.rs
- 现有frontage/parcel/service/road/farm/residual程序的解析与合法范围。

不再重新阅读全部历史审查，不联网做新资料研究，不改A原输入/世界/runtime/证据，不动F盘。所有操作仍由单执行者完成，不启动agent，不Git提交/推送/切分支/清理。

## 2. 本轮核心交付

实现一个Rust确定性profile编译步骤，将原始用户意图与profile转换为现有生成程序可消费的effective programs，并真实接入生成与独立审核。

- 不创建第二套摆放算法，不让HTML按国家/profile发明建筑。
- 保留六个现有profile，所有字段有明确映射或unsupported/未接入标识，不能假装全部参数均生效。
- 首批实际连通至少：parcelInsets及建筑尺寸/高度默认值、已有道路层级参数、residual公共空间/牧草地参数、village田块模板。设施数量与需求保持显式，不凭profile凭空发明人口/学校数。
- profile默认参数只在原输入未明确给定时使用；用户显式尺寸、count、setback、预算、程序、保护/固定合同优先，非法显式值报错，不悄悄替换或缩小。
- profile范围与现有asset/偶数/坡度/退界等合同不相容时明确拒绝或报告unsupported；不得改宽验证范围让默认配置通过。
- 不能缺count时擅自分配数量；用明确的新控制输入提供16住宅+8店铺等需求，profile负责默认形态参数而不是减少需求。
- 同seed/profile/输入展开结果确定；请求顺序和新增无关请求不重排旧ID；不依赖HashMap遍历顺序或系统时间。
- 不配置profile时沿用legacy输入/布局语义；完整显式program与profile共存时显式值不变，不重复产生一批请求。

建议返回resolved/effective style与字段来源表：original-input/profile-default/derived/unsupported。记录profileId/version/caseIds和heuristic证据级别；数据是工程参数，不宣传成国家统一实测标准。

## 3. 生产与审核接线

在生成前只编译一次有效程序，所有road/frontage/service/farm/residual消费者使用同一套effective输入；不要各模块分别填不同默认值。

独立审核从**原始manifest+profile版本**重新展开，核对发布的effective内容及计划/实体；不能直接信任生成器写出的effective报告，也不能把manifest覆盖成已展开结果导致原需求丢失。

必要时给effective方案明确版本/source hash，新generator revision清楚记录。原始输入、展开结果与实际产物分别绑定。

Joint已经验收的工程函数直接复用，不在本轮改其支撑/来源/容差。原自然512m困难案例的profile关闭/原程序保持，仍未解，不能为了本轮CLI演示去改该冻结场景。

## 4. 测试与实际效果

### 定向编译测试

六profile至少覆盖：有效展开、无profile默认、错误类型/未知ID、显式值优先、冲突/越界拒绝、相同seed确定性、缺少必需数量拒绝、声明unsupported字段不伪装生效。

对compact-mixed-block、low-density-residential、village-field作同一明确需求/seed的对照：输出可解释的地块前后侧院、宽深/高度、道路/田块/开放空间参数差异。不能只比较profileId字符串。

### 真实函数集成

使用R6已有明确可行的测试输入/harness作为独立正例来源，分别通过profile编译的有效程序调用生产生成函数。若新形态导致布局容量不足必须显式缺额，不能削减需求。至少两个profile在一个真实可行控制上实际影响几何或田块/地表执行，不仅输出默认参数表。

审核负例：只改effective参数或profile版本、改已落盘来源、profile覆盖显式尺寸/删需求、重复请求、发布计划与原输入重新展开不符。工程与固定入口负例仍应保留。

### 小型CLI验证

在新的examples/morphology/b04-c1-*.manifest.json使用现有CLI能表达的输入；不要给CLI加“跳过审核/强制flat”入口。最多两次512m生成及对应独立quality/gate，每个120s/4096MiB，串行。

需要证明生成与审核均走同一编译规则，发布的字段/几何与effective输入一致。CLI控制有其他既有内容缺额时如实partial，不冒称原自然24完成或B整体通过。若当前CLI接口确有阻断，给具体复现/接口位置，函数集成结果与CLI未证实分开，不偷偷转成手写scene。

## 5. 修改白名单

生产：bevy/crates/core/src/settlement/morphology.rs；可新增同目录morphology_compile.rs；settlement.rs只导出；worldgen.rs只生成/验证编译入口与版本/有效程序传递；frontage.rs、services.rs、joint_frontage.rs、road_network.rs、farm_program.rs、residual_land.rs仅接收已编译输入的必要小改。

bevy/crates/cli/src/main.rs仅独立重编译/来源审核与报告接线；可新增bevy/crates/cli/tests/morphology_compile.rs或core/tests等价小集成。结果文档docs/collaboration/b04-c1-result.zh-CN.md；自己的work目录bevy/work/collaboration/ds-b04-c1/。

现有R6 joint/支撑checker、自然算法、资产/依赖/lock、HTML视觉/渲染规则只读。确有白名单外接口需要时说明最小理由，不顺手展开B05/C。

## 6. 交付与停止

交付字段映射/优先级表、源码增量hash、编译结果、至少两个profile实际函数集成对照、CLI新输出/审核、原输入与effective绑定、正负例及剩余unsupported字段。

单项定向测试120s，首次编译可独立300s，不跑全workspace/core、release、最大图、240块浏览器。不重新收集R6证据或复制同一平地包；引用原件与本轮实际增量即可。

状态ready_for_review或partial。服务路径距离、完整研究来源、多场景最大图等仍按B04/B05后续需求，不因本任务完成自动签收B。完成停止等检查员，不Git写操作、不启动agent、不改A冻结材料/F盘。
