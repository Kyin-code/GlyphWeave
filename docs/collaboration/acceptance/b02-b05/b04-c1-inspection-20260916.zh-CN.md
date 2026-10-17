# B04-C1 独立审查（2026-09-16）

## 结论

**needs-changes；有效接线进展保留，B 整体仍 partial，不签收 B，不推进 C。**

本轮检查员只读源码/已有世界、执行小型测试与独立探针、写审查和任务文档。没有修改业务代码，没有启动子 agent，没有 Git 写操作，没有新生成、gate、release 或浏览器巡检，没有改 F 盘。R6 已验收子项不重开。

基线 HEAD：`fb36dc4f9fdd38e6aa9c5a89676a65b96e5001b1`。工作区包含此前未提交的多人/多轮改动，不能把整个 dirty diff 都算作 C1。

## 1. 独立复验范围

证据目录：`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b04-c1-20260916/`。

- worker receipt SHA256：`81231baa3d3f3e178847aa54bee258091734bced3a967b3b538fa3bcfd72e6e3`，匹配；递归找到的 28 项 path/hash 绑定全部匹配。身份一致不等于内容正确。
- `cargo test --manifest-path bevy/Cargo.toml -p glyphweave-core morphology -- --test-threads=1`：18 passed，4.53s。
- `cargo test --manifest-path bevy/Cargo.toml -p glyphweave-cli --test morphology_compile -- --test-threads=1`：2 passed，0.92s。本机已有 work 产物，确实执行了验证；干净克隆问题另见下文。
- `cargo test --manifest-path bevy/Cargo.toml -p glyphweave-cli --test baked_audit quality_report_rebuilds_development_and_rejects_forged_ownership -- --test-threads=1`：exit 101，1 failed，旧错误消息断言不匹配。
- 离线 core lib build exit 0，供独立 Rust 探针链接；`compiler-probe.rs` / `compiler-probe.json` 直接调用当前生产 compile、frontage parser、parcel boundary，无业务源码注入。
- `evidence-review.json` 记录两份已有 CLI 世界的 frontage、road audit、gate、自动道路实体与参数。没有重复生成它们。

## 2. 可以保留的进展

1. Rust profile 编译器及字段来源表已存在，不再仅 resolve profile ID。
2. 主生成消费者共享 effective style；原始 manifest 保留，发布 effective 文档；CLI 从原输入重新编译比较，不直接信任发布报告。
3. 无 profile 的 revision 分支保留旧语义，有 profile 才绑定 effective 内容。未在本轮重新运行 A 全链，不能声称 A 全量重新验收。
4. 真实函数级形态集成测试通过；两份 CLI 世界实际道路宽度、田块/地表输出有差异，不只是改 profile 名称。
5. worker 明确披露 0/24、strict fail、unsupported 和原自然案例 unresolved，这些诚实状态保留。

## 3. 必须整改的问题

### P1-1：未知输入被编译器静默吞掉（已独立复现）

位置：`bevy/crates/core/src/settlement/morphology_compile.rs:284`，重建 frontageProgram 时只保留 version/requests。

探针在同一原始 style 加 `frontageProgram.unexpectedField=true`：
- 原生产 `frontage::from_style` 拒绝 unknown field；
- `morphology_compile::compile` 成功，effective 中未知字段已消失。

这破坏 fail-closed，拼错参数也可能被忽略而以默认值生成。必须在补默认前验证容器/允许键，或保留字段交给严格消费者拒绝。注意缺省字段是编译器应补的，不能简单提前调用要求所有字段完整的 parser 导致合法缺省也被拒绝。补正向缺省和各层未知键负例。

### P1-2：parcel 尺寸被当成 building footprint（已独立复现）

位置：`morphology_compile.rs:182,217`；真实地块算法 `worldgen/parcel_geometry.rs`。

village-field 的 parcelWidthM=[14,32]、parcelDepthM=[20,48]；编译器直接生成 building 22×34，再加 front=6、side=4、rear=20。调用真实 boundary 得到 **30×60m** 的完整地块，深度超出自身 profile 的 48m 上界。

