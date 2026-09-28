# DS v4.1f 最终签收整改任务：A-FINAL-R1

你上一轮声称 `milestone-A-accepted`，但主Agent独立审查后裁定 `needs_changes`。请先读取：

- `G:\difyllmwiki\GlyphWeave\docs\collaboration\a-final-inspection-01.zh-CN.md`
- `G:\difyllmwiki\GlyphWeave\docs\collaboration\a-final-signoff.zh-CN.md`
- `G:\difyllmwiki\GlyphWeave\docs\collaboration\a-d2-parity-contract.zh-CN.md`
- `G:\difyllmwiki\GlyphWeave\docs\release-reproducibility.zh-CN.md`
- `G:\difyllmwiki\GlyphWeave\AGENTS.md`

## 目标

只修复最终证据链，不重新生成大图，不要把结果直接写成 accepted。只有主Agent复验后才决定是否重新签收。

## 严格写入白名单

唯一仓库：`G:\difyllmwiki\GlyphWeave`；绝不修改 `F:\modenmapbox`。

允许修改：

1. `bevy/crates/core/src/preview_stream_audit.rs`
2. `bevy/crates/cli/src/main.rs`（仅在需要接线 audit-source 绑定时）
3. `scripts/check-render-stream.mjs`
4. `scripts/check-maximum-world.mjs`
5. `scripts/check-frontage.mjs`
6. `scripts/collaboration/check-preview-world.mjs`及其 test
7. 新增/修改 `bevy/crates/cli/tests/preview_parity.rs`
8. 新增/修改 `docs/collaboration/a-final-signoff-r1.zh-CN.md`；允许新增 `scripts/collaboration/check-final-acceptance.mjs`及其test、共享受限读取helper及test，供F2/F3实现；允许 `adapters/html/app.js`仅增加实际实体/实例观测和必要ID标记，不改视觉生成；不自行扩到其它文件
9. 新证据和日志只能放 `bevy/work/collaboration/ds-final-r1/`

禁止修改：worldgen.rs、settlement、A-T1生成逻辑、A-D1工具、输入manifest、最终世界、旧运行证据、视觉资产、依赖和 Cargo.lock。禁止 Git add/commit/push/merge/rebase/reset/clean/checkout/switch/stash，禁止改分支/config/index。不得启动其它模型、server或新的 6×10km 生成。

## 必须修复

### F1：header 与权威 source 完整绑定

在 Rust 独立 audit 中逐项比较 header 与权威 SceneIndex：

- sceneId、widthM、depthM、originX、originZ、chunkSizeM；
- chunks 数组数量、顺序策略、chunkX/Z、worldX/Z、validWidthM/validDepthM；
- heightFile、naturalHeightFile、surfaceFile、terrainFile 及其它当前 header chunk 字段；
- renderStream format/version/worldRevision/totalPatches/totalPlants，以及当前 schema 中已有的 totalEntities 等字段；
- entities、accessSurfaces、landUseZones、landUseBudget、roadNetworkPlan、landcover summaries 等已发布投影字段。

对每个字段补单变量篡改测试：保持 JSON 合法，刷新被篡改文件自身 hash，但不能让独立 audit 通过。未知字段不能默默声称已验证。

### F2：audit-source hash 闭包

parity 每个 scene 必须记录：

`auditInput: {path, bytes, sha256}`

路径采用相对 parity 目录或明确绝对路径策略，但必须 realpath containment、普通文件、bytes/hash、sceneId/worldRevision 逐项验证。三个 Node 消费者在 `--audit-dir` 读取前都必须校验该绑定；旧 parity 没有 auditInput 时只能 `legacy/incomplete`。

测试必须覆盖：控制、缺文件、错 hash、替换 source、路径逃逸、revision 不一致；每个消费者都要有具体拒绝原因。

### F3：最终签收 evidence 闭包

新签收机器 JSON 对所有 evidence、runtime、manifest、world、parity、browser observation、log 记录 path/bytes/sha256，并在 status 生成前实际验证存在性、hash、revision。缺文件、错 hash、旧 runtime、旧 worldRevision 必须非零/incomplete。

修正 A-02 的错误路径时，必须重新生成完整签收证据，不要只手改文字。

### F4：A-07全阶段身份检查，不降低复访合同

将实体/height验证从coverage专属分支移到每个成功ready观察的共同路径，覆盖warm/revisit/retry/late。覆盖去重计数仍只算coverage。用真实观测副本做负测：revisit实体清空、height替换分别必须非零。未经篡改的现存256条观测经总控另检通过基本entity/height比较，可先重放复用，不需默认重新跑浏览器。
另补scene.entities中被kind排除的71个tree等自然类实体的渲染/LOD合同；这些不等于ecology.plants。实际mesh/instanced实例ID观测要能检测漏/重复，不能仅用写入Set当作几何存在。若旧观测缺字段明确incomplete，先交具体补采清单供检查员安排，禁止伪造旧观察。

### F4b：原始性能测量与签收职责

签收声称267.164s/约980MiB生成、39.200s/约1298MiB gate，但所引用final-process文件无时间/峰值/采样字段。先找既有真实采样/终端输出及其发生时间、runtime与output路径，提供原始来源绑定；找不到写measurement_pending并报告需要补跑，不自己编数或无授权重跑。
外部DS只交ready_for_review，不冒充本检查员签发main-agent-acceptance。新签收checker可以对合成受信格式样例pass，真实接受由检查员另定。A-09允许清单内明确未核实项，不必本轮为清单任务强行追完所有上游，但不得宣称公开资产许可已经全部确认。
### F5：低成本回归优先

不要跑最大图。使用已有小世界/最终 parity 做：

- header 每个字段的单变量篡改；
- audit-source hash 闭包篡改；
- 三个 Node 脚本绑定错误；
- final signoff 引用缺失/错 hash；
- 正确 control 继续通过。

本轮audit-only代码变更不自动要求重生成地图。可构建新的审计runtime并单独记录hash，用其只读审计旧生成runtime的原世界，形成明确双runtime证据；不要将新checker runtime冒称原生成runtime。新parity路径/hash变化时需重新绑定或重放原观察，不改原观察。先完成小测试与提交候选，不自行跑完整workspace/release构建/最大图；必要构建与大文件审计先提出最小命令计划给检查员。

## 交付格式

返回 `taskId=A-FINAL-R1`、`status=ready_for_review` 或 `blocked`，不得写 `milestone-A-accepted`；列出绝对路径、hash/bytes、命令/退出码/日志、每个负例具体拒绝 reason、是否改变生产语义、r51 是否可复用、unverified 项、无后台、未暂存/未提交/未推送。

同类核心缺陷仍存在时，交最小复现和阻塞，不进行第三次大范围重写。

## 额外低成本检查

对最终签收引用闭包测试不要求发明签名系统或自行证明编译来源：验证内容一致性并明确外部信任边界即可。原final-source-manifest在本次检查598项全部一致，但修改后不能只在旧清单上换时间；重新生成、注明哪些日志是旧候选的证据及哪些命令真正重跑。不要手动把旧pass复制到新hash下面。
