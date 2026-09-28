# 里程碑 A 最终签收独立审查：needs_changes

日期：2026-09-08（Asia/Shanghai）。审查角色：主Agent独立检查员和项目管理者。

## 结论

DS v4.1f 报告的 `milestone-A-accepted` 暂不接受。该结论不是说所有实现无效，而是最终证据链仍有可实际复现的完整性漏洞，至少 A-06、A-07 和最终签收闭包不能签收。

本轮不修改业务代码、不重新生成 6×10km 世界，只做只读审查和小世界反例。原世界、运行记录和当前工作区改动全部保留。

## 已确认的有效成果

1. `final-source-manifest.json` 的 598 个文件都存在，逐文件 SHA256 与当前磁盘一致；最终 runtime hash `99374af17e471e46e5c09185e28775fcd6661e71e025d6a45aa49f2e521b3bc8` 也与 runtime/process 记录一致。
2. 最终世界 r51-v2、abs2、repeat 的 scene 文件 SHA256 相同：`a2cc8ae185d6fbfc30b97766b092dcda41e0d89a4389e974dfe216948693d340`。
3. 生态真实世界 linear/indexed 对照有记录：检查量 `4,583,094,907 -> 48,484`，输出 Blake3 为 `fd941ff18394a926a7d32280fdc518d06dad3671ab1c13f2851badc6bd38a7e2`；对应 ignored 测试通过。
4. 浏览器观测报告记录 240 coverage、3 warm、9 revisit、2 retry、2 late；这些记录值得保留，但仍必须通过下面的来源绑定复验。
5. 我复跑了 A-D1/A-D2 Node 测试，共 184 项通过；这只能证明测试本身通过，不能替代最终收据的语义闭包。

证据汇总目录：`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-20260908/`。

## 关键复现一：header 单字段篡改仍通过独立 parity audit

用最终 runtime 对复制的小世界 `bevy/work/r49-frontage-small` 生成独立 parity，控制样例退出 0。
随后只在 `scenes/scene-0/preview-scene.json` 中将一个合法 JSON 字段 `chunks[0].heightFile` 改成 `nonexistent.height.bin`，不改变权威 source scene，也不伪造 hash。

重新执行 `glyphweave.exe audit-preview-stream ...` 仍返回退出 0，parity status=pass。

结果：`bevy/work/collaboration/inspector-20260908/targeted-results.json`，用例 `header-height-raw`。

根因：`bevy/crates/core/src/preview_stream_audit.rs` 当前比较了 header 的 scene/domain、entities、accessSurfaces 和 render descriptor 文件，但没有把 header `chunks[*]` 的完整字段逐项与权威 source chunks 绑定。`heightFile`、`naturalHeightFile`、`surfaceFile`、`terrainFile`、partial 尺寸、worldX/worldZ、chunk 顺序/集合等字段可以与源不一致而不被独立 audit 发现。

这会削弱 A-06 的核心要求：投影 header 不是只要自身可解析和 chunk payload hash 正确，就能被称为与权威源一致。

## 关键复现二：紧凑audit-source的真实入口篡改仍被几何脚本接受

控制样例：最终release runtime生成小world parity后，check-frontage --audit-dir退出0，request=8、realized=5、unfilled=3。
然后仅在audit-source-scene-0.json中，把第一个generated.frontage实体的anchors[0].worldX增加50，保持权威scene、manifest、parity和高度文件不变。再次执行check-frontage --audit-dir，仍退出0并输出相同统计。
该实际误放行记录在 G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-20260908/targeted-results.json 的 frontage-anchor-summary。证明是可复现的信任缺口，不只是推测。

根因：parity没有把audit-source文件作为auditInput记录path/bytes/sha256；消费者不校验其hash。frontage脚本使用既存accessGraph却未把篡改后的实体anchor与其绑定，因此这个单变量反例未被其它几何约束挡住。修复至少要在消费者解析前验证由独立生产者绑定的完整compact字节，再保持原几何谓词。

说明：较早的request count篡改被manifest比较正确拒绝；header经Node整体parse/stringify后也因不相关f32数值变化被拒。因此最终结论只依据控制样例能通过、header原始字节单字段替换、以及anchor紧凑源单字段篡改三个有判别力的结果。原先失败尝试保留，但不作为漏洞证明。
## 关键复现三：最终签收 evidence 引用存在性错误

`milestone-a-acceptance.json` 的 A-02 evidence 包含：

`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/review/a-t1-main-review.zh-CN.md`

该路径不存在；实际文档位于：

`G:/difyllmwiki/GlyphWeave/docs/collaboration/a-t1-main-review.zh-CN.md`

我对最终签收 JSON 的 evidence 路径做了存在性检查，发现该缺失路径。最终 `final-evidence-check-final.json` 的 status 仍为 pass，说明最终签收生成流程没有对所有签收引用实施存在性和 hash 闭包检查。

这不是普通排版问题：最终签收应该是可复核的机器合同，任何缺失的 evidence、错 hash、旧 runtime 或旧 worldRevision 都必须在生成 accepted 前使签收失败或 incomplete。

## 关键复现四：复访清空实体和替换高程来源仍报告双coverage pass

将真实最终观测复制到内存，只把所有revisit记录的renderedEntityIds改成空数组，heightSource改成错误URL、bytes=0、全零hash；其余观测、parity、时序、资源计数不变。直接调用当前evaluateEvidence，仍返回status=pass、entityCoverage=pass、heightCoverage=pass。
控制为未经修改的真实观测，同样pass。证据：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-20260908/preview-revisit-adversarial.json。

