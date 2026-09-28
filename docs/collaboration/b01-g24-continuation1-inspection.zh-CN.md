# B01-G24 continuation-1 独立检查（2026-09-21）

## 1. 裁定

**partial：认可实际核心实现与22项具名测试进展，不接受“核心已做实、只余audit/CLI”的收口判断。** 公开生产API探针仍检出漏实体、无效道路关系被接受、场景身份与来源字节分离等原合同缺口。不是要求重做已通过的跨块基本裁剪，不新增功能或重置预算。

本轮worker源码：engineering_context.rs 64568B / b1f4fd22dda80c0aabf0624db506f46c693c0b5d4fa4f415d6f18a587a6aead4；lib.rs 516B / a031f15cb29d725541309a887939f9a921abfce2e2873f0ef8d69a39239f1429。

## 2. 实际复核与接受项

- 检查员运行 cargo test -p glyphweave-core --offline --lib engineering_context -- --test-threads=4：**22/22通过**，591 filtered out。不是全库或原engineering_保护矩阵。
- worker continuation-1 清单9条 hash/bytes匹配；4次运行记录与三失败一成功日志留存，较上轮无失败材料有实质改善。未有最终产品回执，不把阶段清单当完成回执。
- 四个固定来源、十项既有保护代码/资产均匹配；前三份G24停点文件未变。G23R1/G23/G22四包87/339/94/54条全匹配，清单本身hash未变。
- 接受跨块逐样本唯一覆盖、缺样本/缺natural/重叠拒绝、双地形区分、负原点等已测进展；接受保存接入标注notReverified，不升格通行证明。
- 当前严格SceneIndex读取确实失败：missing field accessMethod at line 587598 column 9。检查员探针复现。此前“已有CLI读取代码就证明本固定来源可严格解析”的推断应撤回，包括立项依据中未经实跑确认的含义；代码存在不等于该输入成功。
- 空naturalTerrain产生missing field而非expected usize、两计数能产生相同usize错误的回归成立；旧原始fixture仍缺失，不补推断具体字段。

兼容读取可作为局部投影方向继续：八个声明未消费子树的移除、原严格错误与遗漏事实必须显式保留，不改变权威SceneIndex类型。它不是完整SceneIndex已验证，不允许把被省略的规划/保护信息解释为安全空集。后续补“消费子树仍非法必须拒绝”的负例，不扩大跳过名单以迁就失败。

## 3. 核心阻断（源码审查 + 独立公开API探针）

所有探针位于独立小型Rust消费者，调用当前glyphweave-core公开API。变体仅修改进程内SceneLoad/SourceBytes，原来源和worker源码均不写入。结果不是CLI验收，但足以反驳当前核心入口满足合同。

### P1-1 依赖原始字节的身份尚未绑定（from_scene 374–425，build_context 1044–1051、1168–1178）

SourceBytes::from_scene读取world/scene字节只计算hash，却依据调用者另外传入的SceneIndex选择载荷；不解析world的场景引用、不对SceneIndex与所读scene字节建立一致性。build_context继续接受独立SceneLoad与SourceBytes。

**实测**：保留原SourceBytes，只把SceneLoad.scene.scene_id改为unbound-scene，build_context仍成功，输出sceneId=unbound-scene，同时sourceBinding保留原a479b909…哈希。

修复：由同一受限读取流程从绑定原字节生成类型/兼容投影和载荷；或入口强制比对权威解析结果。内部已解析载体应封装，不能任意混配后标为来源已评估。校验world→scene路径/身份/域，元数据和实际读取载荷同源。新audit仍必须从显式根独立重建，不能只重复信任该混配API。

### P1-2 实体筛选漏项且事实丢失（footprint_of 823–867，footprint_intersects 870–879，EntityRecord/AccessRecord）

现逻辑优先盲信保存bounds，未与变换几何检查一致性；相交比较采用严格 < / >，排除边界接触，与任务闭几何域保守纳入不符。

**实测**（原固定窗口x=[256,320]）：
- wall中心x=254、宽8，真实未缩放轴向轮廓到x=258；保存bounds缩到[253,255]，实体被完全遗漏。
- wall中心x=254、宽4，轮廓恰到x=256，边界接触实体同样遗漏。

修复：原bounds与按生产变换约定的实体轮廓不一致时明确拒绝或保守并集纳入，不能信较小bounds漏障碍；闭边界接触须保留。明确scale作用方式并与源约定一致，不盲目重复应用scale。

此外EntityRecord丢失scale/grounding/anchors及完整原始bounds，AccessRecord只剩pathPoints计数、不保存原path，distance被毫米四舍五入，且把全场18条接入全带入而无局部引用关系。不能称完整保留原实体/接入事实。按原合同保存完整源事实，派生统计另列；对窗外引用明确referenceOnly，不用数量代替路径。

### P1-3 道路关系并未可靠交叉核验（plan_facts 898–967，build_context 1107–1127）

