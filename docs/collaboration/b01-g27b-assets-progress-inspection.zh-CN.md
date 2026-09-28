# G27B continuation-3资产规则进展检查与接续要求

日期：2026-09-23。结论：**本轮有真实实现进展，接受资产校验局部改进；完整包合同和G27B整体仍partial，授权持续有效。** 不将可核实的代码产出说成“什么也没干”，也不推测执行者是否偷懒。

## 独立核验

源码G:/difyllmwiki/GlyphWeave/bevy/crates/cli/src/context_preview3d.rs从70032B变为82895B，当前SHA-256 beacf7c5fa3b47a95ad1acc134fb8916e0353e696a63cbfe3e2eb41d39fc0179。新增artifacts_of/artifact_rule/check_artifacts/check_producer_assets确实接入export/audit；并非只有未调用的helper。检查员实际执行cargo test -p glyphweave-cli --offline --bin glyphweave，64 passed / 0 failed。此轮检查员成本cargo1、生产CLI0、browser0，不占DSH余额。不声称本轮有生产CLI或WebGL观察。

认可的实现：四资源声明bytes/hash/唯一性与生产者build id；viewer逐字节生产者匹配；HTML与包内模型生成的预期HTML逐字节匹配。这一HTML全字节比较确实覆盖内嵌内容，不因后续16字段列表漏一项就否定其已生效的保护。

continuation-2的manifest/command-log/receipt与上次检查其被重写后的hash相同，未发现本次再次改写。此前封存后重写仍保留为历史事实，不能因新清单自洽而抹去。

## 尚未闭合与测试精度

1. 旧requiredFiles路径仍filter_map字符串后sort/dedup，类型错误或重复会被规范化；四资源artifacts校验不替代六文件manifest/marker固定引用与版本合同。模型完整类型/形状/字段映射、sources全字段、trace三个漏项和有界读取、写失败用例仍按原任务完成。
2. “16字段全量比对”不准确：16项列表没包含heightDisplayConversion；完整性目前另依靠预期HTML逐字节相等。建议整Value比较或由同一字段规范生成列表，避免维护两套字段集；不移除已生效的HTML字节匹配。
3. tampering_only_the_embedded_model_is_refused修改status.entityCount和sources.sceneId，不是所报“schema与来源”；没有同步HTML的artifacts bytes/hash，也未重封manifest/marker。现有测试可能仅因普通hash失配被拒，不能单独证明内嵌语义防线。应同步普通声明/链后再审，断言previewCompleteness及明确producer-page/embedded诊断，或直接调用producer-assets校验并验证其目标错误。其他格式保持干净。
4. 三个负例主要assert is_err，后续补读报告核对阶段与具体拒绝原因，不把任意错误当目标防线成功。viewer测试已同步资源声明与链，属于真实更强的反例。
5. README目前有实际字节绑定，并不意味着其内容已经过生产者语义认证；不扩大为任意自洽文本安全证明。按原格式/披露合同明确其策略。

以上没有新增范围：均属原完整任务及既有验收规则。不要再只修一项trace或文案后要求审批。

## 下一实施段：立即连续执行

继续使用G:/difyllmwiki/GlyphWeave/docs/collaboration/dsh-b01-g27b-complete-chain-task.zh-CN.md及G:/difyllmwiki/GlyphWeave/docs/collaboration/dsh-b01-g27b-remaining-execution-order.zh-CN.md，已有授权不变。

- 将本次测试补强与剩余包字段/trace校验集中实施，避免每一字段消耗一次完整cargo。
- 主实施重点转入build_model有向实体体块、道路保存矩形带、无高度观测层；savedBounds保持世界AABB事实，不二次旋转/scale、不填常数Y。
- viewer绘制实体面，与实际几何相同的射线求交；地形实际索引三角求交，删除nearest-sample步进，隐藏层不参与拾取。
- 一组具名数学测试覆盖90度旋转、非零/负原点、边界/隐藏层、2×2斜面和不共面双三角。
- 不等待本实施段另一次批准，随后完成core保护、最终构建、18项CLI与五类真实WebGL观察，最后统一证据绑定。中间进度可以汇报，但不等同需要等用户批准才继续。

下一次实质进度至少提供几何/拾取代码差异及数学测试actual；若未做到，准确说明具体未完成实现和工具/外部阻断，不重复把64项旧结果计为下一轮成绩。

## 预算纠正

上次检查后剩cargo11，本次cargo-46再用1，**现在剩cargo10，不是11**。G27B累计cargo10/20；CLI9/32（余23）、browser0/7（余7）、aux0/8（余8）。不追加、不重置预算；前置服务与失败照计。新交付日志明确修正，已封存目录不回改。

本次检查员未改产品或worker证据。A冻结/B partial/C不推进；G27B持续执行。
