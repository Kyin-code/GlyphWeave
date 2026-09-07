# 外部Agent交接检查点说明

日期：2026-09-08（本机Asia/Shanghai）。用户明确授权先提交一次代码，由用户转交prompt给外部DeepSeek v4f。此次提交是**当前工作状态的checkpoint，不是Milestone A或C验收通过**。

## 提交范围

保存此前尚未提交的Rust核心/CLI、HTML/Godot适配、规则、测试、样例、研究/重构文档、协作任务书与NOTICE。这些是多轮累计成果，不能称为本次新完成。提交前排除 `bevy/target/`、`bevy/target-r*/`、所有work大图/日志/runtime和node_modules；不删除这些本机文件。

已有MIT LICENSE继续保留；完整第三方代码/模型/纹理/GIS许可审查未完成。本次仅做有限模式的敏感文本排查及提交范围/大小检查，不作全库安全或合规认证。没有推送远端；独立机器无法仅凭commit ID取得本机未推送对象，须用户授权推送或由用户传递仓库快照/包。

## 已知未完成项必须随交接保留

- r49完整世界可生成，完整quality gate仍有六类真实失败；道路/土地/生态/自然—人文总目标未完成。
- 当前源码在生成accessGraph后重置frontage实体，有图/实体潜在不一致；已派Terra A-T1，尚未修复。core测试通过不证明此顺序问题不存在。
- r49已生成runtime和当前源码不是相同已验收候选；不能拿旧世界报告证明新源码整图通过。
- Terra已经确认暂停、没有本任务文件写入或活动测试/构建进程；其他历史preview保持，不清理。
- DeepSeek未在当前环境启动；只接受用户外部转交。v4f禁止暂存、提交、推送或变更Git历史/分支，交付未提交白名单文件。

## 验证口径

已有最新core日志为369通过/2ignored；此前r49最大图生成/独立gate记录仅证明各自运行的runtime。checkpoint另运行目录树、差异、当前workspace编译检查和轻量前端测试，具体输出由总控最终回复给出；未重新跑完整大图，不能宣称所有集成测试/性能都已验证。

## 交接操作

用户将本次commit ID作为BASE_COMMIT附于 `docs/collaboration/deepseek-v4f-forward-prompt.zh-CN.md` 前发送给v4f。若v4f工具能访问本机，可直接打开G盘项目，只写任务书白名单；并行Terra只写其Rust范围。若另一台机器，在该checkpoint的独立副本操作，原work数据可能缺失：只跑合成fixture，真实证据检查标integration_pending，不重新生成大图补数据。

回传时必须包含新增文件（普通git diff会遗漏untracked）、命令退出码、证据、未验证项和明确“未提交”。主Agent审查/验收后再按用户授权统一提交；v4f不自行提交。
