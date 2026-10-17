# A-FINAL-R1 证据链整改提交（ready_for_review）

日期：2026-09-08。状态：`ready_for_review`。本文件不是验收签收，也不写
`milestone-A-accepted`；由主Agent检查员复验后决定。

## F1：header 与权威 source 完整绑定

`preview_stream_audit.rs` 现在逐项绑定：

- sceneId、widthM、depthM、originX、originZ、chunkSizeM、chunkCountX/Z；
- header.chunks 的完整 typed 对象（数量、顺序、chunkX/Z、worldX/Z、valid 尺寸、
  height/naturalHeight/surface/terrain 文件、hash、boundary 及未知字段）；
- renderStream format/version/worldRevision/totalPatches/totalPlants；
  当前 schema 若出现 totalEntities 也必须等于源实体数；
- landmarks、entities、accessSurfaces、developmentPlan、landcoverPlan、
  landUseZones、landUseBudget、roadNetworkPlan、landcoverSurfaces；
- renderStream.chunks 与独立读取的 chunk payload/源 chunk 逐项一致，顺序固定为 producer 的
  `(chunkX, chunkZ)` 顺序。

小世界单变量篡改测试覆盖 38 个字段/结构，全部必须非零 fail：
`cargo test --manifest-path bevy/Cargo.toml -p glyphweave-cli --test preview_parity -- --test-threads=1`
结果 4 passed / 0 failed。

## F2：audit-source hash 闭包

每个 parity scene 新增：

```json
"auditInput": {"path": "audit-source-scene-0.json", "bytes": 4605068, "sha256": "..."}
```

`scripts/collaboration/audit-input.mjs` 在读取前验证普通文件、realpath containment、
bytes、sha256、sceneId、worldRevision。三个消费者
`check-render-stream.mjs`、`check-maximum-world.mjs`、`check-frontage.mjs`
均接入该 helper。

测试覆盖 control、缺文件、错 hash、错 bytes、路径逃逸、替换 source、revision 不一致，
并分别对三个消费者验证缺 auditInput 时非零且 reason 含 auditInput。

## F3：最终签收 evidence 闭包

新增 `scripts/collaboration/check-final-acceptance.mjs` 及其测试。候选 JSON 对所有
runtime、audit runtime、manifest、world、parity、browser observation、evidence、log
记录 path/bytes/sha256，并在状态生成前验证存在性、hash、world/parity/observation revision。
测试覆盖缺文件、错 hash、旧 worldRevision、旧 runtime、旧 audit runtime、观察绑定错误、
缺 log、伪造 main-agent authority 和未标注 timing 测量。

机器候选：`bevy/work/collaboration/ds-final-r1/final-acceptance-r1.json`。
A-02 已改为正确路径 `docs/collaboration/a-t1-main-review.zh-CN.md`。

## F4：A-07 全阶段身份与自然实体实例

- 实体/height 身份检查已从 coverage 分支移到所有 ready 观察的共同路径；
  coverage 去重计数仍只在 coverage 分支。
- 浏览器观测从实际 `nearScene` 场景树/InstancedMesh 记录
  `renderedEntityInstanceIds` 和 `renderedEntityInstanceSources`，不再用写入 Set 冒充几何存在。
- parity 对 `scene.entities` 的 tree/rock 自然类实例增加逐块
  `naturalEntities/naturalEntityIds/naturalEntityIdsSha256`，与 `ecology.plants` 分离。
- 反例覆盖：revisit 清空实体、revisit 替换 height、重复实例 ID、缺失自然实例。

小世界实测：

- `browser-small-check-r1.json`：2 chunks 全阶段 entity/height pass，含故障注入。
- `small-natural-check-r1.json`：真实 natural tree 实例检查 pass；因单块世界无第二块，
  网络重试/迟到响应无法注入，整体保持 incomplete，没有伪造通过。

最终 240 块旧观测不含新的 instance/source 字段，按 hash 重放到新 parity 后明确
incomplete：`final-observation-replay-r1.json`。需要检查员安排一次最终世界的补采；
不要求本轮重跑最大图。

## F4b：性能测量

未找到带 elapsed/peak RSS/sample/budget 字段的原始采样文件。候选 JSON 将 generation
和 gate 都标为 `measurement_pending`，详见
`bevy/work/collaboration/ds-final-r1/measurement-pending.json`。不把文档中的 267.164s/
980MiB、39.200s/1298MiB 当作已绑定测量证据，也不自行重跑。

## 新证据与限制

- 原最终世界、原最终 runtime、原运行证据和旧浏览器观测均保留，未修改。
- 新审计 runtime：`bevy/target-r-audit-r1/release/glyphweave.exe`，与生成 runtime 分开记录。
- 新 final parity：`bevy/work/collaboration/ds-final-r1/final-parity-r1/`。
- 三个 Node 消费者已对新 parity pass。
- 未提交、未暂存、未推送、未改分支/config；未启动其他 agent。
- A-09 的 Quaternius building 来源和 `workerd-windows-64` 许可证元数据仍为明确未核实项。
