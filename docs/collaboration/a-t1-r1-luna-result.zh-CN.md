# A-T1-R1-L：CLI frontage 恢复与拒绝证据补丁

日期：2026-09-08（Asia/Shanghai）。状态：`ready_for_review`。
仅测试候选交付；没有运行或声称通过 Rust 测试，不代表 A-T1 或 Milestone A 验收。

## 文件与隔离

唯一代码修改：
`G:\difyllmwiki\GlyphWeave\bevy\crates\cli\tests\baked_audit.rs`

- 最终 SHA-256：`0FC8AE0FFECED2936B7A84F068357FDDCB01686F8276DCE8D6B564C2F3589690`
- 入口基线 SHA-256：`4A8D8B568F59DA01953C03E560FD055281BB5149C1CC1AD7E2AEFCC59BC90C98`
- 基线 HEAD：`6020fbe2713e8c79c1b2934b244402dc93e1df14`
- 分支：`refactor/authoritative-baked-audit`
- 修改仅限 `local_height_frontage_bakes_real_city_buildings_and_rejects_forged_plans`
  函数体和其中局部闭包；文件其余部分与入口快照逐字节一致。
- 入口已经有其他协作者的 9 行脏改动，已融入此目标测试的收紧，未撤销其他测试。
- CLI 生产、Terra 的 access.rs、其他 core 文件、原输入 manifest、原 world、
  A-D1 冻结脚本均未编辑；F:\modenmapbox 未修改。

## 变更

1. 每次 `quality-report` 前删除此合成 TestDir 内已有报告；每个 anchor/roadPoint
   负例后恢复原 scene 字节，重新运行 CLI，成功后才读取新报告。
2. 初始及每次恢复后都检查独立 `accessGraph` 是对象，且等于持久化原图；恢复后
   再逐字段比较 baseline 的 accessGraph、frontageAudit、serviceProgramAudit、
   landProtectionAudit、roadNetworkAudit 和 landUseLedger。
3. 两个负例分别只改同一 frontage 实体的 `/anchors/0/worldX` 或
   `/grounding/frontageRoadPoint/0` 一个整数坐标；把该坐标回填后与整个原 scene
   相等，防止其他字段差异掩盖原因。原 source building plan 字段保持不变。
4. 两个负例均要求正常 CLI 退出码 1、明确 stderr 来源诊断、无 panic、没有复用
   旧 quality report，并检查 CLI 没有自动修复 scene。
5. 明确保留 8 个 frontage 需求（住宅6、商店2）、住宅缺额3、实际5栋、原4服务
   设施请求/实现、保护区 changedHeightSamples=0、道路审计 pass 和实际建筑面积。
6. 初始和恢复后的 world-quality-gate 都要求退出1、报告 fail，且恢复后 failures
   列表与基线完整一致；保留具体 frontage 缺额失败文本。
7. 原 scene/发布 road plan 同步伪造负例保留，并增加真实源码重建诊断断言；原
   高度 payload 单毫米 old+1 负例保留，不放松毫米阈值。

## CLI 真实诊断的依据（源码静态核对，不是运行采集）

anchor/roadPoint 负例预期 stderr：

```text
glyphweave: persisted land-use budget differs from rebuilt entity and zone areas
```

- `G:\difyllmwiki\GlyphWeave\bevy\crates\cli\src\main.rs`：main 打印
  `glyphweave: {error}` 并退出1；quality_report_command 在 frontageAudit 输出之前
  比较持久化 budget 与 plan_landuse_budget 重建值，返回上述实际字符串。
- `G:\difyllmwiki\GlyphWeave\bevy\crates\core\src\worldgen.rs`：road-first
  budget 调用 development::realized_footprint_areas。
- `G:\difyllmwiki\GlyphWeave\bevy\crates\core\src\settlement\development.rs`：
  realized_cells 仅计入 same_entity 匹配的原规划 owner；same_entity 只忽略派生
  bounds，仍比较 anchors 和 grounding。因此测试显式绑定 frontage entityId、
  source building 和被改字段，不能把任意非零或崩溃当通过。

CLI 没有在此提前返回路径输出“anchor mismatch”或“roadPoint mismatch”专用文本，
本补丁没有臆造这类字符串。诊断运行时是否符合预期仍须总控串行 Rust 运行确认。
plan 伪造诊断 `road plan differs from natural raster reconstruction` 来自
`G:\difyllmwiki\GlyphWeave\bevy\crates\core\src\worldgen\road_network.rs`。

## 实际执行的静态检查

工作目录：`G:\difyllmwiki\GlyphWeave`。

| 命令 | 退出码 | 范围 |
|---|---:|---|
| `node G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/luna-t1-r1/static-check.mjs` | 0 | 目标函数外字节未改、冻结基线、诊断来源、恢复顺序及必需断言文本、A-D1冻结hash |
| `git diff --check -- bevy/crates/cli/tests/baked_audit.rs` | 0 | 空白检查；仅已有LF/CRLF提示 |
| `node scripts/generate-tree.mjs --check` | 0 | AGENTS目录树未过期 |

日志：`G:\difyllmwiki\GlyphWeave\bevy\work\collaboration\luna-t1-r1\static-check.log`。
静态脚本内子命令逐个记录退出码。它不解析/编译 Rust，不等于 cargo check/test/fmt。

## 建议总控精准命令（未运行）

```text
cargo test --manifest-path G:/difyllmwiki/GlyphWeave/bevy/Cargo.toml -p glyphweave-cli --test baked_audit local_height_frontage_bakes_real_city_buildings_and_rejects_forged_plans -- --exact --nocapture --test-threads=1
```

仅在总控串行 RUN 槽执行。一次目标测试包含一次512m bake、初始/恢复后的独立
quality-report、两次完整 gate 和原有负例；不是“全报告status=pass”或最大图证明。
如果断言失败，请保留具体 stdout/stderr 与实体/字段上下文，按文件锁退回修订，
不能删诊断断言或降低8/3/4/毫米要求。

## 未运行与声明

- 没有执行 cargo 的任何子命令，包括 test/check/fmt；没有直接启动 Rust CLI。
- 没有运行512m bake、quality-report、world-quality-gate，也没有新红/绿运行证据。
- 没有全workspace/release/最大图/服务/后台进程/额外模型；未启动T2/D2/C。
- 没有认证真实runtime来源或全域自然/保护区，只写合成测试的预期断言。
- review目录只读，未执行其中脚本；未安装依赖或引入第三方代码。
- 没有git add/commit/push/merge/rebase/reset/clean/checkout/switch/stash或其他Git写操作；
  未暂存、未提交、未推送。交付后等待总控。
