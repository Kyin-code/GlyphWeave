# DS v4.1f B-02：需求驱动的道路供给与地块/农田可达性

执行授权来自 G:/difyllmwiki/GlyphWeave/docs/collaboration/ds-v41f-b01-b05-runbook.zh-CN.md。B-01达到可用接口/工程自测通过后可自主进入；不是等待主Agent当场签收。只修改本任务范围，完成后写checkpoint并按总任务书进入B-03。

## 1. 已知问题与必须交付

A有270条已实现road edges，但自动增长24段预算耗尽，882目标中813未服务。现有auto_roads主要看zone相交与中心距离；“道路几何合法”与“实际供应足够”不是同一指标。

本任务交付Rust的可配置道路层级/需求排序与有限增长，使住宅地块、服务设施、农田接入在小场景中真正受益。保留原目标/拒绝统计，不通过改变距离阈值或漏掉难目标冒充覆盖率提高。

## 2. 行为合同

1. 复用已有road_network、auto_roads、road_alignment、road_extensions；不写第二个脱离工程高程与保护区的路网生成器。
2. 原program缺省/旧version保持原行为。新层级可作为显式version/可选program，记录 collector/local/farm-access 的功能、来源和父连接；不把换名称当成实际拓扑层级。
3. 主供给路承担跨需求区连接，local连接可分割地块与设施，farm-access承担农田末端接入。至少证明两级道路/接入的真实连接关系；农路优先复用farm_access已有4m通道，不将其冒充城市主干路。
4. 如道路层级引入不同宽度，本轮仅允许8/12/16m显式配置，旧8m语义不变。宽度必须进入权威Entity/road profile、占地分区、保护/水体/碰撞/支撑及独立审核；不能只改渲染宽度或label。无法安全完成宽度合同则保留partial，不弱化旧8m安全校验。
5. 候选优先级考虑真实未满足请求、可用临路长度/地块空间、公共设施需求、农田接入缺额，不能只优先最近zone中心。计划收益与最终实现收益分列。
6. 连接只通过合法交点/端点和同高程约束；切分生成道路需保留parent/source ID，不能移动authored/固定道路、删除旧道路给新道路让位。拒绝无依据的立交、桥隧或跨水捷径。
7. 规划、工程地形、最终road entity/access graph必须来自同一最终道路计划。道路增长完成后再形成B-01地块，不允许新增道路切穿已确认地块却不重新验证。
8. 不放宽原填方、坡度、保护、水域、固定支撑、连接与通行约束；不要改自然源形状来让路走通。

## 3. 本轮预授权的道路政策预算

原A manifest永久只读。可新增B候选manifest，在原需求、用途目标、自然设置不变的前提下选择一次最终道路政策：

- maxNewSegments可选24/64/128/256；不能超过现有256上限。
- maxCandidateAttempts默认16384，上限65536；不得无界循环。
- segmentLengthsM沿用现有支持的偶数16..1024m范围，候选档位不超过8。
- reachDistanceM仍160或输入原值；不能为消掉旧unserved把它增大。
- 原144新增frontage、设施/housingMinimum、楼体尺寸、旧资产需求不减少。

先用静态容量分析与≤1km小测试选择一个最终配置，写明预计新增路长/可服务前沿与实际资源上限，再冻结。**不得对4档预算各跑一次最大图穷举。** 这个授权改变的是新B内容政策预算，不改变600s/8192MiB计算预算，也不声称24段原配置已满足。

原zone目标检查继续保留；若引入额外地块/农田目标，不得从分母移除原困难目标。实际不可达/不可建、预算耗尽、无候选、工程不可行分别报告；不可行判断必须有约束依据，不是优化器没找到路径就宣称数学上无解。

## 4. 写入白名单（路径均在 G:/difyllmwiki/GlyphWeave）

生产：
- bevy/crates/core/src/worldgen/auto_roads.rs、road_network.rs、road_extensions.rs、road_alignment.rs；
- 可新增 worldgen/road_demand.rs 或 road_hierarchy.rs，模块声明仅接线；
- worldgen/frontage.rs：只接入新最终路网/完整地块候选，不改B-01输入语义；
- settlement/development.rs：仅真实道路宽度/来源占地接入；
- worldgen.rs：必要版本/管线接线；不得修改自然生成算法；
- worldgen/road_profile.rs、access_surface.rs、access.rs：仅新显式宽度/连接的最小一致性适配与测试，原固定入口/毫米负例必须保留。

新增 tests：bevy/crates/cli/tests/road_supply.rs；examples/b02-road-supply.manifest.json；docs/collaboration/b02-result.zh-CN.md；本轮work。

若B-01已新增几何模块可调用，不绕过其占地合同。不得修改CLI gate成功判据、旧A checker或生态/农业实现；农田需求此阶段作为目标输入，实际田块执行归B-03。

## 5. 冻结的小场景验收

- 保留旧program缺省正例，加入同seed新program重复、非零origin、输入请求乱序的稳定性测试。
- 构造≤1km可行控制：至少2个由collector连接的需求片区，各有local/末端接入，包含B-01地块需求、至少1个已有支持的公共设施和1片农田接入需求。
- 同一个控制输入对照“无新增供给/旧预算”与新方案：原有需求不下降；至少一个原未满足地块或农田接入需求变为真正可达，不能只更改report的has_road。
- 合法增加道路后，B-01地块/门/庭院仍无冲突，旧accepted实体/道路身份未被无故重排。
- 负例：需要跨水但无桥合同；保护带隔开目标；同平面交叉未建节点；与固定road/site支撑不兼容；路段预算耗尽；错误宽度占地；新增道路切穿完整parcel；重复/移除旧road ID；人为篡改需求分母。
- 可行控制目标全满足，困难控制明确失败/缺额。大图容量未证实可标partial_content，不能把小场景通过称为882目标全覆盖。

定向测试建议前缀road_supply；并跑auto_roads/road_network与受影响frontage正负例。最多3次≤1km bake+gate，遵循总任务书预算。全部字段/函数来源与前后对比写入b02-result；供给不足但工程稳定可交B-03继续补末端连接，不能自签B-02通过。
