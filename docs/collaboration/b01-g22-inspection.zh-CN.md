# B01-G22独立复核：固定预算反转已修，包装层无效输入还需收尾

日期：2026-09-20。

## 裁定

**原完整CLI预算耗尽反转的修复接受；裸地、楼梯和指定审核成对观察接受。G22的“所有失败入口安全拒绝”要求仍有两个helper/API包装边界未闭合，整体暂留partial，不推翻已通过主修复。**

不要再修改楼梯、扫描线求证器、空间筛选主干或安全阈值。仅需处理无效部件在校验前被层过滤丢掉、无有效路径时空区间被拒绝函数视为通过的两个分支。没有发现原远处构件CLI反转仍存在；下面新发现明确限于API/helper，不冒称完整CLI可绕过。

## 1. 绑定、测试、CLI和来源

检查员证据：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g22-20260920/。

- 清单94/94 bytes+SHA256符合，manifest24037B /3a47d5730954db7ab3f29630a776abd530f99a91fb94d23bc370457e56eeae09，与receipt绑定一致。该清单主要列包内文件，源码绑定另由入清单的progress/report及检查员实际hash核实，不把94条称作94条源码检查。
- core408565B /469f181e3dca5c16bf883b0d506bb3a1fad393a00e317a2f90fe4fd30bd875d9。
- support_coverage31719B /79cf0bf6dcaddfc6ab4c80f72193c9214cec2cdda81f473f43b96087f75bd30d。
- CLI源码99c90e46…，access/auto_roads保持前次绑定。
- 两个新增g22资源与检查员原始ground差分输入逐字节一致；不是移坐标后另换反例。
- 定向核心测试独立29/29通过。未运行全量auto_roads测试；旧证据写盘测试的既有失败不作为本包新增回归失败，也不据此声称整个仓库全绿。
- 重新构建最终CLI，独立17项基准观察全部符合预期，exitCode由检查员直接测量。见review-cli-ledger.json。
- 对G21收尾检查时绑定的74项旧包内证据重算，74项未变。三类合法结构的9个plan/elevation/mesh文件与此前检查员G21版本逐字节一致，保留其生成几何。

### 已独立复现的17项结果

1–2. 原ground窄承托和仅添加远处独立构件的差分均在ground_edge_unsupported_by_structure拒绝。
3–4. 正常裸地built/found、verified。
5–6. positive-terrace built/found、verified。
7–8. 最小直跑built/found、verified。
9–10. 最小折返built/found、verified。
11. 真正原偏移端点拒绝。
12. 墙穿梯段拒绝。
13–14. 保存身份/覆盖事实自洽篡改，payloadHashMatches=true、stored_capabilities_resampled失败。
15. 保存几何篡改由指定支持检查拒绝（还会触发其它相关几何/路线重查，不冒称“仅一项失败”）。
16. payloadHash篡改拒绝，payloadHashMatches=false。
17. 新目标路径final-write-sync实际到达同步原语前故障，无bundle/views残留。

worker最终台账17项status与各自report匹配；8项成功退出码明确标记derived，不能升级为原始测量。报告、stdout、bundle与路径自标识支撑成功语义，检查员本轮直接实测提供额外独立证据；旧两份错误汇总保留，不倒写为从未失败。

## 2. 主修复闭合范围

生产预算probe：同一有效窄面用event_budget=0，结果为Structure+Indeterminate /coverage_budget_exhausted；ground_refusal和structure_refusal均明确拒绝。

无效half_width=0的probe为Unknown+Indeterminate/invalid_frame，两种消费者均拒绝；正常无部件、正常有效路径为NoStructuralSupport+NotApplicable，ground支持门无需结构而structure门拒绝。

这与原来indeterminate被写死centreBorne=false的路径不同，认可身份/覆盖双轴及相关性筛选确实解决了已复现的主缺陷。

## 3. P2：top=NaN在有效性校验前被层过滤，仍误变为无结构

位置：G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/worldgen/engineering_connections.rs约3789行。

包装层先用abs(frame.top-level)<=tolerance筛选frames，再送纯求证器检查。top=NaN时比较为false，因此该frame没有机会进入invalid_frame检查，随后得到no_candidate_parts_proven。

检查员直接调用本次生产rlib的structural_support_detail_with_budget：

- 路径(6,1,8)→(10,1,8)，一个中心(8,0,8)、半长3、半宽0.2的MemberFrame，仅top改NaN。
- 返回NoStructuralSupport+NotApplicable，reason=no_candidate_parts_proven。
- ground_refusal=None。

无效高度不能证明它属于其它层，更不能证明当前层不存在结构。应在依据frame.top或其他几何字段进行层/空间筛选之前，完成包装入口的有限性检查，或明确把此情况传播成Unknown。

**范围限定：NaN无法用标准JSON数值直接表达，未证明当前完整builder/CLI会产生此frame。** 这是本包明确承诺的生产支持API无效输入契约未覆盖，并非对原固定CLI修复的否定。

## 4. P2：空路径/全退化路径仍在空区间上“无项可拒”

同一包装入口：

- polyline.len()<2约3750行直接返回初始NoStructuralSupport+NotApplicable；
- 全部段normalize_xz失败约3762行逐段continue，仍无interval；
- ground_refusal与structure_refusal仅遍历intervals，没有检查有效路径/全局unknown状态或空证书有效性。

实测：

| 输入 | 身份/覆盖 | intervals | ground_refusal | structure_refusal |
|---|---|---:|---|---|
| 空路径 | NoStructuralSupport/NotApplicable | 0 | None | None |
| 两点完全重合 | NoStructuralSupport/NotApplicable | 0 | None | None |

两种输入都不是“已经证明无需结构的有效路径”。修复可采用显式无效路径结果并令消费者先检查全局状态，或生成能被消费者读取的unknown证据。不能只改标签却继续返回空interval而被for循环跳过。

允许的重复顶点应有明确规范化策略，不能误拒合法楼梯；但空路径、全退化或非有限路径不得被当作已评估裸地。此处同样为API/helper级证据，不声称完整CLI已放行空路线。

## 5. 建议同包局部收尾，不另开算法重构

- 在层筛选之前处理无效frame.top/非有限路径数据；
- 区分有效裸地与根本没有可评估路径；两种消费者均读取失败状态；
- 补具名回归：top NaN/Infinity、空/单点/全退化路径、有效裸地及允许重复顶点正例。低预算失败、远处构件固定反例、合法楼梯与指定审核保持原语义；
- 不放宽CLI输入，不增加新设施/配置开关，不改碰撞或楼梯坐标；
- final工作树变更后绑定最终源码重测受影响入口，不能把检查员旧版17CLI无条件用于新生产改动。

G22已用cargo8/12、CLI17/24，按自报最多余cargo4、CLI7，漏计扣减；检查员消耗不冲抵、不增加worker额度。预算不足则partial，不追加确认包或自行超限。

## 6. 台账边界

独立probe/脚本进程额度针对驱动人工计算/生产库/CLI的执行，普通Get-Content/Get-FileHash等只读检查不因此被重算成生产probe。就本轮披露本身，不把约20次只读命令判作隐藏超支；编译、测试、CLI脚本和失败调用仍按原定义计数。历史G20超支1保持。

检查员成本：cargo2（定向test、最终CLI build），rustc1、生产API探针进程1、CLI17；无自然/ST18/bake/realize/默认集成，产品源码/测试资源修改0。

A冻结、B partial、C不推进。主缺陷已证修好，不把上述两个未闭合包装入口扩大成“楼梯或完整求证器需要重做”。
