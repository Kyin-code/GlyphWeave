# B01-SUPPLY-R3 独立审查（本轮）

## 结论

- **C3 完整旧parcel保护：accepted-subtask。** 上轮旧13m²被误算为新增的反例修复，关闭此项。
- **C2 Rust内部生成与声明来源链：accepted-subtask。** seed0与17点猜来源已删除，声明贯通到实际供给与编码器。
- **C2 CLI独立审核闭环：needs-changes。** CLI quality-report仍调用旧无声明入口，v3必被拒绝；这是剩下的一处跨crate接线，不需要重做算法。
- 整体partial，B不签收、C里程碑不推进。A事务/B滚动/C1共享审核及R2/W1/R6不重开。

检查员没有修改业务代码、启动agent、Git写操作或改F盘；无bake、无80时钟扫描、无冻结128扫描/carve诊断、无浏览器/最大图。

## 1. 独立复验和身份

HEAD：`fb36dc4f9fdd38e6aa9c5a89676a65b96e5001b1`。

证据目录：`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-supply-r3-20260918/`（本轮检查员输出目录名）。

- 回执完整SHA256：`a59c1ad61445fe59aac495d1fa1606db257877203e7b85b6802da00e3a8af06e`，符合用户给出的前缀。
- 21项path/log/hash/bytes绑定匹配；W1登记7个旧文件未变。
- 三份旧receipt分别保持`69ff2c9b…`、`d84a59be…`、`b3c39e92…`；R6源码hash仍`fb5dd93504e29acd609d07643e9276081a30dd536f7a2ae4ec7fa06027baaa34`。
- 定向独立执行（统一`cargo test --manifest-path bevy/Cargo.toml -p glyphweave-core`）：

| 筛选/用例 | 结果 |
|---|---|
| production_v3_entry | 2 passed，4.45s |
| supply_final_matches_the_production_encoder_bit_for_bit | 1 passed，7.66s |
| undeclared_natural_source_is_refused_by_the_v3_entry | 1 passed，1.75s |
| a_supplied_raster_edit_outside_the_old_probes_is_not_overwritten | 1 passed，6.30s |
| frozen_baseline_parcels_are_protected_and_new_parcels_must_be_resolved（明确只读ignored） | 1 passed，8.27s |
| transaction_guards_reject_regression_identity_yard_and_unrealized_plans | 1 passed，0.04s |

合计7个定向用例，不是全套回归。C3 fixture只重建基线及面积，不是128候选求解。

为独立公开API探针执行了一次`cargo build --offline --manifest-path bevy/Cargo.toml -p glyphweave-core --lib`（18.74s）；本地原rlib尚旧，初次链接得到旧版行为，已明确废弃，最终探针重新链接本次构建的当前rlib，以下结论只用重建后结果。没有把cargo check等同运行时复验。

## 2. 已通过的C2/C3范围

### C2 core

bake_world以`world.seed XOR scene.seedOffset`及精度建立AuthoredSeed，经plan_with_style_declared/append_declared传给baseline与candidate实现；内部verify_with_style_declared也带声明。没有来源的v3拒绝，SuppliedRaster不再靠17点猜来源。

内部入口测试用非零sceneOffset验证baseline，候选真实接受用显式SuppliedRaster；两种证据区分明确。接受这一结构性贯通，不要求把自然困难场景强行制造正收益。它尚不证明CLI独立审核已接齐，见第3节。

### C3

session保护集合来自全部baseline development assets，而非仅frontage；独立production派生fixture得到21实体/21parcel，旧缺额9+4=13m²，frontage信用仍0/0。

- 旧缺额不变+新yard0：允许增量。
- 旧修复+新yard缺13：拒绝。
- 旧parcel之间总量不变转移缺额：点名增长parcel并拒绝。
- owner变化、旧资产映射缺失、未运行landcover审核、第二轮新增地块缺额增长：定向守卫测试覆盖。

landcover.rs未再修改；非yard unresolved_area仍为总量限制，不声称它已有逐来源保证，本轮不扩展这一项。

## 3. 唯一阻塞：CLI仍走Undeclared审核包装

### 代码链

1. `bevy/crates/cli/src/main.rs:1623`，quality_report_command对road plan调用公开`road_network::verify_with_style(...)`，参数里没有自然源声明。
2. `bevy/crates/core/src/worldgen/road_network.rs:1965–1986`，该旧包装调用verify_with_style_declared并固定传`NaturalSource::Undeclared`。
3. `auto_roads::append_declared`对v3+Undeclared明确拒绝。
4. `verify_with_style_declared`与NaturalSource目前均pub(crate)，CLI crate无法直接调用；生成器内部接线成功并未解决独立CLI crate的调用。
5. world-quality-gate先validate再进入quality-report，因此不能把cargo check或bake内部审核通过当成发布闭环通过。

这是**所有通过此CLI道路审核路径的v3输入都会遇到的缺失上下文**，不是自然地形无解或内容缺额。

### 独立反例范围

`cli-audit-entry-probe.rs`读取旧W1自然/final/scene只读，在内存将style改为合法v3+显式supply（maxCandidateStates=1），manifest.validate和auto程序parser均通过；调用与CLI相同的公开verify_with_style得到：

```
invalid baked terrain: version 3 supply requires a declared natural source (authored seed or supplied raster)
```

详见`cli-audit-entry-probe.json`。它是**公开审核入口反例，不是一份新烘焙v3世界的端到端测试**；该错误在来源重规划、比较候选与record之前就发生，旧record/v3输入差异不是本次错误来源。结合实际CLI调用点，可确定当前CLI缺接线。

需要面向CLI提供最小manifest-bound公开门面（或等价可信context构造），从已验证原manifest+scene推导源，再复用内部声明审核；不能把trusted-record共享叶子作为CLI绕过来源重建的替代，更不能给旧包装静默fallback。

## 4. 文档/回执口径（非代码阻塞）

R3正文说明本轮没有128扫描/carve诊断，且证据日志区分了R2勘误。但R3 receipt顶层budget仍写frozen128RescanPerformed=true/runs1、carveDiagnosisRerun=true/runs1，容易误读为本轮重跑。下一回执将“本轮实际执行”与“R2历史纠错”分开；旧receipt不覆盖。

不应写“未重跑时钟测试但不变量必定成立”作为新验证结论；可写保持已验收事务结构、未复测覆盖断言。本轮检查员也未重跑它。

## 5. 下一步

只做CLI来源审核接线与一次自给小世界往返验证。C3与内部source算法不再改，禁止重新自然128扫描、大测、carve重构。

任务：`G:/difyllmwiki/GlyphWeave/docs/collaboration/ds-v41f-b01-supply-r4-cli-task.zh-CN.md`。
