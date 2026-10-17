# V4.1 B01-SUPPLY-R4：仅补CLI独立来源审核闭环

## 1. 只剩一项

先读`docs/collaboration/b01-supply-r3-inspection.zh-CN.md`。C3旧parcel保护、C2内部声明链、此前A/B/C1已验收，禁止继续改这些算法。本轮不追求24、不改自然/carve/fill/坡度/旧支撑，不重开R6。

仓库仅G:/difyllmwiki/GlyphWeave，保留dirty与旧证据，不启动agent、不Git写操作、不改F盘/A冻结材料，不签收B/C。

实际漏洞：CLI quality-report仍调用verify_with_style，该旧包装固定Undeclared；v3因此在独立复核中被拒。cargo check编译通过不能证明这个运行路径。

## 2. 最小实现

- 在core提供供CLI使用的最小公开审核门面，接收已验证WorldManifest/scene等必要输入并从可信原输入推导effective seed、精度、来源；不要从产物自报finalSource/seed授权，不广泛公开AlignmentContext/ZoningMask或规划内部类型。
- CLI quality_report_command改走该门面。内部继续复用verify_with_style_declared的**来源重规划+plan对照+final支撑审核**；不能调用trusted_record叶子绕过源重建。
- 生成和CLI两端使用同一推导规则：world.seed XOR scene.seedOffset、同精度/scene origin/水体参数及effective style。明确SuppliedRaster若当前CLI没有合法来源入口，不得自动猜测为该模式；保持fail-closed。
- 保留旧无声明v3入口拒绝的行为，不恢复seed0或抽样fallback。v1/v2、A身份和已有独立审核不退化。
- 同步world-quality-gate路径（它调用quality-report）；不改gate阈值，不把质量缺额隐藏。

## 3. 必须的真实CLI往返测试（本轮最重要）

新增/更新自给TempDir小集成：

1. 一份合法v3 authored输入，非零world seed和非零scene.seedOffset（不要只让两者抵消成0），自然小场景，状态预算1–数个即可。无需任何frontage成功，也无需gate全绿。
2. 只bake一次同一fixture，记录原manifest、派生有效seed、来源类型；生成后实际调用CLI `validate-world` 和 `quality-report`。
3. 正向：quality-report能够完成独立源重建、输出正常审计/缺额报告，**不再因Undeclared错误退出**。全质量fail可保留，不能将内容fail与来源接线错误混淆。
4. 同一fixture的源绑定负例：错seed/offset或源plan篡改明确拒绝，不能仅错误发生于无关缺字段；必要时同步普通revision/effective声明，使测试真正到达被测来源绑定层，不能绕开已有保护。
5. 记录旧公开无声明v3 API仍拒绝；不用为此再烘焙。legacy小单元或已有定向测试即可，不全套重跑。

不要只再调用core helper传正确context就说CLI已经完成。至少一条实际CLI子进程正向结果必须交付。

## 4. 范围、预算与停止

白名单：
- `bevy/crates/core/src/worldgen/road_network.rs`：最小manifest-bound公开门面及测试。
- `bevy/crates/core/src/worldgen.rs`：必要共享源推导接口，不改地形/事务。
- `bevy/crates/cli/src/main.rs`：独立审核调用点。
- 新`bevy/crates/cli/tests/frontage_supply_source.rs`（或既有对应小测试文件）。
- 新`docs/collaboration/b01-supply-r4-cli-result.zh-CN.md`；R3结果追加勘误即可。
- 新`bevy/work/collaboration/ds-b01-supply-r4-cli/`，create-new。

不再改auto_roads/frontage_supply/landcover等已验收业务算法；确有不可避免接口参数变化先给最小理由，禁止顺手重构。

最多**一次**最小合法尺寸≤512m TempDir bake用于整个正负例复用；正式世界生成0次。定向测试每项120s、首次必要编译300s；禁止80时钟扫描、128追加、carve诊断、全core/CLI/workspace、release、大图/浏览器。

新回执清楚区分本轮执行与R2历史勘误：本轮冻结扫描/诊断为0，历史1次单独说明，不复制到本轮budget。旧receipt/hash/世界全部保留。

交付源码hash、生成及真实CLI命令/exit/log、有效seed/offset、正负例准确拒绝原因、bake次数、其余内容缺额。doc-tree:check/git diff --check通过后停止等复验，不提交Git、不启动agent、不自行签收B。