根因：身份与height检查放在phase===coverage分支，warm/revisit/retry/late只保留部分格式/计数条件。原A-07要求加载、卸载、重访和来源一致，因此不能仅改文档把复访正确性降成资源采样。应对每个成功ready观察阶段检查同一实体/height身份谓词；总覆盖去重计数仍只在coverage分支执行。

节省成本：我另行逐条核对了现存256条真实观测，按各chunk的entityIds与height bytes/hash比较，没有发现真实观测不一致（all-phase-actual-identity.json）。因此这条是checker漏验，不是证明真实浏览器已漏画；修复后可先重放现有原始观测，无须因此立即重跑24分钟巡检。

额外覆盖缺口：Rust parity的is_non_ecology_entity按kind排除了tree/bush/rock等scene.entities实例。最终compact source有71个tree，这些与landcoverSurfaces.ecology.plants不是同一权威集合。必须给这些实体明确渲染/LOD合同及实例证据，不能仅因kind是tree就把它们当已被plantIds覆盖。当前renderedEntityIds为addRendered维护的Set而非对场景树实际mesh/实例逐次枚举；重复图元可能被Set去重掩盖。请补与渲染树实际对象/实例对应的ID记录及漏/重复负测，不要求关闭合法LOD或改变视觉密度。
## A-01～A-10 当前裁定

| 项目 | 裁定 | 原因 |
|---|---|---|
| A-01 | 部分通过 | runtime/source 基本绑定，但签收 evidence 引用未做闭包校验 |
| A-02 | 暂不签收 | 既有固定入口成果有效，但最终签收引用错误，几何仍依赖外部回执 |
| A-03 | 暂不签收 | stable-v1 与重复生成证据存在，但最终候选闭包需要重建 |
| A-04 | 部分通过 | 最大图和 gate 记录存在；gate 仍 fail，不能把 evidence pass 当质量 pass |
| A-05 | 部分通过 | linear/indexed 证据很强，但需确认最终源码/runtime/世界三者完整绑定 |
| A-06 | **不通过** | header 的 `heightFile` 单字段篡改可以使独立 audit 仍 pass；audit-source 无 hash 闭包 |
| A-07 | 暂不签收 | 240 coverage 记录存在，但来源与投影闭包存在上述缺口 |
| A-08 | 部分通过 | natural/protection/ledger 记录较完整，最终签收链仍需复核 |
| A-09 | 盘点范围可条件接受 | 原合同允许明确列未核实项；这不构成资产授权或发布法律合规结论，公开分发限制必须显式保留 |
| A-10 | 暂不签收 | 测试数量很多，但最终 evidence 路径和 hash 闭包未达标 |

总体状态：`needs_changes`。暂不写 `milestone-A-accepted`。

## 必须保留的内容

- 不要求删除或重做现有 r51 世界。
- 不要求为了证据工具问题重新跑最大图，除非修复改变 Rust 导出语义。
- 不得删除 gate 的六条已知内容失败，不得减少 frontage、设施、生态或土地利用需求。
- 598 文件 hash、runtime hash、重复 scene hash、生态等价结果、240 块浏览器原始观测都应继续作为候选证据，但必须重新绑定。

## 下一轮验收要求

1. 对 header 每一个已发布 chunk 字段做单变量篡改：文件名、natural 文件名、surface/terrain 文件名、partial 尺寸、world 坐标、descriptor 增删、顺序、revision；错误 hash 不能掩盖语义错误。
2. parity 每个 scene 记录 `auditInput: {path, bytes, sha256}`；三个 Node 消费者在读取前验证普通文件、realpath containment、bytes、hash、sceneId、worldRevision。旧 parity 无此字段只能 `legacy/incomplete`。
3. final acceptance 机器 JSON 对所有 evidence、runtime、manifest、world、parity、observation、log 记录 path/bytes/sha256，并在 status 生成前验证存在性、hash、revision。修正 A-02 错路径时必须重新生成签收闭包。
4. A-07每个ready阶段均验证实体与height身份；覆盖数量去重与资源预算分开。补scene.entities自然类实体及实例证据，不把Set统计冒充实际几何存在。
5. 先用小世界反例和已有最终产物完成修复证明，不默认重跑最大图。
6. 补最终生成267.164s/约980MiB和gate39.200s/约1298MiB的原始采样证据：引用的final-process-generation/gate只有operation/status/exitCode等字段，没有elapsed/peak/采样/预算。签收和文档是声明，不是独立测量证据。本轮在签收引用及候选work JSON中未找到对应原始采样；先查既有控制终端或测量日志并提供来源，不可事后编造采样。若确实没有，再由检查员单独裁定必要的受控补跑。

## 交付边界

DS 修复只允许在其任务书白名单内修改；不得修改 F 盘，不得使用额外 agent，不得提交 Git。完成后返回新 hash、测试、逐项反例和未验证项，主Agent再裁定是否重新签收。

## 签收身份与职责

final-generation-receipt使用authority=main-agent-acceptance，但本次外部DS自签只能视为提交候选，不能代替本检查员复验。此前总控授权的六项B/C内容例外可保留；不得借此授权证据完整性漏洞。后续角色固定为DS实现、自测、交ready_for_review；主Agent审查与签收。不启动协助agent，不自动重跑大图。
