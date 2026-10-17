# B01-L1-S1 独立审查（2026-09-17）

## 结论

**S1 的冻结 W1 个案诊断可采信；S2 的生成实现与冻结基线求解 needs-changes。整体仍 partial，不能说“all work complete”。**

英语或压缩后的措辞不是主要问题：代码与评估对象有实质偏差。本轮保留有效诊断、共享frontage候选提取及显式v3入口，不推倒重来；不签收B、不推进C，不重开已验收R2/W1/R6。

检查员仅审查、执行小范围测试/只读复验和写文档；无业务源码修改、无子agent、无Git写操作、无F盘修改，无新generate-world/gate/浏览器/最大图。对两个ignored诊断先检查源码无文件写入，再运行并将stdout保存到检查员自己的新目录；未覆盖执行者证据。

## 1. 独立证据

HEAD：`fb36dc4f9fdd38e6aa9c5a89676a65b96e5001b1`。

检查员证据：`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-supply-20260917/`。

- 实际receipt SHA256：`69ff2c9bae77411e92f5877dfe086557995894f77d899ea30c5a0eb2f05acfac`。用户本轮未提供预期hash，故这是读取计算值，不说与用户hash匹配。
- 24项path/log/hash/bytes绑定匹配，W1冻结世界登记的7文件hash保持，本轮manifest副本与原件相同。
- 独立`worldgen::auto_roads`：12 passed，0.89s。
- 独立`worldgen::frontage::`：10 passed，1.12s。
- 独立只读S1 ignored诊断：1 passed，2.82s。
- 独立只读S2 ignored诊断：1 passed，7.56s。
- 未复跑worldgen177/settlement99/CLI43；这些数字来自worker日志。原结果表“CLI 2+4+1”与实际43不一致，应更正；其中CLI多项含自身TempDir bake，不得称所有测试都仅内存且无任何世界目录。

命令统一`cargo test --manifest-path bevy/Cargo.toml -p glyphweave-core ... -- --test-threads=1`；ignored两项使用完整测试名并加`--ignored --exact --nocapture`。详见本轮日志及从stdout提取的s1-replay.json/s2-replay.json。

## 2. S1：当前冻结世界确实是几何候选穷尽

独立重放：自然hash相同、节点27、道路35，节点/边比较均true。两请求各24次尝试，均0接受，拒绝分布各为：用途8、自然/填方10、旧支撑3、占用3。不是碰到8192站点预算。

原前轮将30次拒绝统计延用到W1并不准确；这里应以当前35边世界每请求24次为准。这次定位纠正了该混用。

`(168,220)`样本在生产preflight中从6885mm变为3360mm，`existing_support_changed`，独立运行也复现了。可以作为后续carve研究的真实输入，但不能因此直接判定“只要改carve就能形成24个地块”。

对共享diagnose模块本身不能整体签收：它在有正供给的多路场景存在剩余需求/站点预算传递问题，见第5节。

## 3. P1：S2没有保留冻结35条道路，122状态结论对象改变

`frontage_supply.rs:469–529`的frozen_w1_stage(version=3)先把原自动程序升为v3，再从31条基础边+2条显式扩展重建，**用v3替代了原v2自动道路阶段**。并非在已独立重建的35条道路上追加。

直接证据：

| 项 | S1冻结重建 | S2 v3求解 |
|---|---:|---:|
| 基础边 |31|31|
| 显式扩展 |2|2|
| 原自动道路 |2|未保留|
| 结束边数 |35|33|

缺失身份：`generated.road-auto.scene-0.0`、`.1`。S2断言只要求edges>baseEdges（33>31），没有检查原35条路/节点/高程义务。**没有删除旧文件**，但评估中的固定基线被替换，违反此次“既有道路保持”的前提。

独立S2重放确实得到122次/96走廊拒绝/26零供给；它只证明**这个33边起点和当前候选器**的结果，不能写成“冻结35边网络完整求解仍无新增供给”。

### 枚举范围也必须收窄

`supply_states`只取每种用途`hierarchy.tier_for(purpose,false)`返回的一个tier；当前122个状态全是12m，没有枚举所有授权tier。新端点高程由infer_end给单一值；无零收益连接路的多步搜索；每接受一步前的局部列表耗尽也不是全局拓扑搜索完备性。

可以保留`current candidate sweep exhausted`，必须同时列出过滤器/宽度/高程策略/起始图。不得据此宣称24不可达、完全穷尽所有授权方案或修改carve是唯一下一步。无需为消除这句夸大而扩大为全局搜索。

## 4. P1：防倒退条件永远为false，实际会漏掉需求回退

`auto_roads.rs:1193–1215`：

