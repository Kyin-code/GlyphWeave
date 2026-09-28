# B-01～B-05 首轮独立审查

日期：2026-09-16（Asia/Shanghai）。检查员：主Agent。只检查，不调用/恢复子agent、不代写业务修复、不提交Git。

## 1. 裁定

**整体维持 partial；本轮整改结论 needs-changes。不签收B，不推进C。**

这是可保留的第一轮实现，不是B-01～B-05功能全部完成。停止最大图/浏览器是符合准入条件的正确决定，不需要为补齐报告而现在重跑。源码中已有真实新增能力，但“engineeringStatus=pass”超出了现有证明范围：存在新程序来源校验遗漏、道路层级选择逻辑问题、profile输入漏检，以及尚未接入的服务关系模块。

A的历史签收不撤销；但B代码没有自动继承A的工程验收。

## 2. 证据与本轮实际运行

WORK_BASE仍为 fb36dc4f9fdd38e6aa9c5a89676a65b96e5001b1。

回执：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b-run/final-receipt.json。
SHA256核实为 97d2e11ad2594158ae2a523b80e471bc7899a2a4d7c5d874c29f2ca96394ea9a。回执中changedFiles/resultDocs共23个文件hash均匹配当前文件；这不等于所有产物都有完整运行身份绑定。

检查员证据目录：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b-20260916/。

| 操作 | 实际结果 | 证据 |
| --- | --- | --- |
| 读取回执/源码hash/小场景报告 | 匹配，内容缺额存在 | evidence-review.json / .log |
| 重算历史workspace日志 | 484 passed / 0 failed / 3 ignored；402仅core | evidence-review.json |
| 本轮定向Rust parcel测试 | 12 passed | cargo-parcel.log |
| 本轮定向Rust residual测试 | 2 passed | cargo-residual.log |
| 本轮定向Rust auto_roads测试 | 7 passed | cargo-auto-roads.log |
| 本轮Node collaboration | 252 passed | node-tests.log |
| 原策略/篡改策略、profile类型探针 | 复现漏检，见第4节 | source-probe.rs / source-probe-results.jsonl |
| 残余地表中心采样重叠检查 | 当前产物未发现中心点重叠；不是全多边形并集证明 | surface-overlap-sampling.json |
| 拒绝原因与caseId引用检查 | 记录实际缺额、3个悬空caseId | content-details.json |

本轮没有重跑完整workspace、Vitest/lint/typecheck、CLI bake、完整gate、release构建、最大图或浏览器。历史日志只记为日志复核，不认领为本轮新运行。探针位于检查员work目录，链接现有B core rlib并调用生产公共API；首次链接选错serde构建变体而编译失败，第二次使用匹配变体成功，两个日志均保留。未在生产源码植入测试或修复。

## 3. 五阶段真实完成程度

### B-01：几何模块可保留，端到端尚未满足

独立定向测试确认24个显式地块的纯Rust fixture通过，四朝向/legacy等测试通过。但真实CLI产物 requested=24，planned=realizedWithAccess=5，unfilled=19；住宅5/16，店铺0/8。yard unresolved=289m²，所以既不是24/24，也不是所有新庭院已经完整实现。

失败原因集中在wrong_purpose_or_protected、occupied_or_road_support。需要检查真正可用的道路前沿/完整地块落在目标用途内的长度和候选顺序，不能仅将原因概括成“再多加路就行”。

### B-02：宽度/层级接口已增加，需求供给和两级实网未证明

实际小场景新增5段、zone unserved=0，但frontage只有4/24，店铺仍0/8。说明zone中心/相交目标满足仍不能替代24个真实地块供给。

auto_roads的targets仍从LandUseZone生成；并未建立任务要求的住宅、设施、农田实际缺额驱动的完整目标链。层级标签和width测试不能作为两级真实拓扑已完成的证明。另有第4节的明确优先级/用途问题。

### B-03：有真实新表面，但不是用途执行闭合

实际表面public_open_space=1293m²、pasture_ground=32386m²；真实值与报告相符。landUseLedger实现138384m²、未实现123760m²，总计262144m²。

**这是512×512米，不是1km×1km。** B01/B02/B03的quality-report与manifest均显示512×512。原任务允许≤1km，所以尺寸本身不是违反上限；错误在报告把它叫“1km控制”，必须纠正统计分母。

当前实现主要是对FutureDevelopment/AgricultureCandidate逐米扫描后按比例增加两种表面，不是完整的城市剩余地分配、共享农路扩展、村落农田空间组织。不得把这一功能增量称为B-03所有要求已实现。

### B-04：profiles和服务关系尚是配置层

morphology只有builtins/resolve/validate等，worldgen只调用validate_manifest，没有实际参数展开调用；service_relations只有defaults及其单元测试，没有访问图路径计算/审核生产调用。

因此“尚未完整展开”应明确为：profile数值当前尚未驱动生成，服务关系也尚未接入实际路径验证。文件顶部“audited against final AccessGraph”和结果文档“实际仍由最终AccessGraph审核”不能作为已实现事实。