**实测三例均接受**：
1. 保留所有道路边但清空nodes；if !nodes.is_empty()直接跳过端点验证。
2. 命名道路实体改中心x=310、rotation=0，计划仍是原中心/rotation90；依然生成上下文。
3. 删除命名道路对应计划边，仍生成RoadRecord，仅planEdgePresent=false。

修复：有边就必须验证两端引用，即使nodes为空；拒绝重复节点与相关实体ID；缺计划边、源实体/道路图几何矛盾按任务返回inconsistent_source/unsupported，不能仅记录false后当成完成的真实道路绑定。保存节点坐标及必要referenceOnly事实，不只是数字ID。若需区分未建模道路类别，明确为不支持，不能默默绕过原指定道路合同。

### P1-4 仍全场读块、复制全场样本，不是有界局部载荷链（from_scene 406–424，crop_window 651–653，输出1219–1221）

from_scene无window参数，遍历所有descriptor读取并解码；单文件16MiB帽不是累计载荷帽。crop_window先校验所有块；EngineeringContext再clone所有chunk_sources完整毫米数组。required_file_set还硬编码scenes/scene-0。

**实测**：固定窗口的有效源增加一个位于x=10000的无关内存块，缺natural即导致局部裁剪拒绝。源码证明磁盘入口也会访问所有远块。

修复：先验证元数据、按窗口求交再读载荷；拒绝相交区域缺块而不是强迫所有远处载荷存在。保留块来源/样本映射即可，不将全场像素塞进局部上下文。加入总字节、块数/实体数上限；从真实scene路径生成必需集，不猜自然文件名或硬编码scene-0。

路径目前只有字符串规范性，fs::metadata/read会跟随链接；发布CLI前须按原任务实现解析后根内约束及reparse/symlink规则。此项是静态检查，未进行越根文件操作或声称已动态复现。

### P2-5 重新实现高度解码已出现与权威入口的差异（DecodedHeight::decode 160–235）

模块只导入HEADER/MAGIC，自写i16/i32解码和验证，并没有复用HeightPayload::parse。已有两个正常样例的parity测试不能证明完整合同等价。

**实测**：513×1的合法长度legacy字节，自写DecodedHeight接受，现有HeightPayload拒绝（支持上限512）。自写count*4等也没有完整checked运算。

修复：直接调用权威HeightPayload，再按encoding/raw进行无损毫米映射；保留元数据包装，不复制格式分支。公开crop_window自身也须有窗口尺寸/算术约束，不能只寄望build_context调用过validate。

## 4. 固定窗口不得更换

任务从一开始就是 **256,32,64,64**，不是0,0,64,64。本版测试改成260并报告“必须260”不成立。

检查员用同一最终源码、同一固定来源、原窗口256实测build_context成功，4096样本，命名道路generated.road-network.scene-0.4-11存在。恢复原指定窗口作为生产与fixture验收目标；260可留额外变体，不能替代。不要改原来源或历史报告，续作如实更正即可。

## 5. 检查状态命名不得混淆

当前terrainParity计算的是natural与final是否相等，不是“输出对原来源逐字段复算一致”；sourceBinding=evaluated仅说明计算过hash，也不证明场景对象与字节同源。比较双地形应命名terrainComparison等诊断；正式四段audit保持notEvaluated直到独立重建执行。没有必要提前给尚不存在的审核结论命名为已评估。

## 6. 后续执行顺序、预算与停点

同一G24、同一原合同，不立新包、不加额度：
1. 保留当前22项与三失败日志；新增上述公开API反例，先修来源绑定、漏实体和无效道路关系，不先把漏洞包装成CLI。
2. 将读取管线改为窗口相关、有限资源、复用权威解码；恢复固定窗口256。
3. 保留显式兼容投影的限制，补消费字段非法拒绝；完整源实体和接入事实不能再降为摘要。
4. 核心完成后再实现四段审核、两CLI和原矩阵；缺完成标记、同步删除、哈希自洽篡改、目标冲突等仍按原任务。

worker自报累计cargo8/12、CLI0/18、辅助0/6；后续最多余cargo4、CLI18、辅助6，不加回历史转入余额。新发现是原合同未达，不是新增范围。不保证4次足够；实施前排好核心验证、最终构建与CLI的调用预算，不足就保留可复核partial，不超支、不删失败测试。

检查员本轮成本：cargo2（一次22项复跑、一次外置Rust probe编译运行）、生产CLI0、浏览器0；只读核验脚本另列。未运行生成/自然/ST18/工程build/世界发布，未修改worker源码、固定世界和旧证据。外置probe只在检查员目录，编译缓存可更新，不是Git写操作。

## 7. 证据与状态

证据根：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g24-continuation1-20260921/。

bindings-before/after.json、core-tests.log、probe-results.json、probe-build.log及probe/src/main.rs。源码行号按本次b1f4fd22…版本。

**G24 partial：核心实现有实质进展但尚有正确性缺口，audit/CLI未实现。G22/G23限域接受不变；A冻结、B partial、C不推进。** 不要求补确认稿，不覆盖原记录。