```rust
increments = totals.saturating_sub(baseline);
if totals.zip(increments).any(|(total,gain)| total < gain) { reject }
```

对非负整数，`gain=max(total-baseline,0)`恒不大于total，因此该条件不能检测任何丢失。

独立算术反例（详见evidence.json）：baseline=[2,0]、totals=[1,1]，increments=[0,1]，丢失检查false、零收益检查false，会通过这两道守卫；outstanding再仅减增量，可能把已丢掉的住宅继续算作满足。

本结论由实际源码和算术直接证明，未冒称已构造完整生产丢地块场景。必须比较新totals与baseline，并保护既有site ID/几何/归属，不只总数。request不能仅靠zip位置假设匹配。新增真实回退负例，执行者也已承认未完成这一项。

## 5. P1：verified供给仍主要是计划信用，而非实现信用

`verify_supply_commit`（auto_roads.rs:1399–1423）仅调用：

1. frontage::append；
2. development::plan；
3. plan_landcover_with_program。

随后直接返回site_ids.len。这里没有候选完整final栅格上的landcover realize/verify、真实yard surface缺额、最终AccessGraph和已实现入口数量审核。

frontage preflight确实做了局部工程/步行连接检查，不是完全没工程；但其结果不能替代任务要求的**合并最终地形上的完整实现链**。新的正例只检查entity几何/parcel边界/anchors；S2的`parcelsWithEntrance`仅检查frontage_road_point和support_level_mm存在，不能叫realizedWithAccess。

本次因0接受，没有伪造24结果；然而一旦候选出现正收益，当前选择器会把规划成功标成verified/committed。修复应接入既有实现/审核函数，不新建第二套弱审核，也避免在auto_roads内部调用会重新进入auto_roads的全规划审核而递归。

### P2：诊断累计预算与剩余需求没有按道路传下去

`frontage_supply.rs:361–391`每个request构建固定remaining=count，循环每条路仍传同一个outstanding；measure又每条路从完整maxCandidateAttempts开始。例：count=3，第一路接受2，第二路仍可接受3；单路预算5，第一路尝试4，下一路仍可再尝试5。

外层只在开始下一路前检查累计值，不能阻止第二路内部超额。冻结个案0接受且预算8192所以未触发，不影响第2节的个案结论；但需正供给多路小fixture修复诊断与生产的等价性。reported station_attempts/rejected也只取第一个有rejection detail的请求trace（auto_roads.rs:1077以后），不应将该值写成所有请求累计164次。

## 6. 有界性/可重现性：计时只能中止，不能决定权威结果

v3直接在生产规划中依赖Instant截止，并在超时时设置stopReason后返回Ok及已提交前缀；verification批次内部又没有统一检查截止。生产道路来源审核会重跑规划，两个机器/两次负载若跨过不同时间边界，可能返回不同plan（或仅report不同也导致来源不一致）。本轮6–8s未触发180s，故没有观察到此次实际漂移；这是明确的实现风险，不能用一次快速运行证明确定性。

要求：确定性候选/站点计数控制成功规划；墙钟用于资源中止时返回结构化incomplete/error且不发布成功前缀。所有计数/诊断/复核共享上限，重型步骤前后可检查，同一时钟/取消机制足够，不做新的调度平台。

## 7. carve结论：样本真实，归因不能直接授权全局改语义

production中graded_influence触发后，full_target按较高priority完整平台重算，最终覆盖blend。代码确实允许“添加新完整平台使已有blend值降低、但仍不低于自然值”。这与“已有已承诺工程支撑不可改变”存在冲突，当前preflight拒绝是保护行为，不是证明这个位置本来合法。

坐标纠正：冲突道路auto.0中心x=160、yaw90、宽12；样本x=168横距**8m**。其整平半宽是**7m**，运输足迹半宽9m。正确描述是“整平带外1m、运输足迹内”，不是“9m整平带外1m”。

“改动会影响每一个已烘焙世界”也无证据：潜在影响是使用相关叠加条件的世界；原文件不会自己改变。真正修复应区分保留既有承诺、合法追加工程、道路精确高程和过渡带，不能简单用max(原terrain,full_target)就算修复，那可能重新抬高应精确整平的路面。

本轮暂不批准全局修改apply_terrain_carves或缩窄旧支撑检查。先纠正S2基线、信用和回退，再用同一冻结上下文提交最小可重现冲突与版本化提议。

## 8. 下一步

任务：`G:/difyllmwiki/GlyphWeave/docs/collaboration/ds-v41f-b01-supply-r1-task.zh-CN.md`。

本轮评审不是再重开R6/版本合同平台；所列问题均直接影响新供给算法是否在正确世界上计数、是否能保留既有建筑和是否把计划错算成实现。24见证仍不强求；不批准正式bake或大测。
