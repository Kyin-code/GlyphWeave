# B Review R1 独立复验报告

日期：2026-09-16（Asia/Shanghai）。主 Agent 仅检查，未调用/恢复子 Agent，未修改生产业务代码，未执行 Git 写操作。

## 1. 裁定

状态：`needs-changes`；B整体仍为`partial`，不签收B，不推进C。

三项合同修复的方向基本正确，但R1-C真实512m小闭环仍失败，因此不能把本轮称为B-01/B-02已完成。F1/F2/F3需要保留，先修复实测阻断后再继续B的内容展开。

## 2. 独立复验

仓库：G:/difyllmwiki/GlyphWeave。实际HEAD仍为`fb36dc4f9fdd38e6aa9c5a89676a65b96e5001b1`，A标签不变。

回执：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b-r1/receipt-r1.json；报告提供的SHA256与当前文件一致。回执changedFiles、inputs、resultDocs和命令日志绑定可读取，未发现已报告的文件hash不一致。

本轮独立运行：

- `cargo test --manifest-path bevy/Cargo.toml -p glyphweave-core -- --test-threads=1`：405 passed、3 ignored、0 failed；耗时123.44秒。
- parcel相关core过滤测试：12 passed。
- residual相关core过滤测试：2 passed。
- automatic road相关core过滤测试：7 passed。
- `node --test scripts/collaboration/*.test.mjs`：252 passed、0 failed。
- 原B01/B02/B03产物和诊断表重新读取，实际小场景数据与R1文档一致。

执行过一次检查员证据脚本时因检查目录名称写错导致检查脚本未生成；这不是项目失败，也不作为结论依据。随后完成了不依赖该脚本的逐文件读取、hash核对和定向测试。该临时错误不会写入worker交付结论。

## 3. F1/F2/F3结论

### F1 residual来源绑定

R1新增`residual_land::audit_source`并在CLI接入，设计上已经针对上轮真实反例补上来源链；结果文档记录了删除、错version、未知scene和同步篡改负例。当前只独立复跑core整体和已交付CLI日志，没有重新执行每个CLI负例；因此将其定为“交付日志支持、未在本轮逐项重放”，不是无条件重新签名。

后续必须确保任何公开的landcover audit入口都不能仅接收被审计plan来推导expected策略；manifest/source应由调用边界提供。

### F2道路选择

结果文档与源码显示已把priority/purpose/tier改为原子选择，并加入用途不匹配负例；16m节点半径修复也有合理的工程原因。定向automatic-road测试通过。真实小场景仍没有形成collector/local两级实际拓扑，因此F2只能签收“选择器单元合同”，不能签收“道路供给完成”。

### F3 morphology输入

显式number/object/null/array拒绝及悬空caseId修复方向正确；core测试通过。仍需后续把profile真正接入生成链，不能停留在resolve/validate。

## 4. R1-C实际失败与阻断

两次512m生成均在frontage之前失败，未运行gate：

1. 第一次旧8m节点半径与宽路共享端点支撑冲突；该缺陷已修复。
2. 第二次在`station 45`因`required 7120mm exceeds 7040mm cap`拒绝。该失败属于现有fill/corridor工程合同的真实拒绝，不允许通过增加fill上限、改平自然地形或换seed掩盖。

因此没有以下证明：

- 16住宅+8店铺真实CLI requested/planned/realizedWithAccess=24/24/24；
- 24个完整parcel及yard surface均落地；
- collector+local至少两级实际road entity/profile链；
- 小场景独立gate通过。

检查员接受“按两次预算停止、没有第三次生成”的流程决定。当前不要跑最大图或240块浏览器，因为前置小场景准入还没满足。

## 5. 报告事实与边界

R1正确更正：历史workspace总数484 passed / 3 ignored（402只是core）；B03样例是512×512m、262144m²；B04 profile和service relation尚未实际接线；B04 village-field没有quality-report；engineeringStatus保持partial。

B-01实际旧/新输出仍分别是5/24和4/24，不应用纯Rust fixture的24/24替代CLI证据。A的质量gate六类内容缺口、两项许可证未解析和A固定证据继续保留。

## 6. 下一步裁定

下一轮只做“512m真实小闭环修复”：先用诊断表定位station45走廊失败来自道路宽度、候选方向、相邻支撑、自然高程还是parcel占地，再在不放宽原fill/坡度/保护/水体合同的前提下修复候选或明确证明需求与地形冲突。

小闭环成功后，才允许把profile参数接入现有frontage/services/road生成，并将service_relations改为真正消费AccessGraph；然后再运行B03剩余用途实现。最大图、release和240块浏览器必须最后进行。

下一步任务书：G:/difyllmwiki/GlyphWeave/docs/collaboration/ds-v41f-b-review-r2-task.zh-CN.md。
