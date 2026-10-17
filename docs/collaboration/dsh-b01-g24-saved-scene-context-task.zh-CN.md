# B01-G24：保存场景的只读局部工程上下文与独立来源重查

日期：2026-09-21。用户已授权继续推进。本文件为单一执行包任务，不是要求另交方案或确认稿。按本包执行、一次交付；范围或预算不足则准确 partial 停止，不自行追加、不派子代理。

## 0. 目标：从人工自报 roadId 推进到真实保存场景身份

G22、G23/G23R1 的限域验收保持，不再以追加预览小补丁代替产品推进。

本包只做一条新数据链：

    已保存 world.json + SceneIndex + 权威自然/最终高度字节
      → 显式选定的 64×64 m 局部窗口
      → 版本化只读 EngineeringContext（地形、原始实体、真实道路引用、既有接入事实及来源）
      → audit 从显式指定的原来源重读、重裁剪、重选择、逐字段比对

目标是让以后工程方案能绑定真实场景，而不是继续由人工输入的 roadReferences 字符串自证身份。这里“真实”指项目已保存的生成场景，不是测绘实城。任务不承诺现场施工安全或全城可达性。

**本包不输出可直接 build 的 EngineeringInput，不规划新楼梯，不将上下文当成可通行证明，不接默认世界消费者。** 原因是 SceneIndex 的实体、surface、水和保护信息不应未经合同转换就被伪装成齐备的障碍/承托环境。此包先把真实来源边界一次做对，避免把缺水/缺障碍误当空环境。

## 1. 固定来源与窗口（只读，不新生成）

指定唯一生产观察来源根：
G:/difyllmwiki/GlyphWeave/bevy/work/joint-r27-verified-baseline/

起步必须独立重算以下绑定，匹配后再用；不匹配不得偷偷换源、回滚或重生成：

| 相对来源根路径 | bytes | SHA-256 |
| --- | ---: | --- |
| world.json | 960 | b36c42c1ae4785bab49832243affd747ed73791ecd3a5bfae666b4b3fe845752 |
| scenes/scene-0/scene.json | 10629013 | a479b909a9099c8df1a3ddec1b2694afe24e1876fdebc27e1def79863a992d86 |
| scenes/scene-0/chunk-0-0.height.bin | 1048592 | beffbdf5876cc07e8556f7545f8458dc96aa66a37b8d368dde54389806dfaa2f |
| scenes/scene-0/chunk-0-0.natural.height.bin | 1048592 | 39a3142a14f64380e0a0008129c264ae2af5aaa23beb2130d762a84476ece45f |

场景 scene-0，原点(0,0)，512×512 m；本轮固定窗口 **x∈[256,320)、z∈[32,96)**，64×64个1m栅格样本，世界坐标不平移。边界接触实体采取保守纳入规则，报告说明闭几何域与半开样本索引的区别。

检查员只读确认存在：
- 道路 generated.road-network.scene-0.4-11，实体中心(288,64)、旋转90°、源bounds x=[284,292], z=[32,96]；必须保留旋转/原始尺寸，不重新解释为另一条道路。
- 接入记录 entityId=generated.roadside.scene-0.generated.road-network.scene-0.4-11.-1，anchorId=front，roadId 指向上述道路，保存折线(276,64)→(276.85,64)→(284,64)。这只是 saved observation，不是本包重新证成通行。

该来源不是 ST18，不得把它改称 ST18 实例。严禁为本包运行 generate/bake/realize、补 natural、重采样造地形或改源坐标。缺自然高度时返回明确 unsupported/missing_source，不许 natural=final 兜底。

## 2. 产品白名单与入口

建议新增：
- G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/engineering_context.rs（类型、读取后的纯提取、来源重查及测试）
- G:/difyllmwiki/GlyphWeave/bevy/crates/cli/src/engineering_context.rs（有限文件读取、命令和新目录写入）
- core lib.rs、CLI main.rs 仅模块注册/显式命令接线。

推荐命令（可在包内以同等含义定稿，不再开命名确认轮）：

    glyphweave export-engineering-context --world SOURCE_ROOT --scene scene-0 --window 256,32,64,64 --out NEW_DIR
    glyphweave audit-engineering-context --world SOURCE_ROOT --context DIR --report NEW_REPORT.json

