# 白班A-D1-R2总控验收记录

日期：2026-09-08。最终状态：accepted_tooling（仅A-D1实现与回归范围）；真实运行integration_pending，里程碑A未完成。下文保留首轮问题与收尾过程。

## 已复验候选

HEAD 6020fbe2713e8c79c1b2934b244402dc93e1df14未动。
Terra实现SHA256 1a958b7442a652e057c145f08d5856ebe9895057a1b450103b3392200f221c3b。
Luna测试SHA256 d981202f3baf0d4b21798be9ff0d76c2592de8d9a10e964dff1dadc829383e68。
总控运行node --test scripts/collaboration/check-evidence.test.mjs：34/34，退出0。
日志 G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/review/a-d1-r2-main-tests.log。

R1原5类问题已有实质修正；但缺少R1安全回归的迁移，且独立发现剩余绑定缺口，故尚不接受。

## 独立单变量复验（实际执行）

脚本 G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/review/a-d1-r2-independent.mjs。
原始结果 G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/review/r2-independent-Nr5jvr/summary.json。
完整control通过；下面5个负例全部错误pass/0，脚本自身断言式退出1。

1. 过程stopReason缺失，不能等同于明确null。
2. manifest及两份过程中的sceneId相同，但world仍指向另一个sceneId；当前只比较数量。
3. manifest.world.seed=999，world.seed=7，当前未比较可用身份字段。
4. generation-report.output为world/scenes，不是过程/收据声明的world根。
5. manifest及两过程的widthM=0.5，Rust SceneSpec.width_m为u32，当前只比较有限正数。

以上均更新直接受影响的hash，未用hash不匹配混淆语义检验。均为合成fixture，不是真实世界认证。

## 一次有界收尾分工

Terra仍只写checker和自己的结果/work；Luna仍只写测试和自己的结果/work。不得修改对方文件。

Terra：过程stopReason必需明确null，外层兼容省略；manifest/world可用seed/renderMode身份字段及canonical场景ID核对；generation-report.output精确world根；SceneSpec数值对齐u32/i32，seedOffset大于JS安全精度不得假称精确验证，可标unsupported。保留Rust默认origin/seedOffset=0。不是实现完整Rust manifest语义校验器，不读大scene JSON，不增加字段协议。

Luna：保留34项，恢复R1 sourceSnapshot/hash/必需报告覆盖/format/version/revision/未知状态/路径与junction逃逸/wx拒覆盖/目录伪装输入/JSON超限回归；新增上述5个单变量反例。按具体非pass原因断言，不以全部样例incomplete蒙混。

主Agent：独立复跑两人最终hash候选、上述单变量脚本及必要正向；不写worker实现、不跑Rust/大图、不提交。若这一次有界收尾仍有核心缺陷则停下并记录具体阻塞，不无限返工。

## 信任与范围

本工具校验材料完整性和内部一致性，不认证authority、不证明源码真实编译/进程实际发生、不替代原CLI gate或几何审计。真实r49仍integration_pending；旧产物不能给当前Rust候选背书。A-D1通过也不代表里程碑A完成。

## 最终验收（2026-09-08 12:18 Asia/Shanghai）

### 冻结候选与主Agent独立结果

- checker SHA256：4609b41b2a8c2e00b10783c496b6b63d5aba63179274109be8ba674498a53b64。
- test SHA256：3ca07450e2f52ca8feea350951e3aa688407974cb97510dad3076a6b4de70a63。
- node --test scripts/collaboration/check-evidence.test.mjs：75/75，0 fail，退出0。
- node bevy/work/collaboration/review/a-d1-r2-independent.mjs：control通过；五个负例均具体原因拒绝；断言式总退出0。
- node bevy/work/collaboration/review/a-d1-r2-positive-compat.mjs：完整control、默认值省略、hash大小写、多场景过程顺序变化四项全部pass/0。
- 两个node --check均0；node scripts/generate-tree.mjs --check通过；git diff --check退出0（他人Rust及既有docs仅CRLF提示）。

独立日志均位于 G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/review/：
- a-d1-r2-final-tests.log
- a-d1-r2-final-independent.log；本次原始目录r2-independent-9EPZib/
- a-d1-r2-final-positive.log；本次原始目录r2-independent-pSV3Dm/

以最终hash候选再次运行真实demo只读紧凑检查：
- r49-frontage-maximum-r2-final-20260908-121758.json：退出1，status=fail，worldQualityStatus=fail。
- r48-frontage-maximum-final-r2-final-20260908-121758.json：退出1，status=fail，worldQualityStatus=not_found。

二者均非正式受信运行收据。r49的源码→binary→本次执行来源仍需工具外确认，故integration_pending。未为了通过而改authority、原gate或最大图。

### 接受范围和兼容性限制

接受A-D1只读证据完整性/内部一致性工具及其回归测试；可用于后续验收流程，不再让v4f重复实现R2。
不接受/不宣称：当前Rust世界生成完成、A-T1最终通过、6×10km当前源码全量通过、frontage几何独立通过、sourceSnapshot编译来源证明、里程碑A完成。

本次明确兼容性策略：log-only旧回执incomplete；过程stopReason必须显式null；不能从非规范路径可靠确定sceneId时incomplete；超过JS安全整数精度的u64身份unsupported，不作舍入后相等的假证明。仍不是完整Rust manifest语义校验器。

### 写入隔离偏差与处置

Terra报告误执行review独立脚本，新增review/r2-independent-SHvGTd/；没有报告改写已有review证据。此行为越过只读review写入边界，记录为流程偏差。保留该目录，不清理，不采用其作为主Agent证据。
总控后来独立运行生成r2-independent-9EPZib/及r2-independent-pSV3Dm/，本次接受仅依据这些结果及主Agent75项复跑。Luna与Terra实现/测试文件写集未合并交叉授权。

### 下一步

结束两名worker本轮写入锁，暂不自动开展D2。下一验收优先处理既有A-T1候选与其未归属Rust差异，再按统一合同冻结D2全域HTML/资源测量接口。最大图RUN槽仍未授予，不重跑旧大图来补不相关脚本证据。
HEAD仍为6020fbe2713e8c79c1b2934b244402dc93e1df14。未暂存、提交、推送，不修改F:/modenmapbox。
