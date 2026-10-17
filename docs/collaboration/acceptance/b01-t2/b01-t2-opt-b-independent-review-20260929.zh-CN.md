# B01-R1 T2 OPT-B 独立检查与续行裁定

日期：2026-09-29。本轮检查员只读产品源码、备份及交付 JSON，独立运行一次测试编译；没有执行测试、生成世界、运行产品 CLI、修改产品或执行 Git 写操作。

## 结论

维持 OPT-B，不需要因为测试字面量同步转向 OPT-A。授权下一个窄修复轮完成机械同步与已发现的生产链兼容修复。当前状态应为 **partial / implementation-present-unvalidated**，不是“T2 生产完成、只有白名单阻断”。T3 未通过，T4 未授权。

原任务书 SHA256 实测为 05D098A881F74B5E27C9F4FA69A17E76381016A898E0D0806FFCE9AF58E39305。两个修改源码和 road_alignment hash 与 checkpoint 相符；三个 pre-edit 备份 hash 与 baseline 相符。前后所有历史目录零漂移未在本次全面重验。

## 检查发现

### F1：编译站点报告不符（实测）

检查员运行 `cargo test -p glyphweave-core --lib --no-run --message-format=json`，exit 101，5 个 E0063，不执行任何单测/世界。

在 G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/ 下实际缺 profile 的位置：

|文件|行|数量|
|---|---|---|
|settlement/agriculture.rs|661|1|
|settlement/development.rs|930、941|2|
|settlement/landcover.rs|1013|1|
|worldgen/road_parcel_feasibility.rs|538|1|

不是报告声称的 8 文件/28 处。没有原始输出支持另 23 处；不能断言原因是造假、缓存或源码漂移。须更正并附旧输出（若可找回），不能凭记忆补造。

### F2：拒绝原因被丢弃（确定的代码路径缺陷）

G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/worldgen/road_network.rs:710–737 返回 Result<Vec<String>, TerrainError>，Err 被收集为字符串；同文件:470 调用 `attach_base_edge_profiles(...)?;` 后丢弃 Vec。随后继续写 planned_connected_network。因而“拒绝具名保留”的声明不成立：在 helper 内收集不等于生产产物持久化。

### F3：profile 会改变既有 base 识别，供给合同尚未适配（静态必达风险，未跑世界）

G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/worldgen/auto_roads.rs:1746–1765 的 base_only_plan 用 road_profile.is_none() 筛基础边。基础边现在带 profile 后会被剔除；全部带 profile 时返回 supply phase needs an ungraded base road domain。

只改筛选也不够：G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/worldgen/road_parcel_feasibility.rs:51–79 的 trusted_base_contract 明确拒绝任何有 profile 的基础实体，且采用固定 level_m 的基线合同。必须以真实 profile 建立不可变基础合同、验证节点约束，不能清空 profile 或跳过审核来通过。

### F4：没有按合同执行实际实体 validate_entity（确定的缺项）

G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/worldgen/road_network.rs:627–702 做了 solver、validate_shape 和部分几何比较，但没有对生产 entities() 得到的实际实体调用 validate_entity。注释不能替代该验证，也不能提前写 locked by test。

### F5：高程与朝向退化输入处理需要补强（代码边界）

同文件:660 用 saturating_mul(1000).abs() 检查高程，不与 RoadProfile 的 -32000..=256000 mm 域等价，极端负数存在 abs 溢出风险；:695 的差值容差判断不能拒绝 NaN。需 checked arithmetic、明确域及有限/四向一致性，并调用真实实体校验。未在本轮运行这些输入，不称已动态复现。

## 执行与任务书自身的问题

- 停下来避免未授权改文件，本身不是懈怠或违规，不归因于动机。
- “机械字段同步”与“广泛结构重写”此前界限过窄；原文已允许必要测试同步，不能让这种变更反复导致整轮停工。
- RoadEdge 新字段会影响 Rust 字面量编译是可预见的；下次改公共结构前先全量枚举字面量及生产判据。
- 新文件仍未编辑时可以现在记录 pre-edit hash，不是回溯造证；已编辑文件不能覆盖最初 pre-edit 记录。
- 仅库编译不能称“库正确”；生产实现不应先于非空消费测试宣称完成。
- 同 solver 不代表基础路现有平台高程不变。必须检查节点、路侧站点支撑及 final terrain 与新 profile 相容，不得放宽工程合同。

## 证据和账目

证据目录：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-t2-continuation-20260929/

- test-compile.stdout.jsonl：原始结构化编译输出。
- test-compile.stderr.txt：exit 101，5 errors，23 warnings。
- inspection-process.json：检查员独立 compile-only 调用 1；世界/产品 CLI 0。
- inspection-facts.json：受检源码 hash/bytes、五条错误及检查边界。

执行者 cargo 6 目前为其声明；交付目录未含六次原始 cargo 日志，不将此次检查员运行算进它的六次，不将声明升级为独立核验事实。目录“7 文件”指顶层；递归实际另有 3 个源码备份，共 10 文件。

下一任务：G:/difyllmwiki/GlyphWeave/docs/collaboration/dsh-b01-t2-production-chain-continuation-20260929.zh-CN.md
