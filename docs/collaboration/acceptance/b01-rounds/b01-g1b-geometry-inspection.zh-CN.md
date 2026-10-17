# G-1b 离线构造检查审查（2026-09-19）

## 裁定

G-1a仍为空集阻断。G-1b **geometry_constructed / engineering_unverified**：已通过非空、连续、避开两个已知冲突点的完全覆盖这三项离线门检。它未获证明可行，也未获证明不可行。报告中的“允许变量下不存在可行构造”“只能mask或C-1”不接受，属于超出所交证据的族级结论。停止自然动作正确；R1已接受事项不重开。

## 为什么不能接受族级结论

上次任务明确几何通过不等于support/access通过；本次worker交出了满足几何门检的G-1b，却将另一个未经采样的工程判断当成几何阻断。

矩形必经保护格，只证明覆盖关系。生产 `G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/worldgen.rs:4482-4523` 中full_target还取决于natural floor、graded_influence、全部完全覆盖组件的priority与target；写入某值也只有在它不同于before时才构成支撑变化。G9两个整平带控制点就是“候选完全覆盖但既有更高priority主导、采样不变”的已有例子。这不证明新的肩部点也不变，只说明不能省略逐点证据。

对于G-1b，必经点可固定北(270,171)、南(270,188)。现提交JSON未给这些点的before/after、natural、决定性full组件与优先级。因此真实工程结果unknown；不能把x268的6502/6987搬用到x270。如果以后要证明整个“anchor到平带全程硬矩形覆盖”的子族不可能，需要证明一个该子族必经点在全部允许参数下必定不等值，并写明限制；不能扩展到其它连接语义。

## 独立完整性复核发现旧证据被覆盖

新receipt-g1b.json为3415字节，SHA256 `d18abc306bd62e2606c9729556689c971ebfccacabe76b4f12fb74abfdbbd743`，6项绑定匹配。

但旧receipt.json的6项中**1项失配**：
- 文件：`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g10-r1-g1-execution-spec/git-status.log`
- 旧：7891字节，SHA256 `a40cc06e7c086806831e496c4d487cddb092709d465f91fbf5cfa497583ff643`
- 当前：8024字节，SHA256 `8c2bdc00f410f25a947adc708e7ae1f7f926c11fe4a600923fbd6024ef56569d`

旧卡与旧receipt自身未改，不等于旧receipt的证据集合完整。不要倒写旧receipt，不要覆盖当前新receipt所绑定的日志来伪装修复。保留现状，新增说明准确承认覆盖；若无真实旧副本，旧正文可恢复性unknown。以后每轮独立目录或独立文件名，create_new。此项不要求重跑任何测试/自然动作。

## 给worker的明确回复

接受G1b三项几何检查；撤回族级不可行和mask/C-1穷尽性表述，工程状态记unverified。无需再次设计G-1c，也不继续以文案推演所有几何失败。新补充说明用新文件，旧卡、旧/新receipt及当前日志全部保留。说明旧git-status绑定失配，不谎称旧证据全未改。

**本次不追加自然额度。** 后续若推进，下一实验只应回答固定G-1b是否真的改变两个必经保护格，而不是直接做access/full-world/realization：

- 固定ST18-R1、G1b原参数，北/南分别在同一个不含候选建筑的道路基线上独立比较，不把两个候选串行累积。
- 核心观测北(270,171)、南(270,188)，旧x268两冲突点作为对照；输出natural、生产同口径before/after、完整carve身份/顺序、graded/full priority/target与分区夹限。若相等，不称完整support通过；若不等，只直接证该输入失败。
- proposed列表只能替换connection；pad/apron和生产合成公式不动，base与base+candidate分别处理。不得以生产candidate_final_terrain重建旧carves来验证新规则。
- 实现/合成验收与自然放行分开。自然诊断未获批前不得重建自然或回放carves；将来即使获批也只做单次局部点诊断，measure/access route/final terrain/baseline及candidate realize/bake全部0。不允许自动转C-1或新参数。

此处是后续实验范围建议，不是已经签发自然执行许可；无需为此重复本轮离线检查或补跑失败结论。

## 检查员成本/状态

本轮只读源码/JSON与哈希/几何算术，cargo/自然/carve/realize/bake全部0。机器结果在 `G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g1b-20260919/evidence.json`。HEAD仍fb36dc4f，staged为空。A维持，B partial，C不推进；无真实增量。