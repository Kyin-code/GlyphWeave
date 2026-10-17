# B04-C1-R1 独立审查（2026-09-17）

## 结论

**部分验收：T1、T3、T4，以及 T5 本轮 8/12m 道路误报修复通过；T2 仍 needs-changes。B 整体保持 partial。**

这不是否定整轮成果。不要重复已通过的整改，也不重开 R6。16m 道路是执行者主动披露的后续生产能力缺口，另列 B02-W1，不把它伪装成此次已完成。

检查员只审查、跑定向测试/独立只读探针及写文档；无业务源码修改、无子 agent、无 Git 写操作、无 F 盘改动、无大图/浏览器巡检。HEAD 仍 `fb36dc4f9fdd38e6aa9c5a89676a65b96e5001b1`。

## 1. 证据与独立执行

证据目录：`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b04-c1-r1-20260917/`。

- 回执 SHA256 `2d3e9c123af477c27df01181e9a40995d9e6cbfd59a8f4b6fb0eaf2271ea570f` 匹配；递归读取到的 **22 项 path/hash/bytes 绑定全部匹配**，详见 evidence-review.json。
- C1 旧世界的 **18 项文件 hash 全部匹配旧回执**；新旧两份发布 manifest 的 JSON 内容分别相同，未发现改 seed/需求/输入；详见 old-world-integrity.json。
- R6 模块 hash 仍为 `fb5dd93504e29acd609d07643e9276081a30dd536f7a2ae4ec7fa06027baaa34`，未改。
- 独立运行 core morphology：**24 passed**（3.77s）。
- 独立运行 core worldgen::road_network：**9 passed**（2.07s）。
- 独立运行 CLI morphology_compile：**2 passed**（4.04s），测试确实使用自建临时世界并清理，不再依赖历史 work。
- 独立运行 baked_audit 的 quality_report_rebuilds_development_and_rejects_forged_ownership：**1 passed**（5.36s）。
- 检查员未重复 CLI 全量 41 项；该数是执行者日志报告，不冒充独立复跑。
- `road-replay.rs` 从两份新世界重新读入实际 natural/final chunk、scene 实体、原始 manifest、计划与分区，重新编译 effective 并直接调用生产 `verify_with_style`，不是读取 status 当作通过。
  - compact：road pass、34/34、maxDesignHeightErrorM=0、failures=[]。
  - village：同上。
  - 详见 road-replay.json。只读复算，无写回 world、quality 或 gate。

测试命令均使用 `--manifest-path bevy/Cargo.toml`，core 的筛选分别是 `morphology` 和 `worldgen::road_network`，CLI 分别是 `--test morphology_compile` 和 `--test baked_audit quality_report_rebuilds_development_and_rejects_forged_ownership`，统一 `-- --test-threads=1`。

## 2. 认可的修复

### T1 严格解析

frontage 容器克隆后仅替换 requests，未知键不再丢失，随后交严格消费者拒绝。新增的程序/场景/请求未知字段、错误版本/类型及合法省略测试独立通过。保持这条实现，不重建 schema 系统。

### T3 比例来源

publicSpaceRatio 不再直接生成 ruralPastureRatio。无显式 residual 程序保留 legacy 无程序，有程序而缺独立牧草地字段则拒绝；有效文档 revision 已更改，未覆盖 v1 世界。此行为与本轮任务允许的保守降级一致。

### T4 可移植回归和旧消息

新测试自建 TempDir、实际 bake、实际 CLI 正负审核；不再缺目录直接返回。旧 baked_audit 改动是消息匹配，真实篡改/拒绝断言仍保留，独立通过。

### T5 审核范围错用的修复（8/12m 范围）

源码显示，生产整平带与运输足迹并不相同，原审核却对整个足迹要求精确设计高程。现 carve 与审核共用整平范围，足迹内仍核对 fill/cut。定向合法坡地与扰动测试通过，两个实际世界的生产审核独立重算通过。不是单纯把报告失败改成通过。

