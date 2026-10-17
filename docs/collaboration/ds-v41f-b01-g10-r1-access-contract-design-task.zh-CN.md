# V4.1 B01-G10-R1：纠正access合同适用范围，补连接设计与profile绑定

## 1. 裁定和边界

先读b01-g10-connection-design-inspection.zh-CN.md及inspector-b01-g10-20260919/evidence.json。归属恢复和同priority不同几何换位测试保留；H-G10-1合成语义方案未批准。

本轮不是新自然实验。自然harness/plan重建/design/append/infer/measure/carve或access路线重放/realize/bake全0。生产源码/阈值/地形/需求/保护/默认/公开接口不改。只允许auto_roads.rs cfg(test)最小profile身份回归、只读已有文件/源码与设计文档。无Git/agent/F/A，B partial/C不推进。

## 2. T1：准确写access合同，撤回过强结论

逐项引用现access.rs和frontage.rs，区分：
- authored anchor：北(270,163)、南(270,197)；
- access::exit_point：北(270,162.85)、南(270,197.15)；facade北(270,162)、南(270,198)。0.30m门槛只在这两点相对建筑ground_height判断。
- 路径中心及两侧0.75m采样，<=0.5m采样步长、坡度<=0.25；2m网格8邻域、最长120m、最多6000expansion；实际frontage接受还要求连指定BC。
- 已观测冲突x268点属于candidate carve的边缘，不等于x270路径中心/±0.75检测带。
- 局部高差能否否定一条明确路径，与能否证明所有允许路径不通，是不同命题。

撤回“0.30m限制使旧肩部高点必不可跨”“B2牺牲真实连通已证明”“保持与连通存在明确必要矛盾”“只有C或拒绝”。正确状态为未证明可达，也未证明全域不可达，不反向声称B可行。

不得编造现有未采样路线的高度/障碍；只读现保存JSON/自然数据不能替代候选最终工程面。若提出需要更小范围的路径数据，明确缺口，不在本轮自行生产采样。

## 3. T2：明确所有不变量及接口，不再只盯四个点

设计要同时满足：
1. 原既有受保护support全部before==after，不能只“非下降”。
2. 新候选building+2m全pad必须等于site设计高程2390，不只入口中心。
3. 门口/立面0.30m限制和真实连续可行路径均满足，且连指定road_id。
4. 原natural no-cut/fill/zone/water/障碍/parcel/owner/yard合同不放宽；未运行的后续审核仍not_run。

将内部TerrainCarve结构可表示与现manifest可作者声明分开。对一个首选连接方案明确：入口、目标、平面路径/高程、硬支撑作用域、blend作用域、是否须新增字段、解析/编译/前检/bake/audit如何一致读取、版本是否绑定。若没有可表达的新规则，就交接口设计而非“现数据已支持”。

避免用H-G10-1笼统“只抬高”代替规则；如仍建议C，必须明确四项不变量如何满足或哪里仍未知，和为何比几何方案更合理，不因本轮局部失败自动要求改全局语义。只能选1个首选、最多1个备选。

## 4. T3：profile身份绑定的小修复

verify_carve_identity当前只比较has_profile。补上源实体的完整profile与carve携带profile的确定性内容比较（可直接逐字段或可靠hash，版本/start/end/elevations/maxFill等不能漏）。保留现geometry/priority/target检查，不影响生产。

新增一个合成负例：同一道路外形/中心高程/target不变，只将非端点非中心的一个profile采样+1mm，当前弱校验会漏，修复后必须拒绝。诚实正例、已有同priority交换、旧G9合成回归各必要精确运行；不宽过滤启动自然ignored测试。

## 5. T4：下一实验提案仅写，不执行

根据T1/T2的缺口提出最小的、范围固定的一次未来验证。优先选择不改变生产语义即可检验的一项假设；若必须修改，则先给测试专用/生产变更范围、revision与默认关闭策略，主检查员另批。

单次特定方案失败，只能否定该方案/输入，不能用来证明所有B连接几何失败。输出必须能区分旧支撑变化、pad不平、入口门槛、路径坡度/障碍/指定道路、预算未穷尽等阶段，不统称access失败。

不要把realize纳入本轮，不跑自然“看看结果”。与本轮0自然承诺相符。

## 6. 交付

新增docs/collaboration/b01-g10-r1-access-contract-design.zh-CN.md和bevy/work/collaboration/ds-b01-g10-r1-access-contract-design/：合同坐标/范围表、一个修订首选设计、输入/源码hash、合成profile绑定测试日志、待批实验方案与回执。G10原文只追加勘误，旧receipt/run/attribution保持。

离线脚本累计<=60s，合成测试/编译单列；doc-tree/diff/hash检查后停止。状态design_ready_for_review，implementation/naturalValidation=not_run、realIncrement=none。
