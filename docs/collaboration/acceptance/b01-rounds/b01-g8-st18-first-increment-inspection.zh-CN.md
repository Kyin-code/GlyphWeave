# B01-G8 单次实验独立审查（2026-09-19）

## 结论

**accepted-bounded-negative-with-progress**。接受两态road append/196格接点匹配、station18 no-cut解除、随后existing_support_changed拒绝以及测量零预测增益。没有真实增量，候选完整final/development/landcover/access/yard仍not_run，B partial/C不推进。

本次不需要补跑G8，也不继续提高B/C。批准下一项窄范围只读支撑合成诊断，禁止修改carve/审核策略。

## 1. 独立核验范围

- receipt 13063bytes，SHA256 22e2874ac3dae72eeef68c10fa38fda38a83f4678f894dca6cb77c072e64714c。
- 当前17项path/bytes/hash绑定匹配；从G7记录核对7项其它生产源hash及7冻结W1输入，均匹配。
- 旧G7提案文档因要求追加勘误而hash变化，旧回执本身保留；台账7项仍匹配。旧源码绑定需要按准确版本披露，见§3。
- run.json与run.log JSON一致；四份raw trace文件与嵌入内容相同，32行rawSample/rawDetail逐项对应，设计高度与对应BC profile[station]匹配。
- 32行建筑12×10、朝向、support范围及自然极值与已验收离线极值交叉吻合；非仅相信五标志。
- 两态AB85/BC113点，profile与design.profileMm逐点一致、request匹配，mesh存在；逐米坡度与9m端平台必要条件通过内容检查。两态接点196格/0不匹配/0缺失。
- rule-table及run-reservation的BLAKE3独立重算与运行记录相同；预约在自然基线重建之前以create_new写入，同路径再执行会拒绝。日志及预约支持本轮单次执行记录；这不是对所有系统历史进程的无限范围证明。
- 独立--exact运行g8_preflight_schema_contract和两项G6-R1合成回归，3 passed/0 failed。没有调用自然harness/solver/design/append/infer/realize/bake。

## 2. 核心进展

两态BC station18实测2390，北/南自然max2307/2390，no-cut最大缺额-83/0。此处通过的是building+2m支撑自然必要检查，不是后续yard/access已实现。

station18两态、两请求、两侧共8次attempt变为existing_support_changed：
- 北侧(268,171)：6502→2390mm；
- 南侧(268,188)：6987→2390mm；
- conflictingEntity均为本态新增BC道路。

全部32次首拒绝：20 no-cut、12 support-change。新增station18证据说明该现象不是仅station90有，但不能扩大为所有地块普遍不可行。

精确before贡献者以及为什么建筑连接carve导致已有高填方采样被低值覆盖，还需要实际窄范围来源追踪。不能因rawDetail指向BC就认为BC自身生成了6502/6987，更不能认定历史35路已破坏或审批放宽保护。

## 3. 必须追加的非阻断勘误

1. 结果§2末尾“故4次attempt”少计一态：每态4，两态station18总8；32次总表20/12正确。
2. station66并非全未动：R1仍147，R2为150，因C从3000到3110后西端坡度传播在此开始支配。R2首点缺额为289/5，完整最大缺额855/999，不能沿用R1的292/8及858/1002。文中“42/66未动”的全称句应改；st42仍1105。
3. receipt.supersededBindings将G6的294054bytes/1994ef49…误写成G6-R1源码绑定。实际G6-R1回执绑定330309bytes/319bdceb5b0a6d414b494a5e471d2dd6344a5175272dcc9bb4d64b9ccd97eb3b；G7 source记录也指该版本。旧G6、旧G6-R1要分别标注，不能串版本。
4. 当前#[cfg(test)]前实际UTF-8字节数为**81685**，SHA256仍是给出的ba6deb959c3e3b913ed2c65a887f4860b1cddf505cfd2ae2fb71758997224d34。81677是字符串字符长度口径，不是字节长度。改为bytes=81685，或另外保留characters字段；前缀hash本身没有算错。

旧G8 receipt/run一律保留，在后续文档/新回执追加，不改原文件或为此重跑。

## 4. 下一步

执行ds-v41f-b01-g9-local-support-provenance-task.zh-CN.md：只在固定G8/ST18-R1计划上，追踪station18北/南两处冲突及2个整平带边界控制点；重建一次，生产点采样一次诊断harness，不测量新的地块、不realize、不bake，不改参数/政策。

目标输出可明确指向贡献实体/具体carve与选择路径的证据，而非再次“层可能覆盖层”的静态描述。解释清楚后先提交修复设计选项，不实现合同变化。B仍partial，C不推进。

本次检查员仅只读与3合成测试、新审查/任务/回复/证据写入，无源码修改、agent、Git写操作或F盘写入。