范围限制：这不代表 16m collector 已能正确施工，也不代表 frontage 已实现。报告中的“任何合法 graded road 在非平地都无法通过”过于绝对；应改成“这两份 12m 控制世界及对应范围不一致机制会产生误报”，不能排除其他地形/宽度本来能通过。

## 3. 剩余确定性缺陷：显式院落没有参与缺省 footprint 推导（P1）

位置：`bevy/crates/core/src/settlement/morphology_compile.rs:267–268`、`:363`、`:390`。

当前逻辑先在所有 requests 外，用 profile 默认 insets 算 footprint，再在每个 request 中保留显式 parcelInsets。结果是“字段没有覆盖”，但**依赖这些字段的推导仍然用了旧默认值**。

检查员 `inset-probe.rs` 直接调用生产 compile 与 parcel_geometry.boundary，使用 low-density-residential + storefront，仅提供数量/搜索合同、不提供 widthM/depthM：

| 输入院落 front/side/rear | 实际缺省建筑 | 实际完整地块 | 问题 |
|---|---|---|---|
| 缺省（8/8/26） | 10×16 | 26×50 | 正常基线 |
| 显式 8/2/26 | **10×16** | **14×50** | 应使用实际院落推导 22×16，从而得到 26×50；现仍使用 side=8 的旧推导 |
| 显式 2/2/2 | **10×16** | **14×20** | 当前静默接受，低于 profile 完整地块宽16/深28下界；真实推导应先得到22×46，再由 storefront 深度合同拒绝，不得偷偷沿用旧 footprint |

以上三个输入当前都 compile 成功。后两例 notes 也没有地块偏离说明（当前偏离检查仅在原请求含 width/depth 时执行）。最关键的是第二例：正确结果 22×16 本来就在 storefront 合同内，不能以“profile 默认不可用”解释。

这仍是原 T2 的“部分显式/部分缺省”范围，不是新增要求。修复应按 request 先解析最终生效 insets，再逐轴推导缺省 width/depth；显式 footprint 永远保持，不为无关缺省轴执行不必要推导。来源表说明实际用了用户院落，不能仍写默认 side=8。

## 4. 16m 道路：已披露的后续缺口与同路径记账问题

`worldgen.rs:3995–4002` 的 graded 横向半宽仍为7m；16m 路面半宽8m。因此当前并未完成 collector 的真实完整路面工程支持，执行者这点披露属实。

源码还显示新覆盖失败记录位于逐格循环内部（road_network.rs:1353、1376），会对每个采样重复追加同一“band narrower”字符串；后面的 realized_edges 仅看误差/fill/cut，未同时排除 band coverage 失败。**这是源码检查结论，未声称本轮独立跑通16m生产正例。** 下一轮应每条路只记录一次结构失败，失败路不能计作已通过实现。

检查员曾尝试构造外部16m独立 probe，但 plan 接口为私有，编译未通过，未执行；不计入验收证据，也没有为此修改生产可见性或业务代码。

## 5. 测试/报告准确性小修正

当前新增 graded 测试的 +1mm 位置是 `transverse_half - 1`：12m 案例为横向6m，不是整平外缘7m。当前审核源码确实包含7m，但该测试不能描述为已经覆盖7m边缘。新增用例函数也只显式构造 +1mm 和 -0.2m，没有单独可辨认的 overfill 注入。下轮宽路测试应明确区分路面边缘、整平外缘、过渡带、超填/挖方；给真实坐标和对应判据，避免多个反例实际只命中同一设计误差条件。

## 6. 项目状态和下一步

- A 保持既有签收状态，本轮未重新认证 A 全链。
- B04-C1-R1：T1/T3/T4/T5限定范围认可，T2 待一次小修正；不再重复前轮已通过的整个审查队列。
- B02：16m 工程支持待 B02-W1。
- 两份新 CLI 世界仍 frontage 0/24，strict gate 六项失败；只是道路误报消除，不能叫整世界通过。
- 原自然512m案例仍 unresolved，冻结保留，不重复四档扫描。
- B整体 partial，C不推进。

下一任务：`G:/difyllmwiki/GlyphWeave/docs/collaboration/ds-v41f-b04-c1-r2-b02-w1-task.zh-CN.md`。
