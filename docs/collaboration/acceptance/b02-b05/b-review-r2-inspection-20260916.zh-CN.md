# B Review R2 独立审查：诊断有效，容量结论未闭合

日期：2026-09-16（Asia/Shanghai）。主Agent仅检查与编写任务，不调用子agent，不修改业务代码，不提交Git。

## 1. 结论

**R2可作为诊断进展保留；B仍partial，没有完成24/24/24。**

准确裁定：固定8m端口的那一条走廊确实不可设计；降低端口后的孤立走廊可以设计。R2没有证明整个冻结需求不可实现，也没有形成新的合法连通路网/完整地块组合。停止盲目bake是正确的，但下一步不能继续只扫孤立走廊然后等待人工重复派同一任务。

需要纠正高度单位解释、道路容量口径，并把“路径设计→可接入网络→完整parcel分配→实际实体/地表”接起来。既有自然案例的成功可行性尚未证明，主Agent此前把它直接当24栋必过正例的安排需要调整：原案例保持未解/困难回归，另建明确可行正例验证端到端能力，不能改变原案例后宣称原案例成功。

## 2. 本轮独立证据

目录：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b-r2-20260916/。

- 实际HEAD：fb36dc4f9fdd38e6aa9c5a89676a65b96e5001b1。
- R2回执实际3549 bytes，SHA256 d88309e000ffc9020978808745c59aa0233f416b96402c014002bcaa5ff3b413，与报告一致。
- R2文档/诊断/日志、复用R1回执/日志、R1八个源码绑定及输入，共21项检查全部匹配。R1绑定的八个源码文件未变；不是声称整仓全部文件逐项不变。
- 自然栅格核对：R2使用的B01成功世界与R1 control-2失败目录中的完整512m natural height chunk逐字节相同，SHA256均为1a616848cacf6e87f2d4ca8f6e467e3cda1009a925e1cf5be0fb1f221ebb8cd2。R1和旧B01 manifest有道路政策/显式走廊/spacing/rulesDir等差异，不因此把两份最终世界视为相同。
- 本轮使用已有debug exe，32366592 bytes，SHA256 9715872d0b3048454dae7c834e1286a03fdd73cc068896099b3a54b6e8f0bc5a；未重新构建，不把exe hash当重新编译来源证明。

独立运行的三条CLI操作：

| 操作 | 结果 |
| --- | --- |
| 原request [258,224]→[258,160]，16m，两端8000mm，design-road-corridor | exit1，精确复现 station45 required7120 > cap7040 |
| 同一request仅两端改7000mm，写检查员新输出 | exit0，design_only_not_applied；maxFill=5849mm，maxRise=80mm/m |
| verify-road-corridor 对上述新设计 | exit0，明确不是建成道路或世界质量证书 |

本轮没有生成新世界，没有gate、Rust回归、release或浏览器巡检。405 core等为R1历史证据复用，不写成本轮执行。旧R1审查中列出的12/2/7/Node252属于前序审查记录，不能据此扩大本轮验证范围。

主要证据：review-evidence.json、natural-identity.json、diagnostic-consistency.json、original8000.log、lower7000.log、verify-lower7000.log。

## 3. station45：是真实拒绝，但7120不是填方厚度

代码来源：G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/worldgen/road_alignment.rs 的 envelopes/solve。

solve比较的是同一高度基准下的路面绝对高程下界与允许高程上界：

- required=7120mm：坡度约束传播后的最低路面设计高程；
- cap=7040mm：该站横向支撑的自然高程+允许填方等约束给出的路面高程上界；
- 二者相差80mm，故该固定request无解；
- maxFillMm仍是6000mm，不能把7120解释为“需要7.12m填方”。

本request总长64m、末端8m固定8000mm，固定段起点station56；坡度上限80mm/m往前传播至station45，给出8000−(56−45)×80=7120mm。这直接说明固定高端口与坡度约束参与了失败。不是仅凭本报错就能断定“中部山脊不可绕过”。

R2诊断表字段requiredFillMm语义不准确，应另写修正版requiredRoadElevationMm/allowedRoadElevationMm，同时保留实际maxFillMm和具体自然支撑见证，不覆盖旧表。

## 4. 单走廊成功不等于整个网络合法

独立设计CLI只读取自然地形/水体/zone做走廊设计，不加入世界，也不审核全部既有实体、完整parcel竞争、父路接点高度一致与端部加宽支撑。

road_extensions真正接入还会检查：父路连接、start/end junction height match、padded footprint fill、与旧道路/site冲突等。因此孤立的7000mm设计不能直接替换原8000mm端点接入。

后续需要把同一共享节点的高度作为网络共同变量/来源；已有固定节点必须保持，新增节点可以在合法高程区间内选择并验证所有相连边。不能每条边各自选一个好看的端点高程，再宣称整网连通。

## 5. 容量结论仍只是搜索结果

R2目录递归盘点：882个文件，其中799份request JSON、74份design JSON；不是799次CLI调用的可靠计数，也不是74条互不冲突的可接入道路。回执只绑定7条设计日志，其他搜索未提供每次命令/退出码/拒绝原因总表及实际parcel候选/冲突图。

这不等于数据造假，但不能把“无design文件”当作每一个request的已验证不可行证据。不要为修复此项重新扫800次；新轮只记录本轮有限候选的直接结果。

已存在设计中的自然/zoning hash一致；表中列举的竖向可行组合能找到对应design文件。诊断数据可用于进一步候选分析，不能当作全局容量上限。

### 容量估算有边界问题

实际urban zone是 [128,256)×[128,256)。用当前frontage 12m道路、setback8、12×10建筑、4/4/12 inset和spacing20算：132m横路有6个station，但两侧并非都在urban内。

- z=224道路：南侧parcel的z范围[234,260)，越过256边界；不能按12栋算。
- z=160道路：北侧parcel的z范围[124,150)，低于128边界；同样不能按12栋算。
- z=216道路：两侧parcel的z范围[180,206)、[226,252)，仅从urban边界这一项看都能容纳；因此“下带只能一行parcel”也需区分道路行数与建筑行数。

上述仅是平面边界核算，尚未扣除道路交叉、现有site、相互冲突与工程支撑，不构成发现24栋合法解的声明。R1原“2×12=24”不能作为预先证明控制可行的依据。

## 6. 管理上的调整

不再要求执行者对未经证明可行的冻结自然场景反复bake直到24/24。正确的测试集合必须同时有：

1. 原R1困难自然场景：保留原seed/用途/需求/地形、固定路网约束，记录找到的可行数量或搜索未解；固定8000mm走廊的失败作为拒绝回归。
2. 单独的已知可行集成正例：明确输入来源，验证24个地块、实际入口/yard、两级路网和独立审计；不得覆盖原案例，也不能声称原场景已通过。
3. 对自然场景的有界联合规划：沿用现有kernel设计/接入/放置方法，把节点高度、道路连通与完整parcel候选一起验证，而不是只枚举孤立长直线。

这调整的是测试设计，不减少原用户需求，也不是把B/C完成标准改成“换简单图过关”。原场景与最大6×10km目标仍待解决。只有正例通过，仍不足以签收B。

下一步任务：G:/difyllmwiki/GlyphWeave/docs/collaboration/ds-v41f-b-review-r3-task.zh-CN.md。
