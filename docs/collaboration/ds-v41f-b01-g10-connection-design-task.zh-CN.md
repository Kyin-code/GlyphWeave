# V4.1 B01-G10：候选连接方式设计与归属离线收口

## 1. 目标与禁止项

G9局部机制已限定接受，检查员已离线恢复base决定性owner，见b01-g9-local-support-provenance-inspection.zh-CN.md及inspector-b01-g9-20260919/offline-owner-recovery.json。本轮不再自然重跑补标签。

主要工作是设计候选如何在不改变既有支撑、不放宽审核的前提下真实连接道路；不是实现策略，更不是批准方案B必定可行。B partial/C不推进。

本轮自然harness/plan重建/design/append/infer/measure/terrain composition重放/realize/bake全0；不改生产源码、公开接口、carve/no-cut/fill/保护/需求/地形/默认。仅最小cfg(test)映射辅助改进、精确合成回归、离线JSON算术与设计文档允许。无Git/agent/F/A。

## 2. A：一次性关闭归属资料问题（不要另起自然轮）

- 接收检查员离线恢复：idx2为旧住宅10-11.1的pad，idx49为旧道路25-26，idx78为新增BC；对比冻结edge/site顺序与144条记录数值，确认来源，输出新派生attribution.json。不要改原G9 run/receipt。
- 标明原ownerEntityId/ownerKind/component、ownerMappingVerified与派生roadCarveBlendM等无效范围；BC正确blend约97.626m，120属于其它carve。控制点2230就是BC的x268处profile（station20），不要说它不是BC设计值。
- 元数据映射补强不能只比较priority。对同优先级的两实体、同优先级pad/apron给出不同几何的合成fixture；验证owner+component与中心、half extents、blend、target、profile、priority等完整签名对应，故意交换同priority归属必须报错。保留base++candidate顺序，不全局重排。
- 可在cfg(test)对单实体production plan_carves结果附owner并重建稳定顺序，再与整批产物完整比较；组件归属也要有实际几何核对，不能仅凭字符串猜。所有验证只用合成实体，不读真实自然场景运行carve。
- 增补两种metadata hash编码和run/current-source分版说明；旧运行源码无存档写unknown。不把合成测试成功充作自然run已修。
- A已签收保持，不改里程碑A状态；只声明本worker不新增签收。

这些修正以G10新文档/派生文件及G9文档追加勘误承载，不覆盖旧JSON/回执，也不要求旧绑定全部继续匹配已声明修改的文档/测试源码。

## 3. B：提出一个可审查的连接设计，不先写生产补丁

只针对固定ST18-R1的station18北/南，pad2390、BC profile与旧道路/其它义务保持；不改B/C/setback/院落/建筑尺寸。

设计必须同时说明：
1. 建筑入口、连接起止点、道路连接目标的平面/高程定义，不把centreline profile值与肩部已有final采样混为一谈。
2. 当连接横穿道路整平带外但仍受保护的8/9m采样时，如何保持旧值6502/6987等；若必须改变任何旧支撑就超出现有约束，明确标出，不躲开测试。
3. 从2390mm入口到受保护较高地表再到道路的连续可行路径：宽度、坡度/高差和步行寻路规则必须来自现有access/entrance生产合同。若模型无要求也如实披露，不能凭主观认为“能走”。
4. 若提出缩短/裁切connection carve，说明现有地形上的剩余路径与入口真实连通为何仍可能成立；删掉构件或不影响sample本身不等于access pass。
5. 新的连接几何/target是否能由现数据表达；不行时列最小schema/版本/consumer变更，不把接口设计冒充代码已存在。
6. 相对于全局carve政策变化，哪些输出会变、哪些旧路径默认不变、revision/version如何绑定、旧证据如何保留。

允许读取已保存4点及冻结实体/profile的有限相关几何，做必要不等式或示意表；禁止生成新高程场、复制全solver或改写terrain composition作“离线试跑”。未采样区间仍unknown，不从4点推出整条路径可行。

结论允许为：方案可表达但缺一项小验证；或现合同下存在明确必要矛盾，需重新决策。禁止称方案B是唯一可能方向、或无需验证就必能解。

## 4. C：为后续实验设计一个预算，但本轮不执行

只交一个首选方案及最多一个备选；明确要检验的具体假设、最少实体/点/状态、预期反例、一次输出字段、停止条件。包括：不破坏旧硬支撑、入口确实可通行、新yard/完整parcel未受损、旧世界默认路径不受影响。

如果方案需改生产代码，先给文件白名单与版本策略，不动手。若需再次点采样或完整自然运行，说明为什么已有证据不足，留主检查员审批。不自动运行。

## 5. 预算与交付

离线脚本累计≤60s；仅必要精确合成映射测试（建议≤3个）及编译单列；禁止宽过滤触发旧ignored自然测试。合成失败可修，但不得启动自然诊断以“顺便验证”。不全workspace/CLI世界/gate/browser/max/release。

新docs/collaboration/b01-g10-connection-design.zh-CN.md，新bevy/work/collaboration/ds-b01-g10-connection-design/：归属派生文件、强绑定合成测试日志、source/inputs/hash、连接设计与接口/版本影响、待批实验方案、回执。旧G9仅追加勘误，旧run/receipt不改。

doc-tree/diff/hash检查后停止，状态design_ready_for_review，明确implementation=not_run、naturalValidation=not_run、realIncrement=none。B partial/C不推进。
