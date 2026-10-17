# G16A有限证书审计独立验收（2026-09-20）

## 裁定

**accepted-finite-certificate-audit / proof_attempt_closed / target_feasibility_unproven**。

接受这次有限审计完成、结论降级和主动停止；不再追加证明包，不要求DSH回一份“已记录裁定”。没有自然额度、新目标、构造预算或合同变更授权。A不变，B partial/C不推进。

## 独立核验（不是只读PASS日志）

检查员用独立Node脚本只读G12 north candidate GWHM，按生产f32高度读取/双线性/三带/采样间隔/1e-6拒绝阈值重算全部边。重新枚举包中声明的有限格点域和两个flat-top集合，得到652条，和产物集合逐条一致（无漏项/多项）。

重算分类：550 fixed_pair_certificate / 71 accepted_gate / 31 variable_or_unknown，652条分类0不符。逐条核对550证书的带偏移、采样索引、delta/limit和声明pinned邻格，0不符。

清单27/27、receipt的manifest绑定与2份产物绑定匹配；生产前缀81685字节/ba6deb959c3e3b913ed2c65a887f4860b1cddf505cfd2ae2fb71758997224d34保持。

证据目录：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g16a-20260920/（verify.cjs、evidence.json）。本检查员cargo/自然/真实route/terrain-carve合成全部0，源码未改；没有删除或覆写worker create_new产物来复跑Rust测试。

## 验收适用范围（直接生效，无需worker单独再修订）

1. **550**：在该候选模型下保持AB/BC保护值、道路p10 full profile及自然/zone不变时，列出的相邻采样对不能由允许的p2 profile-less候选改变，因此该边的坡度拒绝保持。有效的局部条件证书。
2. **71**：在当前保存面上通过了此次terrain-only坡度检测；不等于障碍/水/完整segment_clear已通过，不等于这些边可从建筑出口抵达，也不保证任何新候选仍保持它们通过。
3. **31**：没有找到本证书形式的固定拒绝见证。应叫“variable_or_unknown/未认证边”，不叫“31条已证明可被合法候选打开的边”。候选预算、组件约束、其它保护来源仍可能使它们无法改变；本审计没有证明存在这样的候选。
4. “652条穷尽”只针对已明确的有限2m格点域、从floor外进入floor内的8邻域边集合，不等同于生产所有goal接入段、直连尝试或连续路线的穷尽性证明。
5. A2/claim C1的“BC北墙不可改写”应限定为有证书的具体边，不能包揽coverage中仍含variable边的整组/端角。boundary字符串混合入边方向与目的区域，引用时以from/to与class为准。
6. “31条是原窗口成败的唯一开关”撤回扩大解释。它们是**本证书闭合失败的显式未决项**，不是完整可达性的充分必要条件。用此限定不需重写旧产物。
7. 124.12056仅作原列举门/goal族的条件算术；120.931236是同一组选定waypoints的欧氏和，非连续最优值。23块/唯一西端/全目标不可达撤回接受，不再追求把它们补成结论。

## 本线工作到此结束

这次证明尝试确已按要求结束：已获得固定局部障碍的可靠证书，但没有证明原目标可达或不可达。不得把“证明未闭合”读作需要继续枚举、增加自然运行或自动转西端。

用户今后若选择新的推进方向，应单独定义目标/唯一变量/信息收益/预算；当前不派G17，不再循环收尾文档。DSH沿用已发布汇报规范，当前结构已明显改善，只需遵守上述证据层级。