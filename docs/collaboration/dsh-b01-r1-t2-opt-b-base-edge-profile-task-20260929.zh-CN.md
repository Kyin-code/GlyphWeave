# DSH执行令：B01-R1 T2（OPT-B）基础道路 profile 消费链

日期：2026-09-29。任务号：B01-R1-T2。状态：**允许实现 T2；T3 闸门未开始；T4 世界运行未授权**。

## 0. 严格执行与恢复纪律

**请严格按任务书完成任务，不得因为其他任务无关问题擅自中断或停止部分步骤；不得因为上下文压缩、会话切换、摘要丢失、其他任务状态或可修复的驱动问题跳过必测步骤。**

如需中断，必须先在新轮目录写 `checkpoint.json`，至少记录：当前源码与输入 hash、T0/T1/T2/T3 状态、实际账目、最后错误、剩余步骤、恢复后的第一条具体命令。恢复后继续执行，不能把 checkpoint 当作交付，也不能把未观察项写成通过。只有出现本任务书的硬停止条件才可停止，并须说明哪一条触发。

业务任务不得写 Git、不得切换/回退分支、不得覆盖历史证据；不得修改 `F:\modenmapbox`。证据目录必须新建：

`G:\difyllmwiki\GlyphWeave\bevy\work\collaboration\ds-b01-r1-t2-opt-b-profile-20260929\`

历史 B 主线、B01-R1 T0/T1、G32-G34 目录只读。不要启动子代理。不要运行 512m 世界，不要运行 6×10km 世界；T4 的最后一次 1km 额度在 T3 全绿并得到本任务书明确的后续授权前不得使用。

## 1. 独立检查结论与方案选择

我已独立复核当前实现链，确认原任务书写的“`CorridorDesign → plan.extensions → entities()`”不能机械成立：

- `road_extensions::append_one` 会新增道路边，要求节点/父路接入，并拒绝已有基础边中段重叠；不能拿它给基础边挂 profile；
- `RoadNetworkPlan::entities()` 当前只从 `extensions` 反查 profile；
- `frontage::stations()` 只接受 `EntityInstance.grounding.road_profile`；
- `RoadProfile::validate_entity` 要求端点、长度、宽度、中点高程和实体完全一致。

本轮选择 **OPT-B**：在道路规划路径内给 `RoadEdge` 增加可选、可序列化的基础边 profile，并让 `RoadNetworkPlan::entities()` 直接消费该字段；不新增 `RoadNetworkPlan` 级 profile registry，不把基础边伪装成 `extensions`，不新增重复道路。这个方案仍属于“基础边 authored 时的必要 profile 接线”，不是 OPT-A 的 plan 结构扩展。

**若实现过程中证明连 `RoadEdge.profile` 也不能让生产 `entities()`、`frontage::stations()` 和后续 earthwork/audit 全链路消费，立即停止 T2，记录最小 OPT-A 设计和阻塞证据；不得偷偷新增 plan-level 字段或另造旁路。**

## 2. 上轮未做好清单（本轮必须主动补齐）

1. 上轮任务书错误假设 `extensions` 是通用 profile 注册表；本轮不得再把基础边塞入 `extensions`。
2. T2 只有研究，没有生产实现、没有最小非空 base-road fixture、没有证明 `stations()` 真正看到基础边 profile。
3. T3 离线闸门未跑，T4 1km 真实消费也未跑；不能用已有 b8 世界或旧 CLI 输出代替新代码验证。
4. 先前质量判定曾只看进程 exit code；本轮任何质量报告必须同时读取 JSON `status`，`status != "pass"` 必须判失败。
5. T0/T1 之外的修改不能沿用旧写集。T2 必须在首个源文件写入前重新生成唯一 write-set、pre-edit bytes/hash 与文件类别，缺项就停止。
6. 失败/拒绝必须保留原因；不能把“未完成”“未观测”写成通过，也不能为了扩大通过率放宽 slope、fill、water/protected 或 `RoadProfile` 合同。

## 3. 修改白名单与禁止事项

### 3.1 允许修改

默认白名单只含以下生产文件，确有编译需要才可增加测试/机械同步文件，并须先写入 T2 write-set：

- `bevy/crates/core/src/worldgen/road_network.rs`：`RoadEdge.profile`、基础边 profile 生成、`entities()` 消费、定向测试；
- `bevy/crates/core/src/worldgen/road_extensions.rs`：扩展道路 `RoadEdge` 构造显式写 `profile: None`，以及可复用的 design→`RoadProfile` 转换/验证 helper；
- 与上述结构字面量同步所必需的定向测试文件，不能借机修改业务语义。

`road_alignment.rs` 本轮只读；`frontage.rs` 原则上只读，因为它已经按实体 grounding 消费 profile。若必须修改，先停止并提交“超出 OPT-B 的理由”，不得直接扩大写集。

### 3.2 明令禁止

不得修改 zoning、reserved ledger、natural/protected/water 判据、需求数量、地块尺寸、reachDistance、urban/rural 比例、zoning cell、road candidate budget、profile slope 上限、max fill 上限、G32-G34 viewer/渲染/材质；不得删除/移动 authored/fixed/base road；不得把 `frontage` 改成忽略 profile；不得复用旧世界、旧 exe、旧 manifest 冒充 T2/T3 证据；不得执行任何世界级“参数扫描”。

## 4. T0：先做新写集与基线

在任何源码编辑、自动格式化或 cargo 编译前完成并落盘：

- `write-set.json`：每个文件唯一一次，包含绝对路径、role（semantic/test-only/comments-only）、pre-edit bytes、pre-edit sha256、相对 T2 基线备份的 sha256、当前 HEAD/dirty 摘要；
- `baseline.json`：至少绑定 `road_network.rs`、`road_extensions.rs`、`road_alignment.rs`、当前生产/测试输入与 R1 T1 结果；
- `intent.json`：明确写 `decision = OPT-B`，并声明没有新增 `RoadNetworkPlan` profile registry；
- 生产源码修改前，复制并校验 T2 源文件 bytes。不要修写旧 R1 receipt；旧轮缺口只在新报告披露。

若发现 `RoadEdge` 的新增字段会要求超出白名单的广泛结构重写，立刻停在 `scope_blocked`，不要先改再请求授权。

## 5. T2 实现合同

### 5.1 基础边的权威 profile 所在位置

给 `RoadEdge` 增加：

- `profile: Option<RoadProfile>`；
- serde 默认缺省为空，并在为空时保持旧 JSON 输入可读；
- 生产 base edge 写入合法 profile 时才序列化；扩展道路显式写 `None`，扩展道路仍由 `RoadExtension.design` 作为唯一来源。

`RoadNetworkPlan::entities()` 的优先级必须明确且测试锁定：

1. `RoadEdge.profile`（基础边）；
2. 旧的 `extensions` 反查（兼容现有扩展边）；
3. 无 profile 的旧平路行为仅保留给未能进入 profile 设计链的 legacy/no-zone 测试输入，不得成为 zoned base road 的默认伪 profile。

同一个 edge 只能产生一个 road entity；不得通过增加 extension edge 来“实现”基础边 profile。

### 5.2 基础边 profile 的生成

在 base road 已选定、端点节点已知、`level_m` 已确定之后，针对选中的基础边：

1. 用 `RoadNode` 的真实端点生成 `CorridorRequest.start/end`；不得用中心点加减长度猜端点；
2. 使用真实 scene id、天然毫米地形、当前 zoning mask；
3. 继承已有道路工程合同：`road_width_m=8`、`shoulder_width_m=3`、`max_fill_mm=6000`、`max_rise_mm_per_m=80`，除非当前源码已有明确的道路宽度来源；不得改上限；
4. 以既有 base network 的 `level_m` 作为两端 tie-in 的毫米高程，`junction_length_m` 使用合法且小于等于道路长度一半的值（最小 fixture 应使用 8m）；
5. 调用真实 `AlignmentContext::design`，保留其自然地形、完整横向 raster、fill/protected/water/grade 约束；禁止手写全平 profile 绕过 solver；
6. 将 `CorridorDesign.profile_mm` 转为 `RoadProfile`，立即执行 `validate_shape()`；再以 `entities()` 生成的实际 road entity 执行 `validate_entity()`；失败就拒绝该 profile 并保留具名原因，不能降级成“看起来可用”的平面 profile；
7. 生成后的 `RoadProfile.start/end`、采样数、宽度、最大填方、entity 中点、rotation、ground height 必须逐项与 edge/node/实体一致。

实现应是小的生产 helper，便于测试；不要复制一套与 `road_alignment` 不同的 solver。

### 5.3 拒绝与记录

至少显式覆盖并保留以下六类拒绝：

- 端点不是当前 plan 的节点 / scene 不匹配；
- 非水平轴、奇数长度、长度不足或超长；
- 宽度、肩宽、max fill、max rise 超出合同；
- 天然高程缺样或毫米值不合法；
- 真实 slope/fill/protected/water 约束无可行 design；
- 设计结果与 entity 的 endpoint/center/elevation/shape 合同不一致。

最小 helper 失败必须返回结构化 `TerrainError`（含 edge id 与拒绝类别）；生产规划不得吞错。若某个 zoned base edge 无法合法 profile，保留该拒绝事实并让调用方按现有错误策略处理，不能无声把 edge 当作合格 frontage road。

## 6. 最小非空 fixture（不可省略）

先写红测，再写实现，再恢复绿测。fixture 必须调用生产函数，不得只构造一个与生产无关的手工 `EntityInstance`：

- 一条真实 base edge，至少 64m，两个 `RoadNode`，真实毫米高度场，合法 zoning mask；
- `RoadEdge.profile.is_some()`；
- `plan.entities()` 恰好一条对应 road entity；
- `road_profile.validate_shape()` 与 `validate_entity()` 全绿；
- `frontage::stations()` 对该实体返回 `Some`，并至少形成一个合法 station/candidate site；
- 序列化→反序列化后 profile 仍存在且 bytes/字段一致；
- 扩展道路回归：`extensions` 仍能消费旧 profile，且不会重复 base edge。

至少六个负测分别命中 5.3 的拒绝类别，且断言错误类型/关键词，不得只断言“is_err”。

## 7. T3 离线闸门（T4 前置）

T2 实现与 fixture 后，先只跑离线：

- `road_profile`、`road_alignment`、`road_network`、`road_extensions`、`auto_roads`、`frontage`、`frontage_supply`、`parcel_geometry`、`services`、`development`、`landcover`、`landuse_ledger`、`zoning` 相关定向测试；
- 生产路径测试：base edge profile → `entities()` → `frontage::stations()` → candidate site；
- serde 旧 JSON 兼容、extension/base entity 不重复、结构/写集/禁止项静态检查；
- CLI build 只在离线测试全绿后进行，随后读取质量报告 JSON 的 `status`，不能只看 exit code。

T3 交付条件：所有 required tests/build exit 0，所有 JSON 结果 `status="pass"`，无 fail/notObserved/未授权 N/A；失败必须非零退出并保留原始日志。T3 未全绿不得跑世界。

## 8. T4 后续授权边界（本轮不执行）

T3 全绿后另行确认再消耗**最后一次成功 1km 额度**，只生成固定 `b01-1km-profile-candidate`，不得跑 512m 或 6×10km。只比较：

- profile base road 数；
- frontage road IDs / station 数 / roadside sites；
- urban homes / shops；
- development plan / realized buildings；
- `scale-audit.json.status`；
- `quality-report.json.status`。

若 profile road 增加但 frontage 不增加，或任何质量报告 JSON status 非 `pass`，立即停在 partial，不再扫参数、不改 zoning 语义、不试 zone-follow-roads。

## 9. 证据交付清单

必须在新目录交付：

- `report.zh-CN.md`：明确写 T0/T1/T2/T3 状态与未授权 T4；解释 OPT-B 为什么合法、原 `extensions` 假设为何错误；列出六类拒绝和所有未观察项；
- `write-set.json`、`baseline.json`、`intent.json`、`checkpoint.json`；
- 红→绿测试、cargo/build/CLI 日志与机器可读 summary；
- 最小 fixture 输入、输出、serde round-trip 摘要、profile/station 绑定证据；
- `review-receipt.json`：`status` 必须是 `partial` 或 `delivered_for_inspection`，`signedOffBySelf=false`；不能自宣 B 通过；
- 实际账目：cargo/build/CLI/driver/world 分开；不估算不可恢复次数；
- 若被 T2 设计阻断，交 `opt-a-request.zh-CN.md`，不得留下半成品生产修改。

## 10. 硬停止条件

触发任一项立即 checkpoint + report，不得继续消耗世界/CLI：

1. 需要新增 `RoadNetworkPlan` 权威 profile registry 才能让生产消费者工作；
2. 需要改变 zoning/reserved/natural/protected/water/需求/尺寸/reachDistance/ratio；
3. profile 只能通过重复 extension edge 实现；
4. 最小合法 profile fixture 经过两次不同诊断仍无法成立；
5. T3 required 测试失败但没有新诊断；不得把“跑满额度”当成修复；
6. 发现历史目录或旧台账被写入；
7. 发现无法从记录证明本轮使用的是当前源码/当前构建；
8. 需要启动 512m 或 6×10km，或需要提前消耗最后一次 1km 额度。

## 11. 现实裁定目标

本轮不是为了制造更多“通过项”，而是确认以下业务链是否真的闭合：

`base RoadEdge → authoritative RoadProfile → RoadNetworkPlan::entities() → frontage::stations() → candidate site`

若闭合，T2/T3 可交独立检查；若不闭合，保持 `partial / T2 blocked`，并明确请求 OPT-A 或回到 B01 任务重定义。无论结果如何，都不得宣称 B 或 C 完成。
