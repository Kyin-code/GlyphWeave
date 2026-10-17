# G-1b step2b 独立合成验收（2026-09-19）

## 裁定

实质修复已完成大部分，10项现有测试独立通过。关闭已修的raw/support分列、top-priority内target与并列/自然下限/分支区分、场景外及非法输入拒绝、两侧几何对照、发布晚过期路径等检查项。**仅留一个局部阻断：blend归属列表与擦除标签仍把完全覆盖贡献混入。** 整体状态 partial_acceptance_pending_blend_provenance_fix；不签发自然额度。

## 实测核验

- 清单9/9、receipt6/6匹配，payload是保存日志原始字节子串。
- auto_roads生产前缀81685字节，SHA256 ba6deb959c3e3b913ed2c65a887f4860b1cddf505cfd2ae2fb71758997224d34未变。
- 检查员一次no-run编译（0.27s），随后当前二进制对9项命名回归及总验收分别--exact执行，共10项全部PASS。无自然、冻结世界carve回放、realize/bake；未修改源码。
- step1e清单11项仅源码绑定因授权后续编辑失配，其余匹配；旧回执不倒写的处理正确。
- 检查员证据目录：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g1b-step2b-20260919/。

## 唯一阻断及现成证据

源码 G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/worldgen/auto_roads.rs:10338-10350：

先处理full_cover和blend两个分支，随后只要terrain增加就执行blend_applied.push(index)。因此完整覆盖填方也进入blendAppliedDuringLoop。

无需另造世界：现有step2b-acceptance-payload.json的cells[0].after.decision.blendAppliedDuringLoop就包含owner=g1b.old.support、component=legacy_high_fill、outsideM=-4、targetMm=8000。outside<=0明确是完全覆盖，不是环内blend。

同一decision的blendErasedByOverride仅以branch_active && terrain_after_loop > full_target判定。该例8000→2390主要是丢弃旧完全覆盖高台；标true只证明循环值被覆盖，不能证明高于最终值的环填方被丢弃。字段需要按来源计算，或将泛化事实另命名为preOverrideValueReduced，不能仍借blend名称冒称原因。

复刻结果与生产逐毫米相等只校验最终数值，不能校验owner/component来源标签。这正是所有现有测试PASS但上述JSON仍自相矛盾的原因。

## 有限补完任务（不是新设计轮）

继续既有cfg(test)授权，不需要再次申请测试修复。仅补这一组来源字段：

1. blendAppliedDuringLoop只记录实际执行环内分支且抬升运行值的项；如需报告full抬升另列字段。每个条目都可验证0<outside<blend，且为blendPotential中的相同index。
2. 区分“循环末值被full分支降低”与“blend贡献被覆盖”。后者必须有真实环内贡献证据；明确记录数值/来源，不能仅依赖最终值变小。
3. 增加反例：只有完全覆盖高台被覆盖（可以有不高于最终值的普通道路blend），此时一般override降低=true，但不能判高环填方被抹掉。该反例可以直接基于现有plain fixture。
4. 保留真正ring-only高填方案例，断言blend来源非空/几何正确、环贡献被覆盖成立；再验证列表subset与来源标签。数值仍与生产比较。
5. 修完运行上述小回归和当前10项精确集合；独立新日志/清单/回执，不覆盖step2b。其余已接受项不再重做设计或追加单独说明轮。

冻结世界自然诊断仍0；不改G1b参数、不增加候选、不改生产、measure/access/final-terrain/realize/bake为0。修复通过后可结束工具合成验收，再单独决定固定四坐标局部自然实验。

## 边界提醒（不另设返工项）

现有采样入口接受显式finite f64并检查scene边界；未来接真实栅格时仍须先从生产读取Option/Result并传播缺格，不可由scene内判断替代真实数据存在性。命名回归实际为9项+总验收=10，不是10项再加总验收。worker两次调用使用g1b_preflight前缀过滤，应称限定集合的两次合成运行；本检查员独立采用10次--exact。日志确认集合仅这10项，没有自然入口。

B仍partial，C不推进，无真实增量；A不变。