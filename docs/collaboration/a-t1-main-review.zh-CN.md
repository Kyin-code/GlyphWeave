# A-T1主Agent验收记录（候选级通过）

日期：2026-09-08。任务范围：固定frontage入口与最终实体/图一致性，当前不是里程碑A完成。

## 1. 既有差异审查

HEAD=6020fbe2713e8c79c1b2934b244402dc93e1df14。7个Rust脏文件保留，不盲目恢复，不猜测作者。

四个未认领文件：engineered_placement.rs、frontage.rs、road_network.rs、services.rs（均位于bevy/crates/core/src/worldgen/）已独立与HEAD比较。总控将HEAD/当前内容复制到独立review目录，以rustfmt --emit stdout --edition 2024 --config skip_children=true规范化两份副本；去掉输出文件名标题、规范换行后逐字节相同。

证据 G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/review/a-t1-format-audit/summary.json，四项formattedEqual均true。这支持本次差异是格式变化的判断；不证明改动作者身份，也不将这些格式变化归为Terra/Luna功能实现。不修改原文件、不格式化原仓库。

其余已有生产变化集中：worldgen.rs删除构图后frontage实体回写；access.rs限制固定frontage进入generic facade fallback。后续拒绝可选候选分支会重建graph，再派生accessSurfaces；development_plan包含roadside site实体，受protected_access_ids保护。此次审查未发现这条路径在派生访问面后再次改写固定frontage实体。

## 2. 补齐证据

旧CLI测试只检查篡改后非零，并在恢复scene后直接读取已有quality-report。需补恢复后重新执行与具体拒绝原因，避免崩溃/旧报告通过测试。
核心覆盖需补四yaw、非零origin、正负毫米、固定失败与可变fallback对照，不以旧单北门测试泛化全部方向。
白班分工见docs/collaboration/day-a-t1-review-task.zh-CN.md。Terra只写access.rs测试；Luna只写baked_audit.rs目标测试；总控统一串行cargo。

## 3. 运行资源与限制

开始前未见cargo/rustc运行；有15个历史glyphweave preview进程，保留不按名称杀进程。记录见bevy/work/collaboration/review/a-t1-processes-before.json及a-t1-memory-before.json。小图测试不是可推广性能基准，不据此报告倍速或最大图预算达标。
禁止worker竞争编译，不跑完整workspace/最大图/release/新server。测试运行和最终hash待收候选后登记。

## 4. 当前状态

accepted_candidate；A-D1保持accepted_tooling；未暂存/提交/推送。源码→最终最大图运行集成及全域HTML均未由本轮证明。

## 5. 最终冻结与独立测试（2026-09-08）

- worldgen.rs SHA256：b1a6d8c45991cbafe0a9380149ab5c448419eccd9574d9496213e72ba520bb54。
- access.rs SHA256：e2ba76fe02b979ac877c8152ccd878b63f3bcd58d5825217bc4b2a86234b6b04。
- baked_audit.rs SHA256：0fc8ae0ffeced2936b7a84f068357fddcb01686f8276dce8d6b564c2f3589690。

主Agent只读审查后统一串行运行下列cargo测试，全部匹配1项、退出0：

1. core::worldgen::access::tests::fixed_frontage_cardinal_yaws_signed_mm_nonzero_origins_keep_reachable_contract（内部16组合）。
2. core::worldgen::access::tests::fixed_frontage_cardinal_yaws_signed_mm_nonzero_origins_keep_blocked_obligation（内部16组合）。
3. core::worldgen::access::tests::fixed_frontage_entrance_is_never_reassigned_even_when_another_facade_is_reachable。
4. core::worldgen::access::tests::generation_selects_another_facade_but_never_rewrites_authored_door。
5. core::worldgen::access::tests::planned_failed_routes_are_not_deleted_as_optional_candidates。
6. core::worldgen::frontage::tests::negative_millimetre_frontage_is_not_raised_by_legacy_quarter_metre_rebasing。
7. core::worldgen::road_network::tests::roadside_front_doors_face_the_assigned_road。
8. CLI baked_audit::local_height_frontage_bakes_real_city_buildings_and_rejects_forged_plans，512m真实bake及反复独立quality-report/gate；编译阶段7.75秒、测试执行30.55秒。时间只作本机日志事实，不作性能基准。

具体命令/日志/hash见 G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/review/a-t1-acceptance.json。
主Agent另存测试后core/CLI源码及Cargo输入文件hash清单和debug测试二进制hash；这不是最大图runtime收据，也不是可重现构建签名。

## 6. 已证明行为

- 四yaw × 两组非零origin × 正负1354mm支撑，在可达/堵门两类fixture各16组合下，固定实体整体JSON不被修改；普通generated对照能切换可达门，固定门则保留entrance_clearance_blocked与失败义务。
- 最终序列化实体重建graph一致，访问面由同一图/高程生成并可校验。固定失败不被删，局部128m完整高程content hash前后不变。
- CLI入口和roadPoint各单字段篡改返回退出1且真实源码诊断明确；不是panic或任意非零。该诊断来自提前ownership budget重建拒绝，而非声称存在专用anchor错误。
- 每次CLI quality-report前清除本测试临时目录中的旧报告，恢复scene后必须重新运行成功，再核对图及frontage、设施、保护、道路和土地账本与原基线一致。测试期间原scene不会被审核程序自修。
- 512m原8栋frontage需求、实际5栋、住宅缺额3、服务设施4、保护区changedHeightSamples=0保留。完整gate仍fail且恢复前后failures相同。原plan伪造与+1mm支撑篡改继续拒绝。

## 7. 接受范围与后续

A-T1 accepted_candidate：固定入口/最终图一致性窄改动及测试可以进入集成候选。本轮新增仅测试，不再次改生产逻辑或放宽gate。初版旧逻辑红测已有worker记录；本轮主Agent未重编译旧生产版本，不将其说成新独立红测。

不代表里程碑A完成：仍须当前最终候选全workspace、最大6×10km与独立gate、确定性/稳定ID、生态性能等价、全域HTML资源和发布可复现性等统一验收。局部自然hash不外推全域自然层/保护区。

下一步应冻结A-D2全域HTML观测接口和资源预算，再将HTML实现与巡检分文件交Terra/Luna；旧四个格式差异作者仍未知，但规范化比较已消除本轮额外行为变化疑虑，不盲目恢复。未经RUN槽不重跑最大图。

本轮保持唯一仓库G:/difyllmwiki/GlyphWeave，F:/modenmapbox未修改；HEAD不变，无暂存/提交/推送。Terra与Luna写集未交叉，无新服务，无新依赖。
