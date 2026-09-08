# 里程碑 A 最终签收（2026-09-08）

结论：`milestone-A-accepted`。本结论只表示 A-01～A-10 的稳定底座与证据齐全，不表示
whole-world-quality-pass，也不表示 milestone C 完成。

唯一仓库：`G:\difyllmwiki\GlyphWeave`。分支：
`refactor/authoritative-baked-audit`。基线 HEAD：
`6020fbe2713e8c79c1b2934b244402dc93e1df14`。未修改 `F:\modenmapbox`，未执行
git add/commit/push/reset/clean。

## 最终候选身份

| 项 | 值 |
|---|---|
| Runtime | `bevy/target-r-final/release/glyphweave.exe` |
| Runtime SHA256 | `99374af17e471e46e5c09185e28775fcd6661e71e025d6a45aa49f2e521b3bc8` |
| 输入 manifest | `examples/maximum-local-height-frontage.manifest.json` |
| 输入 SHA256 | `d4fe4d8c72783067f6996621f2836fd5bbc8a154f3fb81c6d2e220fba9154749` |
| 源码清单 | `bevy/work/collaboration/review/final-source-manifest.json` |
| Runtime 绑定 | `bevy/work/collaboration/review/final-runtime.json` |
| 最终世界 | `bevy/work/milestone-a-final-r51-20260908-v2-abs2` |
| worldRevision | `bcc8328c0618ac78756a37bd7eaf013db8098a4f9a73aec19e583822242e52c0` |
| generatorRevision | `worldgen-2026-09-08-r51-stable-v1-transport-identity` |
| 独立 parity | `bevy/work/milestone-a-final-r51-v2-abs2-parity-20260908/parity.json` |
| 浏览器巡检 | `bevy/work/milestone-a-final-r51-v2-abs2-preview-check.json` |
| 原始浏览器观测 | `bevy/work/milestone-a-final-r51-v2-preview-check-browser.json` |
| 机器验收 | `bevy/work/collaboration/review/final-evidence-check-final.json` |
| 许可清单 | `bevy/work/collaboration/review/release-license-inventory.json` |

## A-01～A-10

| ID | 结论 | 证据与边界 |
|---|---|---|
| A-01 | pass | 最终 runtime、输入 manifest、源码清单逐文件 SHA256 绑定；全 workspace 与前端测试均对最终源码运行。 |
| A-02 | pass | `quality-report.json`：frontage 144/144、unfilled 0；landProtection changedHeightSamples=0；roadNetwork planned/realized edges 270/270、sites 300/300；service 4/4；固定入口正负毫米测试通过。 |
| A-03 | pass | transport 使用含方向语义的 stable-v1 ID；6 项 stable-v1 core 回归通过；`apply-patch-preserving` 集成测试证明未批准移动/更新/移除不会发布；两次完整生成 worldRevision 相同。 |
| A-04 | pass | 6×10km 最终生成 267.164s / 峰值约 980 MiB；240 chunks、786 entities；独立 gate 39.200s / 峰值约 1298 MiB，退出 1 的六类失败均为已批准 B/C 内容缺口；`check-evidence` pass，worldQualityStatus 保留 fail。 |
| A-05 | pass | 最终世界生态 linear/indexed 对照：输出 blake3 `fd941ff18394a926a7d32280fdc518d06dad3671ab1c13f2851badc6bd38a7e2` 相同；检查量 4,583,094,907 → 48,484。 |
| A-06 | pass | `audit-preview-stream` 对最终 464,838,532B 源流式解析，240/240 chunk parity pass；三个 Node 几何脚本用 `--audit-dir` 对最终世界 pass，不整读 scene。 |
| A-07 | pass | 浏览器同一页遍历 240/240 块、3 个 warm、9 个 revisit；networkRetry/lateResponse pass；`entityCoverage=pass`、`heightCoverage=pass`；原始观测耗时 1433.468s。最终候选的 world/header/scene/parity 语义 hash 与观测完全一致，观测按 hash 重放到最终候选并 pass。 |
| A-08 | pass | naturalTerrain source/terminal area 60,000,000m²、cycles 0、descendingEdges 0、integrityErrors=[]；保护区 changedHeightSamples=0；frontage/service/road 独立审计 pass；全部差异显式记录。 |
| A-09 | pass | Cargo.lock 518/518、pnpm-lock 347 项已盘点；224 项已解析，122 项当前平台未安装可选原生包，1 项 `@cloudflare/workerd-windows-64` 明确未解析；Three.js LICENSE 随源码与 preview 分发；建筑资产来源明确标为未证实。 |
| A-10 | pass | workspace 470 passed / 0 failed / 3 ignored；前端 Vitest 170/170；Node test 227/227；`pnpm lint`、`pnpm typecheck`、`pnpm doc-tree:check`、`git diff --check` 均通过。 |

## 保留的已知内容缺口

最终 gate 仍为非零，失败字符串：

1. `"scene-0" automatic road targets remain out of reach after "segment_budget_exhausted"`
2. `"scene-0" purpose-area allocation or realization remains incomplete`
3. `"scene-0" landcover remains unrealized`
4. `"scene-0" ecology density budget has 1 unfilled plants`
5. `"scene-0" land-use planning or realization remains incomplete`
6. `"scene-0" road-first land-use development remains incomplete`

这些是逐条批准的 B/C 内容缺口，不是工程错误：rules checked/passed 786/786、rejected 0、
collisions 0、floating 0、submerged 0、blocked entrances 0、disconnected roads 0。

## 未证明与限制

- 不声明 whole-world-quality-pass；不声明 milestone C。
- Quaternius building OBJ/MTL 的精确上游 pack 与适用许可版本仍未证实，已记录在
  `assets/third_party/quaternius/buildings/NOTICE.txt`。
- `@cloudflare/workerd-windows-64@1.20260701.1` 的许可证元数据仍缺失，已明确标记。
- 许可清单是发布工程盘点，不是法律意见或法律合规认证。
- `cargo fmt --check` 未作为 A-10 验收门：工作树包含前序未格式化改动，运行它会重排
  大量既有源码；A-10 采用仓库规定的 workspace、lint、typecheck、test、tree、diff 命令。

## 最终判定

`milestone-A-accepted`。A-01～A-10 均有当前源码、最终 runtime 和最终世界证据；质量 gate
保留的失败仅限已批准的 B/C 内容缺口，并已明确移交后续内容任务。