审核来源根由调用方显式传入，不照待审包里的任意绝对路径读文件。SceneIndex 为只读输入，**不修改 SceneIndex schema 或默认构建链**。可直接复用现有 height_payload 解码、serde 与已有哈希工具；不复制一套高度格式，不造通用导入框架。

冻结：G22连接/求证器/CLI三文件，access.rs、auto_roads.rs、natural/engineering/baked_terrain/height_payload既有语义；G23 preview core/CLI、viewer/HTML/vendor/镜头/点击均不改。G23R1 core engineering_preview=2a3eae8d…、CLI52af51ae…；其验收报告为 G:/difyllmwiki/GlyphWeave/docs/collaboration/b01-g23r1-inspection.zh-CN.md。

不把上轮非阻断“未执行子段显示pass”顺手塞入本包修复。但新 audit 自己必须正确表示 notEvaluated，不能复制该错误状态默认值。

## 3. 上下文事实合同

### 3.1 来源、地形与精度

每个实际消费文件记录相对路径、角色、bytes、SHA-256；world/scene身份、格式/版本、场景域、chunk域和选窗必须交叉一致。已有来源内部哈希按其真实算法校验，区分内容SHA-256与既有语义hash，不强行当同一种值。

naturalMm 与 baselineMm 分别由 naturalHeightFile 与 heightFile 解码裁剪，row-major、明确世界原点/步距/宽深/单位。复用 HeightPayload：i32-mm原样保留；若支持legacy i16-quarter，须明确整数×250的无损换算。无有限/有效样本、截断、缺块、域/编码冲突都拒绝，不插值、不填0、不用worldY代替地形。

本观察源为1块，仍以小型内存测试覆盖跨块窗口、非零/负原点、边界索引和缺块。不要为测试制造整张512m以上世界，也不要要求全世界栅格进内存才裁小窗。

### 3.2 实体、道路与接入关系

- 按窗口相交保守选出**所有**相关实体，保留源完整实体事实（id、kind、position、rotation、scale、尺寸、bounds、grounding、anchors），不截短实体再假称原实体；特别不能只按中心点是否在窗内选。
- 可采用有向实体轮廓的保守包围筛选；若 bounds 与变换轮廓不一致，按明确保守规则纳入或拒绝，不缩小到漏障碍。未知角色不自动归为可走/无障碍。
- 保留来源索引或稳定ID，防同名合并、重复ID和引用错绑。为结果确定性排序，但不改变几何。
- 道路引用必须来自选中源道路实体与 road_network_plan 的真实边/节点关系；说明道路事实的来源层次。不得仅因 roadId 字符串存在于待审上下文就成立。
- 源 road plan 与实体存在矛盾时报告 unsupported/inconsistent_source，不替它修路。引用所需的窗外节点可作为 referenceOnly 保留，不能丢了端点还声称边完整。
- 相关 access_graph 记录保存原 status/connected/path，标注 **sourceObservationOnly / notReverified**；不升格为本包 route=found。相关 access_surfaces、工程操作、landmark等已保存记录可按明确规则携带，不能从它们凭标签授予承托/碰撞豁免。
- context 明确列出水、保护、实体碰撞体和通行层等事实是否只是原始来源记录、尚未转换或缺失。缺失状态不是[]的“已证为空”。禁止输出 safetyVerified=true / buildable=true。

### 3.3 输出与审核不相互自证

输出最少含版本化context、独立来源绑定和完成标记；明确格式必需集合、路径/角色/唯一性。create_new/新目录发布，失败不得留下可被audit判通过的完成包，冲突不得覆盖既存文件。

审核从**显式原来源**重建期望上下文，不只重算待审文件hash。至少独立报告：sourceBinding、terrainParity、entityAndRoadParity、contextCompleteness。未执行=notEvaluated；只有所有必需阶段实际pass才总pass。

自洽篡改（改context＋改自身声明hash）必须被对应parity拒绝；篡改来源绑定不能把另一场景无声变为同一来源。若调用者明确换了来源，则是新身份，不宣称可以识别无外部信任锚的任意“整体换源”。

