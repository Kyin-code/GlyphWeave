# B01-G27B continuation-1 独立检查（2026-09-23）

## 1. 裁定

**接受P1的关键来源信任漏洞已修复；G27B整体仍partial。** 这次不回到hold，不推翻已修的来源链，也不允许跳过仍未完成的3D包合同、几何/拾取和真实浏览器验证。

worker在cargo额度耗尽后停止、不使用陈旧二进制跑新矩阵的处理正确。不过续作不应“先最终构建，再修包合同/几何”：应完成剩余实现和测试后构建最终CLI。上次仅余3次cargo确实没有覆盖两个实现失败加全链验收的空间；本次任务合并全部剩余工作，并显式给出新增额度，不逐项开小包。

## 2. 独立证据

源码绑定：
- context_preview3d.rs：65543B / e12f4f9931a5cbe5ab24593405d3fd6d6b1a8c728ad11314e992042f881254fb。
- context_preview.rs：70225B / f37dea0b10e6d86f0a423aad7456ff0df0cf1bcd327f7a8c6cf9d65c6bfcebaf。

continuation-1清单7/7、0未列文件、receipt五项绑定匹配；三次日志包含编译失败/六项测试失败/59通过，保留有效。首轮G27B 85项与另外12组历史清单条目逐一匹配。首轮声明15项保护文件中只有context_preview.rs发生获准的可见性变更。

检查员仅在内存中移除新增pub(super)标记，得到70137B / 52a8eb0e4c9cc3c1bdad48cc7686d14b2f41bc5da3da79ca3d28a52faa32613a，恰等于G25冻结版本：证明G25函数逻辑未被本轮顺手改变。未将这份反向文本写入源码。

检查员独立运行：
1. CLI测试59/59；
2. core engineering_保护回归82/82、545 filtered out；
3. 当前源码CLI构建成功；
4. 随后6次生产CLI观察。

这是检查员成本，不计入worker预算，也不倒写worker“本轮未最终构建/未core回归”的记录。由于检查员已更新target，后续不得再笼统称target仍是旧版；DSH最终交付依然必须自行构建并绑定自己最终源码。

## 3. P1已证修复

读取路径为显式world/G24 context → G25 load_context_package/context_window/rebuild/build_model → 与传入v1模型逐字段比较。来源不再由报告totalPass决定。

本次6次CLI结果：

| 检查员用例 | 结果 |
| --- | --- |
| 合法world/context/v1、不提供audit trace，export | exit0 |
| 上述新3D包audit、不提供trace | 四段pass |
| 改v1 final样本+1,000,000毫米、同步模型/manifest/marker、提供伪造完整四段pass报告 | export exit1，明确surfaces.final.samples[0]与来源855不符 |
| 同一篡改v1，不提供trace | 同样拒绝，证明不是仅凭报告缺字段碰巧拒绝 |
| 正确来源+两字段伪报告 | 拒绝trace缺stage合同 |
| 正确来源+矛盾trace（totalPass=false、错误modelSha256、四pass外再加同名fail） | export仍成功，见下一节限定缺口 |

前三条拒绝都没有创建目标输出目录。原来源和worker交付均未修改，所有篡改仅限检查员新目录。

## 4. P1附带的trace校验仍有小缺口，不回退来源修复

load_inputs在检查trace时只统计(name,state=pass)数量；四个名称各一条pass外，再加一条同名fail仍能通过，且totalPass/modelSha256未核对。与“每阶段恰好一次”的自报不等价。

因为真实world/context重建已先执行，这**不是原来的来源绕过**，不应再说P1全未修。但应在后续包合同任务顺手修正：如果保留--context-audit，明确它是非可信留痕，且输入若声称是成功审核报告，就按其真实schema检查四阶段完整唯一、各状态pass、totalPass一致、模型hash一致、类型/大小有界。或把它降为明确的opaque attachment，不再宣称验证成功报告；不能混淆两种口径。本次续作选择前者，避免接口语义漂移。

另外当前按已知v1字段列表比较，没有拒绝额外根字段，read_v1_package仍只校验必需名字而未形成闭集。它们并入原§2包合同，不重新发明一个P1实现。

## 5. 未完成范围与顺序

worker对§2/§3未实现和browser0的披露准确。不得只做构建后就开始浏览器计数：坏脚本/遗漏声明可审计通过、无高度补固定Y、AABB线框/有向体块、nearest-sample拾取等前轮发现仍需落实。

正确顺序：
1. 保留P1正负例，完成3D格式/资源/内嵌模型和全部字段审核；
2. 完成真实有向几何、未知高度显式未定位、实际三角面拾取；
3. 实际WebGL2最小冒烟尽早进行，计入浏览器预算，用于发现着色器/坐标/渲染错误，不提前声明最终验收；
4. 完成具名单测/路径A保护回归，最终构建；
5. 用最终版跑CLI矩阵及五类浏览器场景；
6. 最后定稿递归清单/回执/只读核验。

不要求worker重跑旧轮证据，也不删原失败记录。可修改新增3D模块/新JS测试及必要可见性接线；既有消费者行为与旧证据继续冻结。

## 6. 推进与成本

用户要求推进，本次发布一个已授权的G27B continuation-2完整收尾任务，明确新增额度与累计上限，不再索要产品需求确认。

任务：G:/difyllmwiki/GlyphWeave/docs/collaboration/dsh-b01-g27b-complete-chain-task.zh-CN.md。

检查员本轮cargo3、生产CLI6、矩阵驱动1；浏览器/工程build/自然/ST18/默认世界0。未修改产品代码或worker证据；报告和任务书为本轮新协作文档。

证据：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g27b-continuation1-20260923/，bindings-before/after.json、cli-tests.log、core-protection.log、cli-build.log、cli-review.json、runs/。

**A冻结、B partial、C不推进。G27B授权继续生效，整体partial。**
