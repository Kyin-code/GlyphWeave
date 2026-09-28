# B01-G25 独立检查（2026-09-22）

## 1. 裁定

**G25 只读保存场景上下文预览消费者达到 scoped delivered_for_inspection；整体状态仍按历史口径为 partial。** 当前没有发现新的生产逻辑阻断，但浏览器驱动存在三项观察误报/未完成观察，且本包有意使用自包含 canvas 2D viewer，需在后续任务中明确接受边界。不宣布 G23 浏览器矩阵、ST18、默认世界或真实城市消费已完成。

## 2. 独立复核

### 源码、核心测试与清单

- CLI 单测独立复跑：**40 passed / 0 failed**，包含 G25 preview 单测及最终源码 export→audit 端到端测试。
- core `engineering_` 保护回归独立复跑：**82 passed / 0 failed / 545 filtered out**。
- G25 manifest 独立递归重算：**149/149** bytes+SHA-256 匹配，0 个未列文件。
- receipt 对 manifest、report、progress、browser-acceptance、command-log、read-only-verification 的绑定均匹配。
- G24 core、G23/G23R1/G22 冻结包和 G23 viewer 资产未漂移；worker 声明的 context_preview.rs、engineering_context.rs、main.rs 哈希均匹配。

### 实现口径

导出前重新验证 G24 context 的 marker/manifest 哈希、v3 schema 和 caller 指定 world root 的 `BoundScene::read + build_context`；natural/final 分层、实体/道路/access observation-only 语义均在源码和端到端测试中有对应事实。G25 没有生成 EngineeringInput、没有重新求路、没有把 `connected` 或 `walkable_route` 升格为 verified。

CLI 15 项矩阵的归属符合报告：正例四段通过；natural/final 篡改归 terrainParity；删实体/提升 observation 状态归 entityAndRoadParity；错误 scene/window 归 sourceBinding；缺 render model/未知版本/声明缩短归 previewCompleteness；报告/输出目录冲突与包内报告路径拒绝。

## 3. 浏览器证据边界

worker 记录 4 次浏览器运行，其中首次发现 viewer 内嵌模型解析缺陷并修复，后续三次记录了真实加载、双表面切换、实体选择、道路/节点/access 标签、重载稳定性和坏包拒绝。三项未通过检查的归因有合理证据：

1. JSON 对象键顺序造成深比较误报，不是坐标错误；
2. `not re-verified routes` 被禁止短语正则误判，不等于页面出现 verified route；
3. 画布取样点未落入40px拾取半径，列表/身份选择已验证，但真实画布点击仍未完成。

本次没有重新启动浏览器；因此接受这些为 worker 观察和未完成项，不把它们写成检查员本轮测量。G23 三场景浏览器矩阵不属于 G25 声明。

## 4. 非阻断但需后续明确

- viewer 采用自包含 canvas 2D，而非 G23 的 Three/vendor。它降低了 file://、ES module/CORS 依赖，但不提供 WebGL/3D 几何证明。若产品要求真实3D，这不是等价替代；若接受“可核对的只读2D上下文投影”，需在后续任务书中明确。
- 画布拾取半径和驱动取样策略仍未形成一次成功的真实鼠标命中记录；身份/列表选择不应冒充鼠标命中。
- 浏览器额度已用尽，不能在本包追加复跑。

## 5. 预算与停点

worker自报 cargo6/8、CLI16/16、浏览器4/4、辅助2/6；本轮检查员 cargo2，未运行生产CLI或浏览器，仅复跑测试和只读绑定。G24历史账本不倒写，旧G23浏览器证据不重跑、不改写。

G25实现可以冻结，后续应另发 G25R1 浏览器观察与 viewer 合同裁定任务，而不是继续修改已绑定的 G25 包。

## 6. 证据

检查员目录：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g25-20260922/。

关键文件：bindings.json、cli-tests.log、core-protection.log。worker原始browser-acceptance.json、browser.log及run records保持原状并只读引用。

**A冻结、B partial、C不推进。**