因此“仅 village 默认尺寸能过 consumer”不等于默认形态语义正确。需要明确完整地块、建筑 footprint、院落之间的关系；用户显式建筑尺寸优先，不得缩小。缺省 footprint 可按明确地块尺寸扣除院落推导，但必须验证正尺寸/偶数/asset 合同，无法表达就拒绝或 unsupported。不要把现有 parcel 统计直接重命名为 building 数据掩盖问题。

### P2-3：publicSpaceRatio 无依据映射为 ruralPastureRatio

位置：`morphology_compile.rs:433`。探针确认 village 同时生成 urbanOpenSpaceRatio=0.14 和 ruralPastureRatio=0.14，后者来源也标为 publicSpaceRatio/profile-default。

此前任务要求接通公共空间/牧草地程序，不等于授权把这两种比例认作同一个语义。应保留显式牧草地输入；没有独立牧草地参数时保留已定义 legacy 行为并如实标记未接入，或采用有明确独立来源的字段。不可虚构研究统计。

### P1-4：CLI 的独立道路工程失败没有被正确诊断

两份已有世界均有 **3 个 grounding.roadProfile 非空的 auto-road 实体**，compact 宽度 [12,12,12]，village [12,12,8]，extensions 均 3 条。`auto_roads.rs:675` 调用 `road_extensions::append_one`。所以“auto_roads 不写 road_profile”与源码及产物矛盾，不能作为修复方向。

更重要的是，quality/gate 不仅报告内容缺额，还报告道路设计审核失败：
- compact 3 条 auto-road 的 design error 约 0.364m / 5.498m / 0.699m；
- village 2 条约 0.364m / 5.498m；
- 两者 maxDesignHeightErrorM=5.497999668121338，roadNetworkAudit.status=fail。

这是**已发布的工程审核失败**，尚不能断言是实际铺路破坏还是审核取样范围/目标计算不一致，也未证明由 C1 引入。必须定位到坐标、采样范围、natural/expected/actual final 和阶段。优先检查 `road_network.rs:1040–1099` 的扩展支撑范围与 profile.height_at 的范围及回退，和生产 carve 语义是否一致；这是诊断线索，不是已确认根因。不能提高容差、忽略 profiled road 或改 gate 为 pass。

frontage 的已有拒绝明细为 wrong_purpose_or_protected 12、natural_support_or_fill_limit 12、existing_support_changed 3、occupied_or_road_support 3。应依据这些实际证据区分缺额原因，不能为了追 24/24 重写道路系统或改变自然案例。

### P2-5：CLI 回归在干净克隆上会空跑通过

`bevy/crates/cli/tests/morphology_compile.rs` 两个测试依赖 gitignored work 目录，缺目录就 `return`；Rust harness 仍计为 passed。当前环境 2 pass 是真实执行，但这不是可移植的自动回归。

改为受控临时小 fixture；必须依赖人工产物的重型测试可显式 ignored 并说明执行要求，但需另有常规、无需历史 work 的正负例。不能把所有测试标 ignored 来代替覆盖。

### P2-6：旧 baked_audit 断言仍红

`bevy/crates/cli/tests/baked_audit.rs:571` 仍匹配旧字符串。development.rs hash 与 R1 绑定一致，支持“不是本轮改动”的披露；不归责为 C1 引入。但应最小修正测试，保留真实篡改与非零退出验证，不能删/skip。当前不能宣布相关工程回归全绿。

## 4. 文档口径修正

- CLI `validate-world` 成功，不等于独立 roadNetworkAudit 或 whole-world quality pass。
- effective 文档审核实际是 serde_json Value 结构/值相等，不是原文件空白/键顺序的逐字节比较。
- 消费者共享一份有效 style 是正确描述；validate、bake、world_revision 都可触发编译，不能声称整个 bake 生命周期只调用一次。此项先修措辞，不要求做缓存重构。
- 六 profile 可用性应在修正 parcel 语义后重新列矩阵，不能继续把“5 个默认拒绝”直接当最终正确设计结论。

## 5. 下一步与边界

执行 `G:/difyllmwiki/GlyphWeave/docs/collaboration/ds-v41f-b04-c1-r1-task.zh-CN.md`。先修本轮已复现的语义/严格解析/可移植测试，再用已有产物定位道路工程失败。R6 已验收模块只读；不重新开启支撑反例平台，不重复自然四档扫描，不跑最大图/浏览器，不要求把原自然困难案例强行变为 24/24。
