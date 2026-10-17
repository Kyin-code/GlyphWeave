# B01-SUPPLY-R2 独立审查（2026-09-17）

## 结论：分别验收，不继续重开已修项

- **A 追加事务：accepted-subtask。** 整轮working copy、截止检查、Err阻止发布成立。
- **B 滚动承诺与结构化归属：accepted-subtask。** 原字符串归属已删除，占用和保护集合滚动维护。
- **C1 共享非递归道路审核：accepted-subtask。** 供给调用与生产verify共用审核体，补extension来源审核，旧窄road_support_status已删除。
- **C2 正式final来源接线、C3旧/新yard分类：needs-changes。** 两个具体数据接线问题，不是要求重写算法。整体仍partial，B未签收、C里程碑不推进。

本轮检查员无业务源码修改、无agent、无Git写操作、无F盘修改、无新bake、无冻结128扫描/carve诊断/浏览器。保留R2/W1/R6既有签收。

## 1. 身份与独立执行

HEAD：`fb36dc4f9fdd38e6aa9c5a89676a65b96e5001b1`。

检查员证据：`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-supply-r2-20260917/`。

- 当前receipt SHA256：`b3c39e92d7496b7ae2e2f9763f0fde9d972153d78acb64b481d42b7b3cf57390`（本轮用户未给预期hash，只报告读取值）。
- 7项changedFiles + 17项evidence，共24项bytes/hash匹配。
- W1登记7文件未变；旧receipt分别仍为`69ff2c9bae77411e92f5877dfe086557995894f77d899ea30c5a0eb2f05acfac`、`d84a59bee0d89765db3756ac78b1f4e306f9216a47c8361afba31c7e9a0f50f5`。
- R6 hash保持`fb5dd93504e29acd609d07643e9276081a30dd536f7a2ae4ec7fa06027baaa34`。
- 独立auto_roads：18 passed，64.26s，包含0..80时钟扫描、归属、两轮身份、真实破坏回退。
- 独立shared_final_support_leaf_is_the_production_audit_body筛选：harness报告2 passed，5.31s，**其实同一函数被连续两个#[test]注册两遍**，不算两个不同场景。
- 独立settlement::landcover：12 passed，0.20s。
- 命令使用`cargo test --manifest-path bevy/Cargo.toml -p glyphweave-core <上述filter> -- --test-threads=1`，日志分别auto.log/leaf.log/landcover.log。
- 只读yard-probe直接调用当前公开production development.plan、landcover.realize/verify，读取旧W1的实际final，未重烘焙；见yard-probe.json。

## 2. 本轮可以关闭的修订

A：append_supply_aware建立working clone，只有Complete才写回；WallClockAborted返回Err。实现后及最终写回前均有截止检查。现有扫描覆盖出现成功后丢弃；wrapper结构同时证明任何Err都不能写回working。测试去除report字段比较，不代表原始report字节也被测试逐字节断言；但wrapper的无写回结构保留它。不要再要求全跑80轮加固。

B：SupplySession保存已实现site、归一化几何与对应report；测量用session.occupied；adopt先校验来源，再替换状态，身份守卫读rolling commitments。此前“第一轮地块在第二轮不受保护”“scene名被当request”问题已收口。

C1：audit_final_support_for_record复用verify_impl的同一支撑/entity循环，没有复制另一套弱阈值；不重规划避免递归。供给另调用audit_extension_provenance和固定节点合同。保留此设计。其调用者仍承担record来源可信，不应当作任意上传plan的独立来源证书。

## 3. P1：正式供给入口还在传seed=0，诊断用42

### 直接源码证据

- auto_roads.rs:1105附近，baseline实现调用realize_candidate(...,&baseline_plan, **0**, ...)。
- auto_roads.rs:1465附近，候选实现调用同样传 **0**。
- frontage_supply.rs:1452的独立baseline诊断则传真实`seed`；冻结W1 manifest为42、seedOffset0。
- frontage_supply.rs:310的final_raster_source比较17个探针样本与传入seed的解析地形，匹配则production-encoder，否则natural-carve。