48条表格记录确实存在，但不是48个已核实、可定位的独立案例：登记表没有逐条URL/来源章节或可定位边界，部分条目只是整座城市/外围区域；profile引用中us-houston、hk-mid-levels、jp-kyoto不在48个caseId中。需要修复引用并补可追溯来源，不能仅靠分类列恰好5/4/3认定研究覆盖完成。本轮只核查本地索引完整性，未联网复核其城市事实。

B04的village-field-2存在baked generation输出，但没有quality-report.json；不能用它证明profile和服务关系经过独立质量审核。

### B-05：回归执行了，最终候选未建立

历史workspace各target通过数为44+7+26+1+4+402=484，3 ignored属于core。402不是全workspace总数。

最大图/release/240块巡检not_run与实际交付一致；没有理由要求它现在跳过小场景门槛补采。小场景quality-report属于独立报告操作，不等于完整world-quality-gate；现有报告/目录未见本轮完整gate和连续进程采样证据，不能据此宣称B进程性能预算通过。

## 4. 必须修复的具体问题

### F1 [P1] residualLandProgram 未绑定原manifest

位置：G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/settlement/development.rs:249–260；G:/difyllmwiki/GlyphWeave/bevy/crates/cli/src/main.rs:1687附近。

audit_landcover直接将 `record.residual_program` 复制给expected再比较。现有farm_pattern在CLI另有farm_program::audit_source核对manifest，但新增residual_program没有对应源校验，代码搜索没有其他生产源绑定入口。

检查员以真实B03的manifest/spec/development/landcover读取到原策略urban=0.1、rural=0.2，将内存中的plan策略改为0/0且不改manifest，调用生产audit_landcover仍成功。正例也成功。证明的是**这条生产审核API无法约束原需求来源，CLI也缺补充绑定**；本轮没有执行“重写整份世界后完整gate绕过”，不能把单API反例夸大为已复现全链攻击。

修复要求：像farm_program一样从冻结manifest/scene独立取得residual策略，核对缺省/显式值/场景/版本，并覆盖scene内与独立发布文件的同源关系。不能只比较两份可一起改的派生plan，不能在验证时把缺失策略补成“正确”策略。

### F2 [P2] 道路优先级与最终用途不一致，tier允许越过用途限制

位置：G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/worldgen/auto_roads.rs:70–83、518–545。

候选遍历urban/rural时 `best_priority=min(...)`，但每次有收益都覆盖 `best_purpose`。一个同时对urban/rural有收益的候选可能priority仍urban，实际tier却按后遍历的rural选择farm_access。另 `tier_for`在找不到符合purpose的tier时退回“只匹配role”，忽略显式purposes限制。

这是源码路径审查发现，不声称已对完整小世界做因果干预证明“所有8m段都由此产生”。需要先加入urban/rural同时受益及不允许purpose的精确单元反例，再修复优先级与purpose的原子选择/确定性tie-break。道路真正供给24地块仍需之后的端到端证据，不能修两行就宣布B-02完成。

### F3 [P2] morphologyProfile 错类型被当成缺省

位置：G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/settlement/morphology.rs:205–208。

`get(...).and_then(as_str)`使显式42、对象、null都进入Ok(None)。检查员生产API探针已复现三种输入全部accepted。缺省可以关闭，新字段存在却类型错必须报明确错误，否则用户以为选了profile，实际却走legacy。

结合B04未接线，本轮应先修输入拒绝并诚实注明配置层状态，再接真实参数展开；不能以继续新增profile条目代替调用链。

## 5. 不升级为本轮已证实阻断的风险

landcover::realize目前用所有surface的顶点floor建立occupied，而不是面覆盖；这依赖现有农田等逐米网格细分。若后续压缩网格为大三角形，内部覆盖可能漏掉。当前B03中心采样未发现重叠，因此不宣称当前已出现地表碰撞；列为网格合并前必须验证的风险。

不使用估算替代数据：本场景两类新表面合计33679m²，新增顶点134716、三角形67358。最大图扩展有序列化/内存风险，不能未经测量就宣称性能不退化；但也不因此立即开启大图。

## 6. 下一步顺序

1. 修F1与F3并更正报告事实；为F2建立确定性反例并修复。
2. 在当前512m输入上诊断完整地块可行前沿，完成16住宅+8店铺的CLI实现与真实庭院、collector/local至少两级实网；不降低需求/改用途/换seed刷通过。
3. 达到上述可行小闭环后再继续B03剩余用途与B04真正展开/服务路径；阶段partial不可混写工程全部通过。
4. 最后才B05新runtime/最大图/浏览器。保持A原证据和原manifest，不重复全workspace/大图来掩盖局部未接线。

可转交的整改要求：G:/difyllmwiki/GlyphWeave/docs/collaboration/ds-v41f-b-review-r1-task.zh-CN.md。

本轮没有Git写操作、没有子agent调用、没有修改业务代码。检查时仅见历史GlyphWeave preview PID60644/8099，未触碰；当前进程快照不能证明历史期间从未启动别的进程。
