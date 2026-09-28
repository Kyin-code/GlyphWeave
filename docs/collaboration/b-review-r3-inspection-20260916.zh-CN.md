# B Review R3 独立审查：平地组合正例成立，联合预检仍未完成

日期：2026-09-16（Asia/Shanghai）。主Agent仅检查与写文档；无子agent调用、无业务代码修改、无Git提交。

## 1. 裁定

整体仍为 **partial / needs-changes**，不签收B。

可保留成果：合成全urban平地中，通过生产函数组合建立了24个临路地块、实际庭院与16m→12m连通道路链；这不是只填JSON数字，也没有被谎称为generate-world CLI成功。

不能接受的扩大表述：当前模块还不是已完成的有界联合规划器，也不是可独立信任的工程审核入口；原自然案例没有本轮新联合候选运行证据。不能写成“原自然案例已经由R3联合搜索过但找不到24”。

## 2. 核对与实际复验

证据目录：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b-r3-20260916/。

- HEAD：fb36dc4f9fdd38e6aa9c5a89676a65b96e5001b1。
- receipt-r3.json实际SHA256：bcb6a0777b4703623b4905613ec60cebee160b4c12896a0b5b78f68cf7ad7f5f，与报告相同。
- 回执7项源码/文档/日志绑定全部匹配；R1八个源码文件hash仍匹配。
- 独立运行 `cargo test --manifest-path bevy/Cargo.toml -p glyphweave-core road_parcel_feasibility -- --test-threads=1`：**5 passed / 0 failed**，测试执行1.63s。
- 历史core日志确实记录410 passed/3 ignored/0 failed；本轮未再跑完整core，不将410算作本轮独立重跑。
- 编译检查员work中的小探针，链接已有当前R3 core rlib调用生产API，未在项目源码插入反例或修复。
- 无bake、gate、release、大图或浏览器巡检。当前只见历史preview PID60644/8099，未触碰。

证据：identity-review.json、targeted-tests.log、audit-probe.rs、audit-probe-results.jsonl、audit-probe-build.log。

## 3. 正例通过的准确范围

新增5个测试确实通过。正例人工构造flat raster、全urban zone、base road，依次用append_one接入16m短段和12m长段，再调用frontage生成16住宅+8店铺，development/landcover/access相关函数确认数量与yard。

这证明的是 **内存中的、零高差合成输入上的生产函数组合**。区别必须保留：
- 16m与12m道路是测试预先指定的相连段，不证明新的需求驱动层级网络搜索已实现；
- 所有道路/场地都在0m平地，没有覆盖非零工程填方后terrain与natural不同的情况；
- tests调用built_artifacts保留对象在内存中，没有持久化/重新读取产物的测试；
- 删除yard patch、改派生建筑入口、地块重叠负例调用对应生产验证函数有效，但不是所有变异都通过新joint入口验证；
- urban边界测试只是调用boundary计算并检查数值越界，没有调用真实用途/选址拒绝路径；文档中z224“北侧”、z160“南侧”标签也写反，实际应为z224南侧/z160北侧。

建议保留这些测试，在同一管线上补最小非零工程与重新读取正例，不重造整套CLI或侵蚀系统。

## 4. 问题 J1：256检查的是扩展段数，不是搜索状态

G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/worldgen/road_parcel_feasibility.rs:118、163。

代码以plan.extensions.len()限制并报告joint_candidate_states。这只是在检查候选结果有多少条扩展道路：一个结果有2段，与尝试过1个还是10000个候选没有必然关系。

没有候选循环/状态计数/合法最优见证保存，也没有对原自然输入的实际运行入口调用。evaluate_style_candidate只是调用一次原plan_with_style，然后汇总一个结果；rg仅找到本模块定义/测试，没有CLI或生产管线调用。

R3 work目录只有4个文件：core日志、tree日志、diff日志、receipt。没有新的自然案例输入/候选/固定node/拒绝原因/供给统计记录。原自然案例状态应写：overall unresolved，R3 joint evaluation not_run；旧R2孤立走廊扫描不能充当R3联合评估。

这不是要求写全局优化器；原任务要求的是最多256个真实联合候选状态的评估与证据，目前尚未做到。

## 5. 问题 J2：固定节点审核漏掉base road与缺失节点

位置同文件29–58行。audit_node_heights只遍历extensions的request端点；fixed校验使用is_some_and，当节点不在这个扩展端点表内时直接略过。

检查员真实公共API探针用128m全平地、单条base road（levelM=0，无extensions）验证：

| 输入 | 实际 |
| --- | --- |
| fixed中base节点[32,64]=0 | Ok，返回节点表为空 |
| 同一实际base节点0m，但fixed要求8000mm | node audit与joint audit都Ok |
| fixed要求存在的节点[99,99]不在plan | node audit与joint audit都Ok |

所以“固定节点不能被重规划覆盖”的保证只覆盖了碰巧位于扩展端点集合的节点。需要从可信base plan的nodes/edges/实际profile导出义务，核对身份、存在性、高程及连接；不只比较新request里相互一致的数值。

探针是最小零需求例，用于证明固定节点校验漏检；不是声称伪造出了24栋成功世界。完整世界独立gate未被本轮攻击或重跑。

## 6. 问题 J3：新入口未完成来源与工程前后高程边界

位置同文件100–184行。

- audit_joint_plan默认传空fixed map；evaluate_style_candidate没有向它传冻结义务；服务program也传None。若被当通用manifest预检，可能不覆盖原设施义务。
- audit_joint_plan从传入plan直接生成实体，再从相同plan生成development，没有在该边界重放原style/road plan或核验natural来源。检查员将最小plan.natural_heightfield_hash改成forged-natural-source后，joint audit仍返回Ok。
- 它确实调用production development/landcover/access函数，但 **audit_joint_plan本身没有corridor重设计/append_one/road_network独立源审核**；这些只在测试构造或上游plan_with_style里发生。不能把上游可信调用的局部性质说成任意传入plan都经过了完整独立审核。
- evaluate_style_candidate用同一个terrain先规划道路，再直接交给实体访问/yard执行；没有单独的natural/final区分、工程地形阶段或等价的已验证final terrain输入。平地0m正例掩盖了这一步缺失。
- development/frontage等返回status/failures，不是所有失败都用Err表达。新JointAudit丢弃这些状态及原因，仅返回部分计数。Ok只能表示函数完成，不可作为engineering pass。access_connected统计所有connected，access_required只统计required；若用二者相等代表所有required已连接，需要改成requiredConnected/requiredUnconnected或按ID逐项检查。

修复可以选择“受信上下文的内部汇总器”或“对外完整验证入口”，但必须说清调用前置条件和上游验证来源。不要为了修这些问题再做通用证据平台。

## 7. 下一步

保留平地正例，停止继续加平地测试数量。下一轮完成：
1. 固定base节点/来源/状态传递的明确合同；
2. 一个非零但合法的工程地形正例，使用生产工程步骤产生final terrain并验证真实yard/入口；
3. 对冻结自然案例实际运行有界联合候选评估，记录执行了多少状态、最好合法网络/地块数与拒绝原因。无需强求24，未找到仍为unresolved；
4. 合成正例产物落盘/重读再审核，不能只留测试断言。

依旧不以合成正例代替原自然案例、最大图或B签收。不跑最大图/浏览器，不扩大C。

任务书：G:/difyllmwiki/GlyphWeave/docs/collaboration/ds-v41f-b-review-r4-task.zh-CN.md。
