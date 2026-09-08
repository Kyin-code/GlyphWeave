# A-FINAL-R2 提交（ready_for_review）

日期：2026-09-09。状态：`ready_for_review`。本文件不写 `milestone-A-accepted`，
最终接受由主Agent检查员执行。

## 阶段 S：实例存在性与引用闭包

### S1

`adapters/html/app.js` 不再直接信任 Group/object 上的 ID 元数据：

- 普通实体必须在其自身子树中存在有效 `Mesh`/`InstancedMesh` 几何；空 Group 无效。
- 树/岩石批实例记录 `entityInstanceBindings`：逻辑 ID、batch UUID、geometry UUID、
  instanceIndex、instanceCount 和部件数组；index 必须落在实际 count 内。
- 一个树可由树干/树冠多个 batch 组成，仍只算一个逻辑实例。
- `check-preview-world.mjs` 必须校验 `renderedEntityInstanceSources` 完整、有效、ID 一一对应；
  缺来源、空几何、count=0、越界 index、缺部件、重复逻辑根均拒绝。

验证：

- `node --test scripts/collaboration/preview-instance-observation.test.mjs ...`
  共 150 passed / 0 failed，含真实 vendored Three 对象反例。
- 2 块控制世界：`bevy/work/collaboration/ds-final-r2/control-tree-2ch-b`，
  含 2 个 tree 自然实例和 64 个非生态实体；浏览器控制
  `control-tree-browser-r2b.json` 为 pass，240 阶段规则下的 entity/height 全阶段通过。

### S2

`check-final-acceptance.mjs` 现在直接核对：

- `parity.worldFile.path` realpath 必须等于 `candidate.world.path`；
- `parity.worldFile.sha256` 必须等于 `candidate.world` 的字节 hash；
- parity 每个 scene 的 source/header/auditInput 必须存在、bytes/hash 一致并 containment；
- browserCheck 报告必须绑定同一 parity hash/world revision；
- 支持 `measurement_pending` 与 `measured` 两种状态；pending 必须列出 missing，
  measured 必须核对 process JSON、sample JSONL、operation、exe/hash、manifest/hash、
  output、exitCode/stopReason、elapsed、600s/8192MiB 预算和采样字段。

测试：13 passed / 0 failed，覆盖 wrong-world-path、wrong-world-hash、同 revision 不同 bytes、
缺失/错 hash source/header/auditInput、旧 runtime、观察绑定、pending 缺项、缺 log。

## 阶段 M：一次有界测量

首次测量因测量脚本把 manifest 传成绝对路径，使 `rulesDir` 变为绝对路径，worldRevision
变为 `a05d...`；该次证据保留但不作为最终等价候选。修正脚本副本按调用参数传递相对 manifest 后，
纠正测量结果：

| 项 | 生成 | gate |
|---|---|---|
| operation | generate-world | world-quality-gate |
| exitCode | 0 | 1 |
| stopReason | null | null |
| elapsedSeconds | 264.41066 | 38.2008743 |
| sampledPeakPrivateBytes | 1131696128 | 1455783936 |
| peakWorkingSetBytes | 1098334208 | 1368526848 |
| sampleCount | 516 | 74 |
| timeoutSeconds | 600 | 600 |
| maxPrivateMiB | 8192 | 8192 |

原始 process/samples：

- `bevy/work/collaboration/ds-final-r2/generation-measurement-r2b/`
- `bevy/work/collaboration/ds-final-r2/gate-measurement-r2b/`

gate 的六条原批准内容失败保留，未改成 pass。

## 阶段 E：新旧内容等价

新 `measurement-world-r2b` 与原冻结 world 比较：

- 两棵树状文件集合均为 1781 个，onlyA/onlyB 均为 0；
- 唯一不同文件为 `generation-report.json`，差异仅 `output` 绝对路径；
- scene.json SHA256 为
  `a2cc8ae185d6fbfc30b97766b092dcda41e0d89a4389e974dfe216948693d340`；
- worldRevision 为
  `bcc8328c0618ac78756a37bd7eaf013db8098a4f9a73aec19e583822242e52c0`；
- 自然/最终 height、surface/terrain、实体、计划等权威文件逐字节一致。

因此 A-05 的 linear/indexed 7 分钟测试可凭相同输入与相关代码未变的证据复用，
本轮未重跑 linear 大测。

## 阶段 B：一次 240 块新观测

- 新 parity：`bevy/work/collaboration/ds-final-r2/measurement-parity-r2b/parity.json`
- served adapter：`served-adapter-manifest-r2b.json`，40 个文件，2,553,529 bytes
- server：PID 62488，`http://127.0.0.1:8196/preview/`，原生成 exe
  `99374af1...`，只按 PID 停止，历史 8099 未触碰
- 观测：`measurement-browser-r2b.json` / `measurement-observation-r2b.json`
- 结果：status pass；240 coverage、3 warm、9 revisit、2 retry、2 late；
  `entityCoverage=pass`、`heightCoverage=pass`，faults pass，errors=0；
  elapsedMs=1,393,592（约 23.2 分钟，低于 40 分钟上限）。

## 候选闭包

- 候选：`bevy/work/collaboration/ds-final-r2/final-acceptance-r2.json`
- 闭包检查：`final-acceptance-check-r2e.json`
  - closureStatus=pass
  - readiness=ready_for_review
  - missing=[]

## 保留限制

- Quaternius building 精确来源未证实。
- `@cloudflare/workerd-windows-64@1.20260701.1` 许可证元数据未解析。
- A-05 linear 结果本轮复用，不重复 7 分钟扫描。
- 未修改原冻结 world、原生成 runtime、旧 parity、旧观测或旧证据。
- 未 git add/commit/push/reset/clean/checkout/switch/stash；未启动其他 agent。
