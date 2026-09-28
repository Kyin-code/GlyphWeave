# B Review R5 独立审查

日期：2026-09-16（Asia/Shanghai）。结论：**三个原反例修复有效；B仍partial，本轮needs-changes**。

主Agent只检查、写文档与运行小探针；无子agent、无生产源码修改、无Git写操作。HEAD仍fb36dc4f9fdd38e6aa9c5a89676a65b96e5001b1。

## 1. 实际验证

证据目录：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b-r5-20260916/。

- 回执4073 bytes，SHA256 a11dc1edb2e2a64832b19dc75d842716d2d95c2fffc55188c46fe143802863a4，匹配。
- 10项源码/文档/日志/自然结果/旧证据恢复绑定全部匹配。
- 独立运行road_parcel_feasibility模块：**7 passed / 2 ignored / 0 failed**，执行3.84s。
- 两个ignored测试会写固定证据路径，本轮未执行；检查日志及实际产物，用独立只读probe验证持久化正例。
- 为probe链接当前R5源码，执行一次离线core库构建（12.78s），不是release或完整core测试；随后编译work目录的独立Rust探针。

## 2. 原三个工程反例已通过独立复验

对R5落盘平地正例的内存副本调用当前生产joint入口：

| 检查 | 实际结果 |
| --- | --- |
| 未改正例 | 24/24/24，requiredUnconnected=0，yardUnresolved=0；读取surface验证通过 |
| [64,160]固定base道路中心final高程+1mm | 拒绝：fixed base road support differs |
| extension.design.roadMesh顶点y+1m | 拒绝：corridor mesh differs from station geometry |
| 删除固定base edge、保留nodes | 拒绝：fixed base edge was deleted |

这些具体修复可以保留，不再要求重复证明相同反例。发展/landcover仍有未开发土地导致incomplete，这是内容状态，不据此否定局部正例。

## 3. 尚未覆盖的原合同：道路完整支撑而非中心单点

位置：G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/worldgen/road_parcel_feasibility.rs:254附近。

代码只读final_terrain.height(required.world_x, required.world_z)——每条base road仅检查中心。没有检查完整道路支撑范围。

检查员把相同+1mm扰动移动一米，仍在同一条固定base road内部：

- [65,160]：沿道路纵向离中心1m；
- [64,161]：道路横向离中心1m。

两个反例均返回Ok，frontage仍pass/24，requiredUnconnected=0，所有工程相关failures未增加。

这不是新增验收目标，也不是指控执行者恶意针对测试；说明修复范围只覆盖了示例坐标，尚未落实“实际完整支撑与计划一致”。应复用road_network.rs中现有完整支撑范围审核，而不是继续补两三个采样点。扩展道路最终支撑也应由同一生产审核覆盖，设计verify只证明natural上的设计，不证明传入final与设计一致。

证据：persisted-probe.rs、probe-results.jsonl。全部变更只在内存副本，不改原落盘文件；本轮没有声称完整CLI gate存在相同绕过。

## 4. 非零持久化确实未完成

R5的persisted-positive-r5目录只有五份平地产物，逐一hash与R4平地包完全相同。没有scene/zones/base/请求及natural/final输入的独立重读包。

R5将persistedNonzeroPositive标为partial是正确且诚实的，不能当成此项交付。内存中的非零工程测试本轮通过，但不能替代完整非零包的重新读取验证。

不要再生成一份不同目录名但相同的平地包。下一轮直接保存现有非零fixture所需最小输入/产物并重读即可，无需完整CLI开关或大图。

## 5. 原自然四状态的准确进展

R5自然评估分支已解析并传入settlementProgram，成功分支改为先生成final再joint；失败extensionCount改null，这些接线改进成立。实际四次仍全部在plan阶段失败，未执行成功分支的frontage/yard审核。

- 8000：原station45约束冲突；
- 7000：extension overlaps road outside a matching endpoint junction；
- 6000/5000：station22的required6880分别超过6000/5000上界。

这些比R4统一改低第一入口的三个立即失败更有意义，但不能据此宣称容量问题已解决。

当前按数组index==0保留第一条start=8000，其他走廊两端统一取候选level。原R1网络还有从既有节点[224,96]进入的另一支路，其端点也必须从固定节点合同取得，不能依赖“数组第一条才固定”的假设。由于本轮提前失败，还没有走到该支路，不能说已经证明所有固定端口保持。

公共evaluate_style_candidate仍传None作为服务program，而测试中的专用自然runner已经传入解析结果；两条路径尚未统一。这一事实应明确，不能将仅测试分支的改动描述为全部生产入口已一致。

## 6. R4历史文件恢复

当前R4 natural-joint-evaluation.json是1143 bytes，SHA256为67666193c600a0adcbcb6120f86e9c3907dbd5c2fc6dcd7022ec39923e1f6694，与原记录完全相同。恢复后的文件身份可确认。

曾覆盖旧证据是已承认的流程错误；不能改写成“本轮始终未触碰旧证据”。恢复后无需重采。根治方式是测试/导出入口使用显式新目录并拒绝覆盖，而不是每轮把源代码里的r4目录替换成r5/r6后继续fs::write覆盖。

## 7. 下一步

只收口已有工作：完整道路支撑审核、非零包重读、固定端口按节点来源绑定、统一需求传递。不要再为原三个反例重跑全部core；不继续扫描固定四档高程来重复相同错误。

本轮不退回“必须马上解出原自然24”，退回的是具体工程漏检和明确未完成交付。原自然场景可继续unresolved，B仍不签收。

任务：G:/difyllmwiki/GlyphWeave/docs/collaboration/ds-v41f-b-review-r6-task.zh-CN.md。