因此“诊断finalSource=production-encoder”**不能证明实际v3供给入口已走同一seed/编码路径**。当前生产传0无法表示一般用户seed、scene.seedOffset；这也是它继续支持自然栅格fallback而小测试仍能通过的原因。本轮没有重跑完整v3证明实际每点差异，结论限于真实调用参数不一致及未证明的等价性。

### 来源不能靠17个点猜

“生产天然源/合成fixture”的区分应来自可信生成上下文，不是抽样相似。只改采样点之外一个格，判定仍可能把传入自然源当成seed解析场并重新生成覆盖它。自然patch/外部raster或将来的其它合法源也不能被猜作测试数据。

要把实际seed（正式规则为world.seed XOR scene.seedOffset）、源类型、origin/精度/权威自然来源传到真实供给入口，测试替代后端显式注入；缺上下文生产路径拒绝，不静默seed0/fallback。无需改自然算法或carve。

## 4. P1：旧21栋义务建筑被当作“新增地块”收费

### 分类不匹配

- realization.parcel_by_entity收集development.buildings的**全部建筑**。
- commit_decision判断旧/新的集合是session.commitments.keys，它只包含**已实现的frontage请求地块**。
- 当前W1有21栋既有roadside义务建筑，初始frontage0，所以session.commitments为空。

结果：旧21栋的yard被当作“新yard”，违背“旧13m²缺额保留，新地块必须无缺额”的合同。

### 独立复验

yard-probe使用旧W1的真实落盘final和当前production realize/verify，得到yardUnresolved=13，来自两个**旧地块**：

- `generated.roadside.scene-0.generated.road-network.scene-0.4-11.-1.parcel`：4m²。
- `generated.roadside.scene-0.generated.road-network.scene-0.20-27.1.parcel`：9m²。

再将当前原样commit_decision/realized_increments抽到检查员小型接口桩（不是完整bake），以这两个实际旧缺额作baseline，构造“旧缺额原样保留+新增一块yard缺额0、实现有入口”的验证结果：

**被拒：`realization_new_parcel_yard_unresolved:13`。**

详见yard-guard-probe.rs/json。第一次探针缺少realized_ids_by_request而命中无增量，补齐与真实报告一致的结构字段后才得到上述目标错误；未改生产代码。

这并不证明当前自然图存在这样一块合法新增地块，而是证明当前守卫会错误拒绝符合旧/新缺额规则的结果。该问题会在首次出现正供给时阻挡实际进展。

### 需要的分离

需求信用集合只含request真实实现地块；旧工程/yard保护集合应来自可信进入阶段的**全部既有parcel**，并加上每轮已承诺新增parcel。两者不能混用。

按每个旧parcel比较其缺额，不用旧地块之间的缺额总和抵消增长；每个新parcel必须0。原21义务不能因为补进保护集合就被计入16住宅+8店铺的新需求。

## 5. 已披露限制与文档问题

- 非yard unresolved_area仍用总量：本轮不宣称非yard的逐来源保护已完成，保留为后续土地实现合同限制；本轮不因此扩成全地表重构。
- no frozen128 rescan与落盘结果/receipt矛盾：结果§5承认一次，receipt.frozenAppendEvaluation.runs=1及carveDiagnosisRerun=1，却又写frozen128RescanPerformed=false。检查员未再扫描。应追加勘误和统一新回执口径，不能修改旧hash记录掩盖预算偏离。
- 两个不同名字的扰动证据flat_top_edge/transport_footprint实际坐标均(228,96)，失败文本相同；只能算同一点重复测试，不能说已分别证明平顶与过渡两类独立位置。共享生产审核本身不因此推翻，不要求新增大型矩阵。
- road_network共享叶子测试重复#[test]导致2次同名注册。移除多余属性并如实计数即可。

## 6. 下一步

只修C的两个数据流问题：实际自然生成上下文传递、完整旧parcel分类。A/B和共享审核体已签收，不重做80点时钟扫描、不重跑128自然扫描、不修改全局carve。

任务：`G:/difyllmwiki/GlyphWeave/docs/collaboration/ds-v41f-b01-supply-r3-task.zh-CN.md`。
