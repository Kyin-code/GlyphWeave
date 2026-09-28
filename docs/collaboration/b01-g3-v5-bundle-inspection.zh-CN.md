# B01-G3 V5 实验独立审查（2026-09-18）

## 结论

**needs-changes（实验负观测可保留，轴穷尽结论不接受）**。

接受：已登记5个不同状态的两次执行均未取得增量、candidate realization=0、S01确实在街段的shared-junction逐格高程匹配处拒绝；已观测运行中主baseline没有泄漏。
不接受：B只有2560mm可行、冻结几何下B轴已穷尽、剩下只能改C/宽度/坐标/工程合同。检查员生产反例证明B=3000mm接入也合法。完整路束在3000是否可行仍未测试，不能反向宣传成功。

B保持partial，C不推进；本轮不批准新的junction合同、不改carve，不重跑原五态。

## P1：把可达下限误写成唯一可行值（已用生产反例推翻）

位置：b01-g3-v5-bundle-result.zh-CN.md §3.3/§6，receipt.json bDimensionPinned/nextStep。

8000-80×68=2560证明的是B下限；1920/1812/2471低于下限、7692高于填方上限，只能说明这些采样点被拒，不能排除(2560,7692)中的其它值。infer_end提供最小可行端点，也不是唯一解证明。

检查员使用当前重建库的公开road_alignment::design/verify与road_extensions::append，在**冻结35边原plan、原natural、原zone、水体、原scene**上检验：

| B高程mm | approach design | verify | append | 边数 |
|---:|---|---|---|---|
| 2560 | pass | pass | pass | 35→36 |
| **3000** | **pass** | **pass** | **pass** | **35→36** |

A=(288,96)/8000，B=(288,180)，12m/shoulder3/junction8/fill6000/rise80，全部不变。公开append按ordinal命名，为不与已存在的显式extension.0/.1冲突，探针在程序中以两个不同scene的跳过槽保留ordinal，真正只执行同scene的第三条候选；没有重命名/移除任何旧实体或改自然/场景。主baseline序列化bytes保持不变。

这是真实production append反例，不是JS近似包络，也不是换seed的合成世界。它只证明接入B不唯一，没有运行B=3000的完整两段推导/测量/realize。

正确口径：**预注册五点集合已执行完；B取值域未穷尽。**

## P2：共享交集漏掉56个端外格点，计数140/23错误

位置：auto_roads.rs:4093–4103，特别是4095的(288-x) as usize和4096的get。

交集[279,171,292,184]按生产闭区间为14×14=196格。x=289..292时站点为负，转usize后get返回None，该区域被诊断静默跳过。生产RoadProfile::height_at应将端外投影clamp到端点，不跳过。

检查员对S01的两段生产design，用两侧真实profile.height_at重算：
- 比较格点：**196**（不是140）；
- 不匹配：**27**（不是23）；
- 漏掉的4个不匹配点：(289..292,171)，approach=2640mm、street=2560mm。
- 街段生产append仍拒绝：extension overlaps road outside a matching endpoint junction。

因此拒绝事实与9m支撑/8m平台风险仍成立；“完整交集逐格比较”的证据必须纠正。不得改生产容差或profile来迎合旧诊断。

## P2：预算被改写成每进程一次，且重复已登记状态

任务书§2/§6规定整轮baseline realization≤1，重启计累计，不重试同态。本轮日志和两个JSON显示：
- unique states=5；实际attempts=10；重复执行5态。
- candidate realization=0。
- **baseline realization=2，超过授权1次**。
- runtime约8.04+7.94=15.98s仍在总180s内；不能用“各自180s”替代整轮预算。

第二次为追加诊断不抹掉其baseline和状态成本。应追加真实勘误，旧回执不改；不能声称全预算合规。无需为了纠正统计再执行一轮。

## P2：新实验harness的末段截止保护不完整（静态确认，当前负结果未受影响）

位置：auto_roads.rs:4023–4026、4257–4264、4270–4348及最终JSON输出。

当前检查循环开始/候选realize之前的耗时，却不在realize返回后、best=Some之前、最终输出之前复查截止。若最后一次realize跨过180s并返回成功，仍可能发布best且incomplete=false；若最后一态慢设计后拒绝，也可能漏标incomplete。

当前两次日志仅约8s且都在realize之前失败，所以不据此否认现有negative observations。但该harness不能继续承担下一次可能成功的实验，须在测试层修复。这里不是重新打开已验收生产run_supply_phase的时钟修订，也不授权80点clock重跑。

共享节点错误守卫只证明第二段start_level不一致被生产拒绝、原plan未改；它不覆盖晚超时/零收益提交。不要扩大该测试证明范围。

## 完整性与真实检查动作

- 回执SHA256：d18105864cfe156eed824c6d77e2629343df2e16edc9dcc9e0819d9db7a6fb2c。
- 14项回执bytes/hash绑定全部匹配；8旧回执/7冻结输入/6个G2-R1脚本与JSON证据匹配；5个其余G1生产源码hash匹配。
- 将新增两测试块从内存副本删除，并恢复紧邻旧测试的两处CRLF后，auto_roads.rs精确还原G1 hash e6af8cb6…：确认只改测试区，未改生产逻辑。没有实际恢复/改写文件。
- 重建core库1次；外部Rust只读probe编译1次、运行1次：2个approach水平、S01街段固定C=-382控制；公开调用design3/verify2/append3（这些API内部可能再次design），无infer_end、frontage measurement、baseline/candidate realization、bake。
- 没有重跑worker自然五态harness/共享节点守卫/128/80clock/carve/CLI世界/release/browser/max/workspace。旧测试通过数只核对日志，不冒称独立重跑。
- 探针只在临时内存plan上暂存；冻结输入文件不改。无业务/测试源码修改、无子agent/Git写操作/F盘写入。

证据目录：bevy/work/collaboration/inspector-b01-g3-20260918/。
level-probe.rs/json、library-build.log、probe-build.log、integrity-review.cjs/json、review-decision.json。

## 下一步

执行 ds-v41f-b01-g3-r1-closeout-task.zh-CN.md：一次性纠正结论/196格诊断/累计预算，并补新harness晚截止小测试；不重跑自然baseline或原五态，不改任何工程合同。新提案可以保留B/C现合同内的剩余取值自由度，不要把本次负结果当成必须修改junction的依据。修订完成停止，不自行开始下一自然搜索。
