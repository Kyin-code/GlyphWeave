# 转交V4.1：B首轮审查整改与小场景闭环

只操作 G:/difyllmwiki/GlyphWeave。先读同目录 b-first-inspection-20260916.zh-CN.md，保留原B结果与失败证据。B仍partial，A签收不变。

不启动其他agent、不提交/推送/切分支/清理Git、不改F盘、不修改A冻结产物。使用原B总任务书的功能约束；本轮优先完成以下确定问题，不扩展C、不重写验收框架。

## R1-S：先修来源和输入，再修道路选择

### S1 residual程序来源绑定

改动范围：bevy/crates/core/src/settlement/residual_land.rs、development.rs，bevy/crates/cli/src/main.rs的独立landcover审核接线及bevy/crates/cli/tests/baked_audit.rs或新增residual_source.rs。

从manifest.style与scene独立取得期望策略；与scene.landcover_plan及发布landcover-plan.json中的策略对应，缺省/字段/scene/version一致，不能clone被审核对象作为来源。

正例：原0.1/0.2与缺省无程序。负例：改0/0、删除程序、增加不存在scene程序、错version、scene与单独文件同时改但manifest不改；要求明确拒绝。检查员source-probe只复现API漏检，你需补到独立CLI审核链，不把“probe返回错误”当全链完成。

### S2 morphology类型

修改morphology.rs与定向测试：字段缺省允许；存在但number/object/null/array等类型非法明确拒绝；未知字符串拒绝，合法字符串保持现有行为。暂不伪称profile已展开，生产接线未做前须在新结果文档明确。

### S3 道路选择

修改auto_roads.rs与定向测试：将获胜priority/purpose/tier作为一致的选择；同等优先级稳定tie-break；tier不能绕过purposes限制，缺可用tier时显式拒绝/无候选，不降级成不允许用途。

先构造同一候选同时使urban/rural受益的红测；优先urban必须选择与urban一致且允许的tier。另测purposes不包含目标、tier只有collector/local且遇rural末端、请求顺序变化稳定、旧v1/8m行为保持。解释每个fallback是否配置授权，不能静默fallback。

保存阶段回执后继续下面业务小闭环，不需逐步等待用户，但任何工程错误未解决不能跑下游大图。

## R1-C：完成24目标的真实CLI小闭环

沿用B01/B02白名单：frontage.rs、road_network.rs、auto_roads.rs、必要road_profile/road_extensions/parcel_geometry/development接线与其测试；可新增bevy/crates/cli/tests/b_content_control.rs和examples/b-r1-control.manifest.json。其余生产范围若必要先写具体理由，不顺手扩大。

当前真实结果：B01 5住宅/0店铺，B02 4住宅/0店铺；zone unserved=0却20 frontage缺额。不能只说“道路供给不够”。

必须产出可复算诊断表：每条profiled road的用途相交、可放完整20×26parcel的连续前沿长度、合法station数、既有site/设施/保护排除、住宅占用后店铺剩余机会。明确是道路不在urban、站点遍历、parcel占地、请求竞争还是工程高度造成，不凭猜测换seed。

在原seed/用途目标/精确尺寸与16住宅+8店铺需求下，用允许的路网/候选分配改进建立真实供给；保留旧最低设施和固定入口。若当前明确输入在原预算下不可实现，给容量/几何证据及最小需求冲突，不降低count/放宽阈值。可以另附显式平地内存fixture证明算法，但不能替代CLI要求。

成功证据必须同时包含：
- CLI requested/planned/realizedWithAccess=24/24/24，unfilled=0；
- 24个完整parcel面积/方向/来源匹配，庭院实际surface核对而非仅plan，新增必需庭院缺额=0；
- 至少collector+local两级实际road拓扑及正确8/12/16m宽度链（以本轮冻结配置为准），不能只给单元测试某一种width；
- 自然源/保护高度/旧实体固定入口不变，几何/来源/支撑错误为0；
- 独立quality与相关audit通过；整个场景其他内容缺额单独保留，不能称B完成。

本轮至多2次新512m生成及对应独立审核/完整gate（各120s/4096MiB private，串行），使用当前源码构建的debug CLI并记录exe/input/source hash。不要覆盖原3次B01失败目录；不要再执行最大图、release、240块巡检或完整workspace。纯函数定向测试用于修复，保持小成本。

小闭环仍失败则交partial及原因，不用在B03/B04再堆未接线模块掩盖前序问题。小闭环通过后可按原完整B任务书继续依赖功能，但本轮的最大图仍不因“测试数量多”自动获得准入。

## R1-D：报告修正及B04后续欠账

新增 docs/collaboration/b-review-r1-result.zh-CN.md、本轮独立work目录与receipt，不覆盖原报告/回执：
- 更正workspace总数484（402是core），明确这是原日志复核还是新运行；
- 将B03“1km”更正为512×512，面积分母262144m²；
- B04当前只是profiles验证和默认关系表，没有实际profile展开/AccessGraph服务距离审核；
- 48条索引不等于48个已核实独立案例，先修us-houston/hk-mid-levels/jp-kyoto悬空引用，补具体来源URL或本地文献章节、可定位范围/证据等级；未核实保留pending；
- B04原village-field-2没有quality-report，不写成独立审核通过；
- 工程状态按本轮真实验证范围填写，未绑定策略/输入漏检未修前不得继续engineeringStatus=pass。

profile真正展开、服务关系路径检查、残余地块几何覆盖压缩仍按原B04/B03任务完成；不为合并网格先削弱占地判断。当前顶点occupied模型必须在换网格后用面覆盖测试验证，不能只保留单个“面积有增加”的测试。

最终回执给出实际改动hash、定向测试与CLI正负例、前后几何/数量/面积、运行次数和未解决项，状态ready_for_review或partial。执行者不签收B，等待检查员最终复验。
