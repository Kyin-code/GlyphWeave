# B01-G23R1 独立复核与限域验收（2026-09-21）

## 1. 裁定

**G23R1 必需资产完整性修复验收通过；结合 G23 已接受的真实浏览器消费、新鲜导出一致性与真实鼠标命中证据，G23“单一只读工程预览消费者”限域收口。**

本次没有发现阻断该修复的缺陷。不是只接受 worker 自报：检查员重建当前 CLI、复跑核心保护测试，并独立执行 12 次 CLI。不得据此宣布工程连接能力、真实城市、ST18、SceneIndex 或默认世界整体验收。A 冻结、B partial、C 不推进；G19R3C/G20/G21 原 partial 与历史超支不倒写。

## 2. 版本与冻结核验

- core engineering_preview.rs：107739 B / 2a3eae8dd2f3fffb4e37bbea4775a077f60a95f6f7f5f5a6912c063986868220。
- CLI engineering_preview.rs：12993 B / 52af51ae39532138cf042142b3b162776b1b73c9e0ec28c68b7306e725609621，未改。
- worker G23R1：87/87、9205153 B；manifest 13860 B / cd05d1968c74ec801d5c23449374adfa1638e262bed442a6aa8892028fd2c43c，回执绑定成立。
- 旧 G23 339/339、G22 主包 94/94、入口包 54/54；各清单 hash 与冻结记录相同。
- G22 三源码、access、auto_roads、三个工程 viewer 文件均与冻结 hash 匹配；检查前后 10 项源码/资产 hash 相同。
- 检查员未改生产代码、旧证据、测试或 fixture；未进行 Git 写操作。

## 3. 源码审查

REQUIRED_ASSETS 独立于待审 files[]，规定十个必需 payload 的路径与角色；导出按该规则组装，具名单测约束静态嵌入资产与规则的一致性。审核逐必需项要求唯一声明、正确角色、真实文件、bytes/hash；另拒重复路径、非规范路径、未定义角色/额外资产、错误 viewerEntry/renderModel 引用、未知 schema/version。

完成标记 engineering-preview.json 在 payload 之外单独要求存在并与读入字节一致，没有循环自哈希。源副本由固定路径读取并在 sourceBinding 校验自身来源 hash；不是信任任意描述路径。审核没有把静态资产 hash 强制等同当前编译器嵌入字节，不会仅因合法历史 viewer 字节差异而被本次结构规则误拒。

## 4. 检查员实际运行

### 核心与构建

1. cargo test -p glyphweave-core --offline --lib engineering_ -- --nocapture：**46 passed / 0 failed / 545 filtered out**，包括 12 项 preview、32 项连接保护回归及另外 2 项既有 engineering 命名测试。12 与 46 是包含关系，不能相加成独立测试数。
2. cargo build -p glyphweave-cli --offline：通过；使用该最终源码二进制执行下表。

导出环境变量 DSH_G20_TAMPER_DIR / DSH_G21_TAMPER_DIR / DSH_G22_TAMPER_DIR / DSH_G23_FIXTURE_DIR 均移除。未运行全量 auto_roads 测试。

### CLI 12 次，退出码均来自本次真实进程

| 观察 | 次数 | 结果 |
| --- | ---: | --- |
| 三个原合法导出 audit：直跑、折返、terrace | 3 | exit 0，四段 pass |
| 检查员原 missing-entry-selfconsistent 目录，只读引用 | 1 | exit 1，只有 completeness fail，点名 preview/index.html |
| 独立制作删除 three.core.js + 删除声明 | 1 | exit 1，只有 completeness fail，点名 vendor 路径 |
| 完全相同重复声明 | 1 | exit 1，completeness fail |
| 入口错误角色 | 1 | exit 1，completeness fail |
| renderModel.sha256 引用错误 | 1 | exit 1，completeness 与 sourceBinding 同时 fail；工程/投影 pass |
| 未知 version | 1 | exit 1，completeness fail |
| 删除完成标记 | 1 | exit 1，completeness 明确 fail；见第 6 节状态口径 |
| 保存 input/bundle 全新 export + audit | 2 | exported / 四段 pass；十个 payload 与原合法直跑包逐字节相同 |

没有浏览器、HTTP 服务、截图、WebGL、工程 build、新几何或地形再生成。G23 既有检查员真实点击和 HTTP404 仅引用原来源，不冒充本轮复测。

## 5. 台账和过程复核

worker 的 9 条 ledger 与各 create_new raw record 一致，stdout/stderr/report 的 hash/bytes 独立匹配；不是从期望值替代观测。worker 本包 cargo3/6、CLI9/12、辅助1/4；历史 G23 前端17/12超5、G20超1继续保留。

检查员自身成本：cargo2、CLI12、矩阵驱动1、离线结果分析驱动1；另有纯读/hash核验和文档检查。不占用、不恢复 worker 余额。

检查员矩阵脚本最初错误地把 wrong-reference 也约束为“只有完整性失败”，因此原 cli-review.json 标了该分析错误；实际 12 次 CLI 均按预期执行。源码原有 sourceBinding 同时检查 renderModel hash，双段失败正确。原脚本、原汇总、原报告全部保留；review-analysis.json 按实际职责重新分析为 PASS，未重跑 CLI、未改生产或输入来凑结果。

## 6. 非阻断记录与验收边界

1. **报告计数快照**：worker report.zh-CN.md §5 仍写 83 文件/8.75 MB；最终 manifest/receipt 和独立重算为 **87 条/9205153 B（约8.78 MiB）**。本裁定以最终清单为准，不要求倒写旧报告或新增纯数字勘误包。
2. **既有未评估状态的诊断歧义**：缺 engineering-preview.json 时，audit 正确整体拒绝，但 engineeringVerification 仍为 pass、detail 空；该分支没有运行 verify_bundle。它不造成预览包被接受，不阻断本次必需集合修复，但绝不能引用这一子段证明工程包已验证。将来若独立消费四段状态，应另行明确 notEvaluated/拒绝语义；本轮不自动扩大为审核状态机改造，也不宣称该状态问题已修复。
3. 本验收证明格式必需集合与独立完整性拒绝成立，不是对任意 hash 自洽 viewer 代码做行为安全认证。

## 7. 证据与停点

检查员唯一新目录：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g23r1-20260921/。

关键记录：baseline-verification.json、final-verification.json、core-tests.log、cli-build.log、cli-review.json（保留最初分析错误）、review-analysis.json、各 *.report.json、runs/ 原始退出码与流、fresh-straight/、独立负例。

前次接受的浏览器证据与报告：G:/difyllmwiki/GlyphWeave/docs/collaboration/b01-g23-inspection.zh-CN.md；本次不复跑、不挪用 worker 观察身份。

验收后冻结，不要求 DSH 补确认稿，不自动开下一包。后续真实世界消费者任务需单独授权目标、合同与预算。
