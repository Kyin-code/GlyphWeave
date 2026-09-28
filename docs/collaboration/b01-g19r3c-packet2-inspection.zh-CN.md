# G19R3C packet2 独立检查：编译阻断与支持判据未闭合（2026-09-20）

## 裁定

**partial / build-broken / structural-support-unverified**。接受DSH明确撤回本轮验证背书，但不接受“唯一剩余目标已实现、补一个括号即可闭合”。源码除了语法损坏，新判据仍有可复算的支持漏检。

当前core SHA256为8c7ba92f6f976887f24321f3c27e630677bbe01e65cd894f478d165726408cfc，与packet2-receipt/progress一致。CLI仍99c90e46…，access与auto_roads保持前次绑定。上一版11/11及7次CLI仅适用于上一版；不能转用于本轮，也不能把当时测试绿色称为整体合同验收。

独立证据目录：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g19r3c-packet2-20260920/。

## F1：当前生产crate确实无法编译

检查员实际执行一次cargo check -p glyphweave-core --lib，退出101。编译器指向G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/worldgen/engineering_connections.rs的verify_bundle（约6229行）未闭合，末尾诊断到7641行；返回VerifyReport结束后、cfg(test)前没有闭合函数。

这不是按字符串花括号数猜测。修复必须遵循Rust语法及函数/模块边界，不能靠文件末尾随意补括号；本检查未修改生产文件，也未在损坏树上运行旧CLI充当新版本验证。

## F2：固定两测点不保证段内支持完整

structural_support_from_frames约3370行起，每段仅取t=0.25与0.75；无按长度或部件支持边界增加测站的逻辑。任意段长的两次测量，不是最大空间步长受限的走查。

隔离探针把当前函数及其几何依赖原样抽出（仅换行归一化；错误类型用最小脚手架，支持计算未改），两个同层宽2m支持面在x=[0,4]与[6,10]，路径x=1→9：

- 两测站x=3、7都有支持；函数返回(band=2.0, unsupported_endpoints=0)。
- 中点x=5落在2m空洞内，原station_band_width返回(0.0,false)。

证据为隔离helper运行，不是当前不可编译生产crate或CLI已经构建并放行；它足以否定“两测站保证段内空洞被量到”的算法命题。记录见该目录isolated-support-probe.log及extraction-binding.json。

## F3：joined允许自身替自己豁免端点宽度

端点判定carried本来检查required_band，但joined只检查frame.distance_to≤0.05与高差≤0.25，没有检查另一真实部件身份、相接处方向净宽或实际承托连续性。端点在原平台内时，同一原平台即可让joined=true，即使carried=false。

隔离探针：平台仍中心(8,1,8)、6×1.6m，路径(6,1,8)→(10.96,1,8.06)，且frames只有这一个平台。

- 原端点测量宽1.480108281678835m，小于要求1.5m。
- 新helper返回最小测站宽1.5101104765777316m、unsupported_endpoints=0。

不存在第二相接部件，也没有真实楼梯过渡；这是自身豁免。此处差值虽小，但不依赖容差混淆：判据已经正确算出端点带宽不足，随后joined另行放行。

此外，每站在两端点高度取所有owner frames的较大覆盖，并未携带相邻part身份；不能把“试了两个高度”直接写成“已证两相邻部件真实交接”。这一项为静态约束缺口，不冒称额外全CLI反例。

## F4：新测试自身存在不成立的正向断言

r3c_structural_endpoint_transitions使用平台x=[5,11]、路径5→14，却断言段内band≥1.5。t=0.75为x=11.75，已经在平台外。原样函数实际返回(0.0,1)，因此该断言不成立。

正确处理应是把“缺支持段拒绝”与“实际相接合法正例”分开，提供完整承托的正例几何；不能强迫算法把11.75当成有支持来迎合测试。之前检查员固定反例不得改写。

当前测试函数静态列举为12个，不是“原11+重加2=13”：原11已经含r3c_band_measurement_assertions。应按具名断言映射及实际test结果汇报，不能按恢复次数加总。

## 状态与预算

progress.stage标记未验证正确，但closed中仍保留“跳过过渡顶点”“CLI已通过”等旧版项，又把未经验证的新规则追加为closed，不能作为当前完成状态的机器依据。下次合法执行时区分prior_verified_at_hash、implemented_unverified、remaining，不另开本轮确认/勘误包。

worker累计cargo20/20已耗尽、CLI13/30。检查员本次单列cargo1（预期诊断失败）、rustc1、隔离探针执行1、CLI0；自然/ST18/默认世界0。检查员未修源码、未回退worker文件，当前损坏版另存于独立审查目录。

不自动追加额度。恢复任务草案在G:/difyllmwiki/GlyphWeave/docs/collaboration/dsh-b01-g19r3c-recovery-task-draft.zh-CN.md，需用户明确批准或转发后生效。建议只追加cargo8次（历史20保留，总28），CLI继续消耗原剩余17次、不新增；不追加自然或新设施权限。

A冻结、B partial、C不推进。
