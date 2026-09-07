# GPT-5.6 Terra 任务书：Milestone A 核心稳定性（v2）

日期：2026-09-08。替代初稿同时承担整个A的大任务；首批仅授权 **A-T1**。先读 `G:\difyllmwiki\GlyphWeave\docs\dual-agent-collaboration.zh-CN.md` 和 `docs/collaboration/acceptance-contract.zh-CN.md`。不修改 `F:\modenmapbox`。

## A-T1：固定入口与访问图来自同一个最终实体状态（已派发）

### 问题与结果要求

当前 `bevy/crates/core/src/worldgen.rs` 在 `place_generated_entrances` 构图之后，又把 frontage entity 重置为规划实体。图可能描述重置前的入口，实体/访问路面却描述另一入口；仅编译或core测试通过不够。

应使显式 frontage 的位置、毫米标高、方向、入口和道路接点作为固定合同进入访问图构建：generic facade fallback 不得改变其入口，失败保留。不要在构图后复制规划实体把错误隐藏；不要信任持久化 accessGraph。最终访问图/访问面必须来自同一最终实体状态，独立重建能复现。

### 允许写入（根目录 G:\difyllmwiki\GlyphWeave）

- `bevy/crates/core/src/worldgen.rs`：只相关顺序/重置窄块，不全文件格式化。
- `bevy/crates/core/src/worldgen/access.rs`：固定/可重新选门的边界。
- `bevy/crates/core/src/worldgen/frontage.rs`：该问题回归。
- `bevy/crates/cli/tests/baked_audit.rs`：针对性集成。
- `docs/collaboration/a-t1-result.zh-CN.md`。

其他全部只读；不改生态/索引/资产/输入/CLI gate/HTML/许可证。保留之前负毫米保护；不将自然+1mm自动“修复”为另一个实体。

### 最低测试

1. 固定frontage：原入口被障碍挡住、另一侧可达时，入口不变且访问失败，不被generic fallback换门。
2. 普通非固定生成建筑：原有合法fallback继续工作。
3. 明确authored入口继续保持，不被新逻辑放宽。
4. 最终图用最终实体独立重建完全一致，最终访问面与图/高程对应；故意更换入口/road point时失败。
5. 正负毫米、yaw四方向、非零origin、自然/保护层不变。
6. 512m frontage 实际bake和独立quality-report可复现；原3住宅缺额继续报告，不降低8栋需求。

先针对性测试/小图；不得自行运行最大图或完整workspace，交候选后由总控授予集成资源槽。结果不能仅是文档/诊断。必须给出红测、修复后绿测、变更范围和行为边界。

## 后续队列（未授权开工）

| 编号 | 输出 | 进入条件 |
|---|---|---|
| A-T2 | 生态实体/patch候选空间查询索引；保留原完整footprint谓词、全部预算与失败原因；同输入逐字段/hash对照 | T1验收，新的文件锁与参考源冻结 |
| A-T3 | 可选实体稳定ID/存量约束；不仅保护roads/sites，显示重命名/移动/移除原因，非法静默移除失败 | T1后单独设计评审；不能声称当前已完成 |
| A-T4 | 根据D1/D2证据修复Rust导出/读取与全域漫游接口风险 | 具体复现+接口锁，不预先改格式 |

T2不能顺便改生态密度或消除1株缺额；缺额修复是独立正确性任务。T3需同输入生成稳定、增加无关可选请求不改其他实体身份、明确存量禁止移除测试；保存世界编辑与从零重生成必须分开。任何序列化合同变更提交兼容/迁移说明。

## 返回内容

`taskId`、`ready_for_review/needs_changes/blocked`、基线文件hash/branch/dirty摘要、改动文件、每条验收编号与证据、实际运行命令/退出码、仍运行进程、失败和未做项、新增依赖/来源（无也声明）。不得自己标A完成、提交commit或做未授权的大重构。
