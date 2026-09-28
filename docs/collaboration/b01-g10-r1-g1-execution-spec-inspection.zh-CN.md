# G-1 执行规格卡审查与明确后续边界（2026-09-19）

## 裁定

**G-1a statically blocked**，不批准其实现或自然实验。不是等自然实验才能判定的 unknown，也不外推其它连接方案不可行。R1 已接受的合同纠正/profile绑定不重开。

## 独立核验

本卡 receipt 3578 字节/SHA256 `94bd6c7d4c714a5dbf6b618a63fd05ed2cdf9acbd7f2d8f9be728ed85c57e013`；6项绑定一致，前轮R1的9项绑定仍一致。直接读取G8存档station18两行parcel，做集合/矩形算术，未运行自然或carve。机器证据：`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g10-r1-g1-spec-20260919/evidence.json`。检查员本轮cargo/natural/carve/realize/bake均0。

## 阻断项

### P1：同一卡定义两种不同几何，其中交集定义已经为空

G8保存的北parcel `[262,148,278,166]`、南parcel `[262,194,278,212]`，与所写B_flat的z区间 `[173,187]` 均不相交，因此两侧 `P_own ∩ B_flat = ∅`。按卡自身规则已经必须停止，不能将其留待“实现时读取”。

若不执行这个mask，而只用cz/half矩形，则北硬覆盖为 `[268,163,272,173]`，南为 `[268,187,272,197]`。前者包含 `(268,171)`，后者包含 `(268,188)`。所谓“截到平带边界便不进入保护带”错误：从入口到平带必然经过所列肩部带；本矩形仍完全覆盖两个已知冲突点。这里仅作几何判定，不伪称重算了terrain数值。

不得将交集擅自换成并集算作同一G-1a；并集还存在parcel与路带间的空档，也不自动表达连续连接。

### P1：完整保护检查仍漏范围

规格只写 `C_conn ∩ old support`，遗漏候选pad/apron及其blend。虽然它们没有修改，但从base到base+candidate它们仍是新增贡献。应按生产frontage.rs:540-585的逻辑覆盖**全部proposed组件的硬覆盖+blend影响范围**与所有既有保护区的交集；既有实体从实际staged plan取得，不靠手写数量替代。

“与保护带的某子集不相交”也不能证明全保护等值；平带本身仍是受保护区域，blend仍可能作用于其它格。

### P1：最终栅格/realize没有消费G-1a的接口

`G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/worldgen/frontage_supply.rs:367-384` 的 candidate_final_terrain 接收plan，内部重新调用生产plan_terrain_carves；不接收替换后的carve列表。`:526-541` 的realize_candidate又调用candidate_final_terrain。

所以只在auto_roads测试中替换carves，不会自动贯穿生产final-terrain/realize。若有一次显式final-terrain调用，再有baseline/candidate realize，真实final-terrain调用也不止一次。不得拿旧规则栅格验证新规则，或不计内部调用而声称整轮上限1。两候选是分别验证还是一起提交也未定义。

### P2：失败分类仍无相应返回数据

access.rs的segment_clear是bool，route是Option；不能仅由false/None判定障碍vs水、120m或6000耗尽。“有依据”的文字不是实际计数接口。直线路段失败不等于有界绕行失败。保留生产状态，未观察细类必须unknown/not_instrumented。“预算未穷尽”与“耗尽停止”也应分清。

## 任务交代纠正

前次检查虽然要求唯一规则，但没有把**保存坐标先算集合非空**设为硬性前置门，也没有明确将诊断与realization拆阶段。这给worker留下了继续推迟读取、把不相容预算合并的空间。后续按下面一个小任务执行，不再循环润色G-1a。

## 可转发任务：离线构造性门检（不是自然实验）

1. 接受G-1a静态阻断，保留旧卡/receipt；无需重跑复现检查员已给的空集和矩形命中证据，不重开台账与R1已关闭事项。
2. 如继续设计，只提交**一个**不同ID的替代连接几何，北/南为同一规则的镜像，不做参数扫。只允许修改候选connection的矩形几何及blend；target2390、priority2、pad/apron、道路/建筑/parcel/入口位置、审核阈值保持。不得引入格点mask、事后恢复旧值或全局carve合成语义；如做不到，明确报这一受限设计尚无可构造方案并停止，不自动转C-1。
3. 先读存档坐标，交数值化矩形/影响范围，离线检查：非空、anchor至平带连接连续、硬覆盖是否包含已知冲突点。新方案若仍将两冲突点完全覆盖且target/合成规则不变，不作为解除已知机制的方案提交。通过这些门检仅表示几何自洽，不证明完整旧支撑/通行通过；实际未采样项继续unknown。
4. 列出全部候选组件（connection/pad/apron）的影响域，给完整旧support检查的域定义；不要把未变组件排除。这里只列定义/离线几何，禁止terrain合成。
5. 接口部分只设计**测试专用局部反事实诊断**：明确由哪个既有底层入口消费同一份proposed carves、产出的terrain如何供pad/support/access读取；指出必须新增的测试侧适配。不得调用生产candidate_final_terrain/realize然后假定它使用了替换carves；暂不设计真实增量签收。
6. 本次授权仅离线读取与算术：cargo/source edits/natural rebuild/design/append/infer/measure/carve replay/final terrain/baseline realize/candidate realize/bake全部0；离线脚本累计≤60s。输出一张替代几何卡和机器几何表（含输入路径/hash与门检结果）；无需再写历史综述或完整台账。做不出则输出明确阻断，不私自扩范围。
7. 只有此离线门检通过，主检查员才考虑另行批准测试侧实现和至多一次局部自然诊断；本任务不预批实现或自然额度。未来局部诊断的realize仍为0，诊断通过后再单独讨论如何与完整生产链保持规则一致。

## 项目位置

道路append/路口匹配与station18 no-cut已有通过证据；连接carve影响旧支撑的局部机制已清楚；替代生成规则尚未构造验证，access未通过，无真实增量。A维持，B partial，C不推进。