路径拒绝穿越、绝对路径、越根reparse/symlink；只读源根不输出报告。预算失败、未知版本和未能完成的检查都不能当作空集成功。

## 4. 最小验收：一个真实窗口 + 足够小的反例

具名核心测试必须覆盖：
1. 两地形源不混同，逐样本整数一致；非零/负原点、边缘/跨块、两种已支持编码。
2. 缺自然面/缺块/坏长度/域错误/未知编码，不生成成功包。
3. 中心在窗外但边界交窗的旋转实体仍保留；靠近但不相交的远物件不影响结果；未知kind不被抹掉。
4. 源道路与实体一致、缺roadId、重复ID、失效from/to引用；保留sourceObservationOnly不变成新route。
5. 改一个自然样本/最终样本并重算context自身hash，terrainParity失败。
6. 删一个相交实体、改rotation、伪造roadId，各自hash自洽仍被entityAndRoadParity拒绝。
7. 缺完成标记/缺必需文件+删声明/重复条目/未知版本拒绝，未执行状态不能pass。
8. 发布冲突与失败清理；普通测试不往历史目录或本包证据目录默认写盘。

生产CLI最低12次：
- 固定真实窗口 export + audit（2）；导出一次同源同窗到另一新目录（1），比较语义与非时间型payload字节确定性，不把时间戳差异当几何漂移；
- 自洽自然样本、最终样本、漏实体、错roadId四例audit（4）；
- 缺完成标记、同步删除必需文件/声明两例audit（2）；
- 导出目标预存冲突（1）、越界窗口拒绝（1）、缺自然源受控派生fixture拒绝（1）。

反例制作只在新目录，优先小型来源fixture；不要为每个反例复制10MB SceneIndex。新source fixture绝不冒称固定生产来源。保留每次实际argv/退出码/原始流/report及hash，错误运行与重试计入成本。

本包无浏览器要求；交付表格列固定窗口自然/工程高程差、选中实体/道路/接入记录数量与来源ID，并给一个可审查局部JSON示例。这是数据桥接证据，不冒充3D消费观察或路径安全证明。

## 5. 预算（一次授权，禁止叠加历史余额）

G23R1 worker实际cargo3/6、CLI9/12，未用cargo3与CLI3一次性转入本包；本包新增cargo9、CLI15：
- cargo总上限 **12**（转入3+新增9）；
- CLI总上限 **18**（转入3+新增15），单次≤30s、累计≤300s；
- 辅助验证驱动 **6** 次新额度；旧辅助余额不转入。矩阵驱动与内部CLI分别记各自类别；纯读/hash核验不算验证驱动；
- 浏览器/WebGL/截图0；工程build/route新求解0；生成/自然/ST18/默认集成/bake/realize/世界发布0；
- 新归档≤64MiB。固定源只读引用＋hash绑定，不无意义复制整个世界；需要单份来源快照时须计入归档；
- 峰值资源、最大输入字节/实体数/样本数要有明确有界检查。最低须容纳本源10.6MB场景索引，窗口上限可定为128×128m（不得未经授权扩大观察域）。

首次核心设计与测试优先，再最终构建和矩阵；遇预算不足则partial列剩余项，不继续超支。G23历史前端17/12超5、G20超1永久保留，G23R1转出的余额不得旧包再用。检查员审查运行单列，不混进worker账。

## 6. 交付与停点

唯一新worker目录（先确认不存在）：
G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g24-saved-scene-context/

一次交付：源基线hash与只读来源事实 → 转换合同/调用链 → 产品代码/具名测试 → 最终CLI矩阵与自洽篡改 → 报告与progress → manifest → receipt → 只读核验。冻结G22/G23/G23R1及固定来源，前后核验消费源文件与保护代码hash；不倒写旧状态。

正分支只可写“一个固定已保存场景的局部上下文提取与来源重查已交检查”，不能写“真实工程已建成/楼梯接入/全城可走”。来源不兼容或关键语义无法无损/保守表达则partial/unsupported，报告实际矛盾，不换输入凑绿。

无需确认稿，不自动接下一消费者、不改默认命令、不自批里程碑。A冻结、B partial、C不推进。
