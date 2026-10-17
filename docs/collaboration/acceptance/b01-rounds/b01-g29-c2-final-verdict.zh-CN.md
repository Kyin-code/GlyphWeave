# G29-C2 独立裁定：产品限定收口，驱动失败保留

日期：2026-09-24。检查方式：只读磁盘核验、原始观察交叉核对、驱动控制流与截图检查；本轮未运行 cargo、产品 CLI、VM 或浏览器。不是 worker 自验证转述。

## 1. 裁定与适用域

**G29 产品：`accepted_scoped_with_harness_exceptions`。不再下达 G29-C3。**

限定接受同一保存模型的地图优先浏览/检查双模式、两种视口下的信息与控件可达、选择信息和既有渲染/拾取不退化。承接 G29-R1 已核查的生产者版本、包审核、坏包拒绝/恢复证据；本轮只补裁定交互证据缺口，不重新声称执行了原矩阵。

**C2 运行仍为 `partial_stop_allowance_exhausted`、exit 1、101 pass / 11 fail。** C1 仍为 96 pass / 6 fail / 1 notApplicable。没有拼成“一次全绿”，没有改写任何旧报告、回执或失败。C2 驱动的状态隔离缺陷仍未修复/未验收。原任务要求的一次完整全绿及 VM39/39 也未达成；本裁定明确采用版本相同、条件匹配的已有产品观察作为限定验收依据，不把合同偏差追溯改成合规。

A 冻结；B 总体 partial；C 不推进。本裁定不是工程安全、真实城市、默认世界或全城可达结论。

## 2. 独立核验结果

机器证据根：`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g29-c2-20260924/`。

- C1 manifest **58** 条、C2 **39** 条：逐项 bytes/hash 匹配；精确相对路径检查无漏列/重复；receipt 关键报告及 review 的 manifest 绑定一致。
- C1 **13** 张、C2 **16** 张截图：磁盘、截图索引、原始 browser record 逐张匹配，不仅比较索引自身。
- 两轮 preflight 的六文件包完全一致，且当前磁盘相符：model `922d404c…`、viewer `6812141c…`、HTML `0e087ca7…`；生产者 `g29-map-first-view-1`。
- 当前 Rust `2018242f…`、JS `6812141c…`、exe `99fd458d…` 与两轮绑定相符。C2 驱动 `f66014b2…` 与 intent 的执行版本相符。
- 另只读校验 G29-R1 **91**、G29 **80**、H1 **43** 条清单，全部匹配；此处不扩大为所有历史目录均无漂移。
- 机器证据：`verification.json`、`baseline-verification.json`、`coverage-crosswalk.json`。后者保留每条原 fail 和采用的另一轮 actual，共 **17** 条映射，不覆盖原 verdict。

## 3. 为什么这是驱动状态串扰，而非产品按钮失效

C2 `runC2Browser` 先做 1280 inspect，展开 `#more-camera` 后未收起；回 browse 不等于重置该 details。960 inspect 的 summary 观察已记录 `details.open=true`，驱动却无条件再点击一次，随后记录 `expanded=false`。故后续五个不可见子按钮无法点击。C2 截图显示“更多视角”收起，正好支持该控制流，不是截图证明展开成功。

同一包的 C1 **960×640 / inspect** 原始观察则有：

| 行为 | 独立复核 actual |
| --- | --- |
| summary 展开 | open=true，截图可见五个按钮 |
| 五个子项可达 | details.open=true，未裁剪，rect 在视口内，中心命中自身 |
| 斜视 | top → oblique，yaw=.6、pitch=.7、distance=89.6 |
| 左/右转 | yaw 分别增加/减少 .36 rad |
| 放大/缩小 | distance 分别乘 exp(-.192) / exp(.192) |

五次点击均有不同的 before/after 快照、cameraMoves=1、无点击异常；数值由检查脚本重新计算，不只采纳 pass 标签。

反向核对：C1 的五个 summary-visible 与一个 fields-visible 缺口，在 C2 有真实 DOM 命中、details 状态、三行字段矩形与文本观察。因此产品证据互补成立；不声称 C2 的有问题序列已通过，也不将一次“能打开”解释成任意状态下盲点一次都会打开。

## 4. 新发现的报告偏差与执行问题

1. **C2 当轮 VM 是14/14；C1 p01 与 p02 的原始 vm-record 也都是14/14。** C1 报告的39/39、C2 分析文档残留39/39，以及“39项来自C1”的转述均无这两个运行记录支持，现予纠正。不得再引用为实际39项已执行。14项可引用，但不能声称满足原任务39项数量要求。
2. C2 receipt 部分说明字符串缺数字，以结构化记录和本裁定为准；不要求为修说明再创建一个 worker 补证轮。
3. 检查员首次核验脚本也误以为两轮 receipt 的源码绑定形状相同：C1为对象、C2为扁平 Sha256 字段，导致6条检查错误。已保存 `verify-attempt1.cjs`、`verification-attempt1.json` 等，再按真实 schema 修正，最终 errors=[]。未把检查器失败当产品问题。
4. 连续复制并扩写驱动、状态前提未显式建立、报告计数手写造成了多轮返工。没有证据判定 worker 主观“偷懒”；但测试工程质量确有问题。协调方此前反复下达“最后一次”小包也未改善流程，停止此模式。

## 5. 下一步与账务

G29 累计 cargo6/6、CLI9/9、browser5/5、aux11/11 不变；旧超支、截图覆盖、历史哈希例外全部保留。不追加 G29 额度。

发布独立 G30 地图读图辅助任务：先在同一实施包中修复可复用驱动的状态前提，再实现方向/图例/保存高程范围；不再只为旧按钮拍一轮截图。新预算与白名单详见：
`G:/difyllmwiki/GlyphWeave/docs/collaboration/dsh-b01-g30-map-reading-task.zh-CN.md`。

新任务替代旧 C2“不得进入G30”的后续限制，仅对新白名单生效；不追溯修改旧轮退出条件。G30 尚未执行。
