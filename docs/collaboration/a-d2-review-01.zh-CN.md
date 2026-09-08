# A-D2首轮主Agent复验：needs_changes / coverage_pending

日期2026-09-08。55项小测试主Agent复跑通过，但未运行真实浏览器或240块。完整结果日志bevy/work/collaboration/review/a-d2-main-unit.log。

> 最终状态（2026-09-08）：实体/高度覆盖已补齐；最终 240 块浏览器巡检结果
> `bevy/work/milestone-a-final-r51-v2-preview-check-browser.json` 为 pass，
> `entityCoverage=pass`、`heightCoverage=pass`。本文件的 coverage_pending 保留为首轮历史。
候选checker hash9e7d50f8f4d3e8745d1409bf1f01b30d6871a59f1cb5b11d875ddd1300a247d1。

独立脚本bevy/work/collaboration/review/a-d2-independent.mjs；本次原始结果review/d2-independent/unit-ZLeERs/summary.json（完整前缀G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/）。control pass；下列问题实际复现：
1. managed_grass的renderedPatchIds全清空，全部搬到nonMeshPatchIds仍pass。任意patch不能凭观测者声明成为免渲染例外。
2. evidence.elapsedMs=-1仍pass。
3. 故障记录只有status/pass、两个普通token，没有诊断或实际迟到响应交付信息仍pass（当前合成control本身就缺实际故障结构）。
4. --out位于parity.worldFile同world树内，会在任何输入校验之前创建并写入新文件，即使最终incomplete；违反只读输入边界。

## R1定点修复

- 在parity per-chunk增加nonMeshPatches/nonMeshPatchIdsSha256，独立生产者仅计material=access_pavement的patch。观测非mesh例外必须与之精确一致。对access_pavement的实际表现还需accessSurfaces对应mesh存在：补accessSurfaces/accessSurfaceIdsSha256，ID按实际access:[sceneId,entityId,anchorId]缓存key规范；逐chunk来源采用与现有verticesOverlapChunks一致的保守重叠规则。正式几何覆盖不能靠“有多少例外”自报通过。
- elapsedMs整体必须有限且非负，所有时序字段同样。
- 故障回执绑定实际注入URL/目标scene/chunk、old失败/延迟token与新完成token、网络failure具体诊断、迟到响应释放/到达后仍新focus的最终观察；checker不得只检查一个pass字符串和任意两个token。只需要结构化内部一致性，不做签名认证。
- 验证out不在world source/header树或其他输入之内，词法+realpath现有父路径，所有使用新目录先验证；输入解析失败不能在未知受保护目录先创建out。保留wx原子拒覆盖；合法自己的work输出正向仍通过。不要先创建文件再发现越界。

## A-07完整覆盖仍缺

当前只核验生态plant和landcover patch，未逐块核对其他实体实际表示及最终高程传输/来源。不能宣称“实体/高度全域验收完成”。本轮先明确coverageFlags entityCoverage/heightCoverage未验证使总体A-D2不能正式完整pass；随后与D3 source parity结合补齐。可先新增可靠实体/height观测字段和单元测试，但不要为了凑全而将未知实体类型全部豁免。

D3 parity v1尚未发布，本次作为冻结合同的pre-release扩充，缺新增必需字段incomplete；不得默默兼容旧parity为pass。总控之后安排Rust source verifier生成per-chunk可核对字段。原33/55测试保持真实shape正向，不能靠全部不完整通过。

Luna仍原D2写集，测试/HTML/结果文档自己的work，不改Rust/旧A-D1/review。单次定点修复，交候选后主Agent授小浏览器诊断槽；未运行者继续integration_pending，不能自行开展大图。费用/时间不以错误pass压缩。
