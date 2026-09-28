# G-1b step1e 合成验收审查（2026-09-19）

## 裁定

部分修复接受，现有精确测试日志为PASS；但任务要求的完整工具验收仍needs-changes，不批准自然诊断。并非哈希问题，也不否认本次实现进展。

检查员只读源码、日志、JSON和哈希，未运行cargo/自然/carve/realize/bake。独立复算清单11/11、receipt5/5匹配，payload为日志原始字节子串；生产前缀81685字节/SHA256 ba6deb959c3e3b913ed2c65a887f4860b1cddf505cfd2ae2fb71758997224d34保持。HEAD fb36dc4f，staged为空。

## 接受的改进

connection替换前验证权威身份、长度和唯一性；非connection的完整字段比较；北南候选分别追加基线；四个合成案例before/after确有数值断言；临时目录预约检查AlreadyExists与内容保持；整数毫米；raw与frontage夹限分列；新证据文件绑定完整。人工8000mm完全覆盖填方可用于基础优先级测试，但不是G9环内填方来源的等价回归，不能把“性质一致”扩大为机制覆盖完整。

## 必须修复

### P1：raw变化仍冒用了生产支撑拒绝类别

`G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/worldgen/auto_roads.rs:10550-10620` 的strict案例断言frontageClampedMm前后都2000，却按raw 8000→2390标existing_support_changed。生产frontage检查比较夹限后的整数毫米，不是raw。这里至多是raw_composition_changed；该点支撑采样应判unchanged，不推断整个候选通过。

必须分别计算rawChanged与supportSampleChanged，不能由输入的changed布尔写失败类；缺失值禁止unwrap_or(0)。新增exact断言：strict raw不同/夹限相同、不报support-change；open完整before/after夹限不同、才报该点support-change；protected下前后自然值相同。

### P1：winner实现没有完成上轮修复

同文件10187-10213仍将全部最高priority完全覆盖者列为fullTargetWinners，没有比较同priority target大小，也没有检查graded_influence/full_target分支是否生效、natural floor是否决定结果。blend仍只是满足环内条件的潜在列表，没有实际决定输出的区分。

必须增加独立反例：同priority的2390和3000只后者决定full-target；同target并列全列；自然下限高于两者不归为carve决定；无graded influence时不能声称full-target分支已生效。以生产数值为基准核对诊断。分清几何覆盖、优先级参与者、分支是否生效及最终数值来源；不需要扩大生产语义。

### P1：自然缺失仍被假造为给定数值

同文件10784-10786：g1b_probe(1.0,999,999,...)直接给了自然1.0，因此不是缺失输入测试。探针natural参数是f64，本身也不能表达missing。应让测试侧采样边界接收Option/Result或查人工小栅格；缺格/非有限值应给明确拒绝及null数值，不能伪造0或1000mm。此项与missing owner属于不同测试。

### P2：x268覆盖控制仍用winner代替几何，并且南点用了北候选

同文件约10680处循环两侧z却均调用gated_n_after；且“winner不含connection”不能证明connection未覆盖（road C会压住它）。北控制配north列表、南控制配south列表，直接断言outside_x=1且full_cover=false，再单独检查赢家。

### 执行保护覆盖边界

已有BundleOutcome即时过期/活时钟测试可以接受，但不能代替实际诊断发布路径的晚过期验证。应把诊断结果发布经同一测试侧门函数，测试“探针后/序列化前后发生过期”时正常结果不发布、incomplete=true；保留可序列化的失败记录。无需引入真正realize或witness。

## 一次性补完任务（不重开设计/台账）

继续原已授权cfg(test)实现修复，以上每个缺口对应一个命名清楚的精确合成回归，再运行总验收；正常失败允许修复后精确重跑，日志独立编号。无需每次重新申请，不准扩大过滤器、启用ignored自然入口或改生产文件。人工合成函数求值允许；冻结世界自然/carve回放、measure/access/final terrain/realize/bake为0。

补一个最小“纯blend旧高填方被候选full-target覆盖”的人工案例，与现有完全覆盖旧填方案例并列，核对生产值及来源记录；不要求复现真实6502/6987，也不读取自然世界。

新回执加要求到测试名的对照，不用单个PASS概括未测试分支。旧step1e文件不覆盖；必要勘误随新交付绑定，不另开说明轮。以上通过后再复核是否放行自然诊断。B仍partial，C不推进，无真实增量。