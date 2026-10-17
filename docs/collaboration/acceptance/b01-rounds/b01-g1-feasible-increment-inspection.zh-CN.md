# B01-G1 受控可行性实验独立审查（2026-09-18）

## 结论

**accepted-experiment（仅最终22状态负结果）；没有实现增量，B仍partial。**

最终实验可以复现，`bestLegalCandidate=null`属实。接受的是“列出的22状态没有找到候选地块”，不是“所有高程不可行”“平台唯一”“必须改工程政策”。G1实验关闭，不要求补跑同一组、扩大高程扫描或重新加固已验收审核链。

主检查员只读源码/证据、独立重放一次当前小harness、写文档。没有改业务代码、启动agent、Git写操作或F盘改动；bake0、正式世界0、冻结128扫描0、carve诊断0、80点时钟扫描0。

## 1. 独立证据

HEAD：`fb36dc4f9fdd38e6aa9c5a89676a65b96e5001b1`。

检查员输出：`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g1-20260918/`。

- receipt SHA256：`8f4e64861328f62c6a8a43023394d19d96e5f5b2894b60d3b07a6c38b9050cb9`，匹配用户前缀。
- 16项登记文件bytes/hash绑定匹配；W1登记7份世界文件未变；L1/R1/R2/R3/R4五份旧receipt hash保持。
- 原R2 near-miss记录有16项、16个不同start/end；排除所列2项后仍是14项，本次从中选择6项，不是“排除2项就只剩6条”。
- 源码当前生产行为变化仅为auto_roads::levels/SUPPLY_SAMPLE_CAP可见性；主要新增代码是frontage_supply.rs的测试harness，不是“总共只改两行代码”。road_alignment/road_network/worldgen/CLI的登记hash保持R4值。
- 独立命令：

```text
cargo test --manifest-path bevy/Cargo.toml -p glyphweave-core frozen_near_miss_corridors_try_a_second_legal_endpoint_plateau -- --ignored --exact worldgen::frontage_supply::tests::frozen_near_miss_corridors_try_a_second_legal_endpoint_plateau --nocapture --test-threads=1
```

**1 passed，11.70s（含基线构建/实现），候选循环3.6565871s。**

- 独立replay.json与执行者feasibility-run.json：全部rows相等，删去seconds后整个JSON相等。
- 22状态；8 design_refused；14 measured；14个predictedTotal全部0；fullRealizations=0；bestLegalCandidate=null。
- 六个12m live controls与R2的首拒绝/站点次数一一匹配：c1/2各8支撑冲突、c3为20支撑冲突、c4/5各8占用冲突、c6为8自然/填方拒绝。

此结果不新增任何“完整realize成功/失败”证书，因为本轮0个候选进入该阶段。已有审核链仍引用先前验收，不冒充本轮重新覆盖。

## 2. 固定输入/控制组审查

当前harness从35边基线复制候选，不写回原plan；需求24、义务21有断言。c3的end(288,96)按node_levels保留8000mm，只变宽度，最终没有违规修改既有端点。其余5条自由端点每宽度比较infer_end与起点8000mm。

每次调用生产infer_end/append_one，再复用frontage测量，未复制弱化摆放算法。没有改变地形/zone/需求/setback/fill/坡度。与live自动选路并非全部程序逐行一致（例如harness直接调用append_one，不走外层完整候选排序/clear），所以“忠实”应指上述控制观测与生产叶子，而非全球规划语义等价。所有状态结果为零，不因此产生不合法供给信用。

## 3. 必须收窄的解释（不要求重新跑实验）

### 3.1 不能由两个采样值推出唯一平台

12m控制与备选：

| 走廊 | infer_end控制mm | 备选mm | 备选结果 |
|---|---:|---:|---|
| c1 |10672|8000|设计拒绝|
| c2 |4160|8000|设计拒绝|
| c3 |8000固定|不允许替代|只变宽度|
| c4 |4160|8000|设计拒绝|
| c5 |4160|8000|设计拒绝|
| c6 |5672|8000|工程可设计，测量仍0|

c1备选实际上是**降低**端点，不是报告所谓统一“抬到路口高程”。c2/c4/c5测试了4160与8000两点，中间整数毫米高程未枚举；未证明可行集合是单点。`infer_end`求最小工程可行平台，也不自动证明该平台是地块供给最优。

可以写“这22状态没有支持本次增量假设”；不能写“唯一可行平台”“无端点高程能修复这些阻断”。本次也没有证明其它高度一定有效，保持未知即可，禁止为了修一句措辞再大扫高度。

### 3.2 首拒绝不是全部原因或严格优劣排序

rows只记录首拒绝，不是所有站点的完整分布。当前harness的wrong_purpose_or_protected来自frontage::envelope_reason对**完整parcel用途覆盖**的检查；不能直接解释成“道路支撑足迹侵入保护区”。16m影响道路边缘到建筑的偏移、候选站点等；根因需坐标和zone交集才能细分。

12m、16m都是0增量，因此本实验只能说“未改善”，不能仅凭首拒绝标签换了就量化为“严格更差”。

### 3.3 实验不能证明必须改变carve政策

existing_support_changed是真实生产拒绝，但本轮没有拆解各站完整贡献，也未证明所有合法布局都触发。不能据此宣布必须引入版本化支撑政策；它只能作为一个备选设计方向，且须另外验证不破坏既有路面/入口/院落。全局max合成仍未授权。

## 4. 预注册与预算披露

- 当前candidate-table.json包含corrections/runs，harness中的6条走廊是硬编码，不从表读取；最终表与代码一致，但表本身不自动约束执行。
- 目录中仅有最终feasibility-run.json/log，前两次原始输出/初始表版本未单独归档，现有records只是披露说明。因此检查员不能独立确认“初始表在执行前写入且未覆盖”的历史时间顺序，也不能复验两次作废运行。这不否认最终结果可复现，不能将frozenBeforeExecution:true当作时间证明。
- 执行者披露3次运行，24+24+22=**70个实际尝试状态**（含作废版本），最终有效集22、最大单次24；合计约44.7s仅来自记录，本轮只独立测最终版本。
- 上轮要求harness一次，实际3次属于流程偏离；没有超180s披露并不抹去次数差异。旧材料不改写，下一文档明确总量。未来表/修订/run采用不可覆盖文件，并绑定表hash；无需建设新证据平台。
- 代码180s检查只在准备fullRealize时执行，不是循环所有阶段的硬截止；此轮实测约12s，未触顶，不声称已经测试了硬超时保证。

## 5. 下一阶段决策

不再重复G1。同一候选轴没有取得增量，下一步先做**有目标地块的两段连接/支路设计**，不直接实现生产策略，也不更改carve。

优先理由：它可以复用已验收的走廊设计/事务/实现链，并探索G1没有覆盖的几何位置；工程政策变化则会触及既有承诺和版本语义，风险更高。该选择是项目管理判断，不是本实验已经证明两段方案有效。

设计阶段最多3个路束提案，每束最多2条段，先从真实未被占用且用途允许的完整parcel窗口出发，再设计接入，而非随机加一层搜索。零直接供给的首段仅是未提交的中间状态，整束无增量则整体不提交。若无合理提案，明确阻断与决策请求，不自行改需求/旧义务/支撑规则。

任务：`G:/difyllmwiki/GlyphWeave/docs/collaboration/ds-v41f-b01-g2-design-task.zh-CN.md`。
