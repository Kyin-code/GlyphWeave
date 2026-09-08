# 白班A-T1候选收尾与验收

日期：2026-09-08。唯一仓库G:/difyllmwiki/GlyphWeave；F:/modenmapbox不修改。
HEAD=6020fbe2713e8c79c1b2934b244402dc93e1df14。A-D1 accepted_tooling，禁止再改其脚本。
总控立即审查既有7个Rust差异、冻结来源；Terra/Luna分离补核心和CLI测试。当前仅候选级A-T1，不启动T2/D2/C、不跑最大图或完整workspace。

## 写入锁

- A-T1-R1-T / GPT-5.6 Terra：只改bevy/crates/core/src/worldgen/access.rs中的A-T1测试及必要窄注释；如发现必须改生产逻辑先报告总控，不扩大写集。结果docs/collaboration/a-t1-r1-terra-result.zh-CN.md；证据bevy/work/collaboration/terra-t1-r1/。
- A-T1-R1-L / GPT-5.6 Luna：只改bevy/crates/cli/tests/baked_audit.rs中的local_height_frontage_bakes_real_city_buildings_and_rejects_forged_plans测试及局部辅助；结果docs/collaboration/a-t1-r1-luna-result.zh-CN.md；证据bevy/work/collaboration/luna-t1-r1/。
- 总控：协调/验收文档和review证据，不代写worker业务实现。

所有上述相对路径均位于唯一仓库。worldgen.rs、frontage.rs、road_network.rs、engineered_placement.rs、services.rs当前均只读。不清理、不全文件格式化、不归属或撤销他人变动。review目录只读，禁止执行其中会写文件的脚本；上一轮Terra偏差不能重复。

## 必读

AGENTS.md；docs/milestone-a-agent-task.zh-CN.md；docs/collaboration/acceptance-contract.zh-CN.md；docs/collaboration/a-t1-result.zh-CN.md；相关Rust实际实现。总控冻结原候选测试副本在bevy/work/collaboration/review/a-t1-baseline/，只读。

## Terra交付

为固定frontage合同增加有判别力的核心回归：yaw四方向、非零origin、正负毫米支撑。沿用实际构造与变换语义，不只改yaw字段却沿用北门位置。核验门anchor、roadPoint、位置、yaw、grounding未被fallback改动；最终accessGraph与独立重建一致；障碍挡固定入口而其他面可达时必须保留失败，不删除实体。普通generated fallback及authored入口已有回归保持。

尽量参数化小fixture，不依赖大图、work/r49或网络；需要地形hash不变证明可对fixture烘焙payload前后比较，不能把局部fixture宣称全地图自然层/保护区通过。总控保留旧负毫米frontage回归和road-door orientation测试作为补充。新增测试要具体命名，便于总控按名称运行。

## Luna交付

当前CLI测试篡改anchor/roadPoint后恢复scene，却不重跑quality-report就读取原报告；收紧恢复与负向证据。
1. 原场景独立quality-report通过，持久化graph与重建graph相等。
2. 每个篡改后不仅assert非零，还必须确认报告或诊断指出相应frontage/anchor/roadPoint来源不一致，而不是崩溃/任意非零；先读CLI真实诊断，不臆造错误字符串。
3. 恢复scene后重新运行quality-report并成功，再读取新报告，核对graph与原图、需求/实现/缺额一致。
4. 原8栋frontage需求、3住宅缺额、原4服务设施、保护区changedHeightSamples=0、原gate失败继续显式保留；不降低要求。
5. 既有plan伪造/1mm高程篡改继续拒绝。不得触碰CLI生产逻辑、原输入manifest、gate阈值。

## 运行资源（串行Rust，禁止双方竞争编译）

双方初阶段只编辑和静态检查，不执行cargo（包括test/check/fmt），交代码与建议命令给总控。可Node内置/只读命令检查文本，不安装依赖。
总控固定两份最终文件hash后统一串行执行限定core测试和一次512m CLI集成；如失败按文件锁派一次有界修订。只有总控授予独占RUN槽后worker才可cargo。禁止全workspace/release/最大图/preview/后台server。

## 验收与Git

必须能明确区分候选级窄功能通过和全A通过；无全域证据仍标未验证。不删失败、不删生成对象、不放松毫米检查。
禁止git add/commit/push/merge/rebase/reset/clean/checkout/switch/stash及Git配置/索引/历史写操作。只读Git允许，不新增模型。
完成交ready_for_review、绝对文件路径/hash、变化范围、未运行清单、建议精准测试命令、无后台进程/未暂存提交推送声明。交付后等待总控，不再自己扩任务。
