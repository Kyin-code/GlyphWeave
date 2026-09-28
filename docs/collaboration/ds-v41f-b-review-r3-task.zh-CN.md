# V4.1 B Review R3：联合可行性预检与明确可行的集成正例

先读 G:/difyllmwiki/GlyphWeave/docs/collaboration/b-review-r2-inspection-20260916.zh-CN.md。

目标：停止重复孤立走廊扫描，将道路连接/共享节点高程/完整地块竞争接成可验证流程，并用单独已知可行正例证明端到端功能。原自然案例不修改、不降需求、不伪称通过。B仍partial，A冻结证据不改。

## 1. 必须保留/纠正

- R1 F1/F2/F3源码与旧负例保留，不再次重写。
- 原examples/b-r1-control.manifest.json、自然height、已固定节点、16住宅+8店铺、用途比例、尺寸/insets、fill6000/grade80等约束冻结。
- 原8000mm端点走廊仍应拒绝；7000mm独立走廊pass只是design-only，不能直接用于已有8000mm节点。
- 新诊断字段区分requiredRoadElevationMm、allowedRoadElevationMm、maxFillMm；能定位约束来自哪个节点/坡度传播或横向自然样本。禁止把7120mm称为填方深度。
- 重新计算真实parcel边界/占地，不再用“路长/spacing×2”代替完整用途/边界/冲突检查。尤其原z224南侧、z160北侧越界必须拒绝。

## 2. S：有界联合预检（重点实现）

复用现有Rust内核，不做另一套JS规划器：
1. 导入不可变自然raster、water/zones、已验证road/site和固定node约束。
2. 新节点维护合法高程候选/区间；同一节点在所有相连边的端点高程一致。固定节点只允许原高程，不能降低它以让一条边通过。
3. 将候选逐条调用现有corridor design与真实road_extensions接入检查，覆盖padded footprint、父连接、共享支撑、既有site/道路冲突。单条design pass仍不能跳过接入检查。
4. 为当前网络调用真实frontage候选/完整parcel审核，住宅与店铺共同分配；每个候选明确parentRoad、request、完整边界、yaw、入口、支撑高程、用途覆盖与拒绝原因。
5. 按实际互斥地块集合计算供给，不能对每条路单独数完再相加；稳定排序和有限回溯，记录搜索预算耗尽而不是宣称全局无解。
6. 新候选可以尝试短段/多段转向、合法端点高程或已有道路的其他合法接入点；不新增桥隧/cut能力，不放宽道路宽度/保护/水域/fill/grade合同。

实现位置限 G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/worldgen/road_alignment.rs、road_extensions.rs、road_network.rs、auto_roads.rs、frontage.rs、parcel_geometry.rs及其定向测试；可新增worldgen/road_parcel_feasibility.rs，worldgen.rs只做模块导出/必要接线。不得改成功判据、自然生成算法或批量格式化。

优先内存构建一次AlignmentContext重复查询，最多256个新联合候选状态，不启动几百次cargo run/重复读整个scene。可只读复用R2已成功design，但每条必须核对request/natural/zoning并在真实网络检查中重验。预算内找不到则输出best legal witness与missing，不自动扩大搜索。

本轮产出简明JSON：输入来源hash、固定node列表、新node/edge及接点高程、候选地块与冲突/拒绝原因、联合accepted count、搜索边界/状态数。只为实际选中及主要拒绝保存详情，不重建通用证据系统。

## 3. P：建立独立正例，不篡改原问题

上轮纯Rust24地块fixture使用512m全urban合成平地，不等同原128×128m都市zone的自然场景，不能复用其数量当原场景通过。

现在允许新增独立的“已知可行集成正例”测试。必须与原自然案例使用不同ID、文件、自然源来源标识，并在新结果中明确用途是验证管线，而不是解决冻结自然案例。

- 首选已有显式输入接口表达可行地形/zone；先固定输入与具体合法node/edge/parcel几何见证，再运行端到端，禁止不断换seed找绿。
- 如果当前CLI不能表达测试自然raster，不修改原生成地形、不增加生产环境“跳过审核/全平地”捷径。允许只在测试harness里注入显式合成terrain/zone，复用生产道路/工程/实体/地表/审核函数，完成持久化产物的独立重建测试；必须标为integration_fixture，不得伪称`generate-world` CLI正例通过。
- 对CLI缺少输入/接口的事实给最小设计建议，单独列CLI_not_proven；不要因为要保持“CLI通过”措辞而伪造manifest或手写scene绕过生产验证。
- fixture需实现16住宅+8店铺（各精确尺寸不变）、24完整520m² parcel、固定入口和对应实际yard surface，并有collector+local至少两级连通路网。沿用现有工程限制和来源/负毫米审核，不能用mock审计返回pass。
- 删一个yard patch、改一处固定入口/共享node高程、制造两个parcel交叠，都要由独立生产审核拒绝。
- 正例布局预先验证无几何矛盾后冻结；失败时修代码或记录无法表达的接口，不静默缩小需求。

允许新增bevy/crates/cli/tests/b_feasible_control.rs（如使用真实CLI）或bevy/crates/core/tests/b_feasible_pipeline.rs（如果只能测试harness），以及仅测试所需的最小公开/测试构造接口；不得添加生产审核绕过。可新增examples/b-r3-feasible-control.manifest.json，但只有现有真实CLI能表达时才创建，注明独立测试需求/来源。

## 4. 验收和执行范围

先定向单元/小集成，不全workspace/release/大图/浏览器。

必须覆盖：
- 原8000/8000走廊拒绝，7000/7000设计pass但接原8000父节点拒绝；
- 同节点端点不同高度拒绝；固定node不可被重规划覆盖；
- z224南侧与z160北侧urban越界拒绝；
- 多条路共享地块供给不重复统计；住宅优先不能吞掉已保证的8个店铺需求；
- 平面可容纳但support/fill不合法仍拒绝；
- 可行正例24/24、两级实网、真实yard来源审核通过，单变量破坏仍拒绝。

生成配额：原自然场景只有联合预检确已找到合法24个候选网络才允许1次512m生成+独立gate；无见证就不bake，原场景保持unresolved而不是再发同一轮扫描。独立正例真实CLI生成若可用最多1次；两次总配额各120s/4096MiB，harness定向集成另记录规模/耗时，不能冒充完整CLI。

一份结果同时列：originalNaturalCase、syntheticOrAuthoredPositiveControl、cliIntegration、jointPlanner、contentStatus，各自pass/fail/unresolved/not_run。禁止把独立正例成功合并成原自然案例成功，禁止以两种案例混合证据签收B。

## 5. 交付

新增 docs/collaboration/b-review-r3-result.zh-CN.md，证据在 bevy/work/collaboration/ds-b-r3/，提供输入/源码/runtime绑定、真实命令/退出码、候选见证及拒绝、原场景未解原因、正例覆盖的生产环节与尚未覆盖的CLI环节。

不重新运行R2所有799请求，不补造缺失的旧失败日志；旧搜索只能列历史设计线索。保留旧报告原文，新结果修正高度术语与容量计数。

不git add/commit/push/reset/clean/checkout/switch/stash、不启动其他agent、不修改F盘及A冻结材料。完成后交ready_for_review或partial，不签收B、不推进C。
