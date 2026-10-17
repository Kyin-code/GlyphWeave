# GlyphWeave 项目总交接文档：A/B/C 主线、当前实现与后续计划

日期：2026-09-29  
文档性质：**项目状态、证据索引、开发纪律和后续执行交接**  
适用对象：接手项目的主 Agent、实现 Agent、独立检查 Agent 和项目负责人

> 本文是交接文档，不是新的实现授权。它把当前已知事实、历史例外、未决问题、下一步任务和长期规划集中在一起。任何“已设计”“已接字段”“fixture 通过”都不能自动解释为“真实世界生产链已经完成”。

---

## 0. 给接手者的最短结论

### 当前产品结论

- **A：** 稳定底座历史签收保留，但历史签收附带的内容质量例外不能被隐藏或自动继承为今天的完整质量通过。
- **B：** 整体仍为 `partial`。当前主线是道路 → 地块 → 建筑 → 入口/通路 → 公共/服务空间 → 农田/自然保留地 → 用途/覆盖/所有权账本的真实生产闭环。
- **C：** 未整体完成，必须在 B 的核心接口和实际业务增量稳定后再分阶段推进。
- **G27–G35：** 主要是局部消费者、预览、交互和读图证据工作，不能替代完整世界生成能力，也不能作为 B/C 已完成的证明。
- **FUT-01～FUT-06：** 已登记需求和架构方向，不是已实现功能。

### 当前唯一优先事项

当前不要扩张渲染、材质、GIS、LLM 插件或完整人口经济系统。先完成：

```text
base RoadEdge
→ authoritative RoadProfile
→ RoadNetworkPlan::entities()
→ frontage::stations()
→ candidate site
→ building / ledger 增量
```

当前任务是 B01-R1 T2 的 OPT-B 续行任务。T2 之前的实现已经写入工作树，但**尚未通过完整测试和真实生产链验证**。

### 当前明确禁止

- 不修改 `F:\modenmapbox`；
- 不执行 Git reset、checkout、stash、切换分支、提交或推送，除非项目负责人另行明确授权；
- 不重跑已经耗尽的 512m 额度；
- T3 离线门未全绿前，不使用最后一次 1km 额度；
- 不运行 6×10km；
- 不用旧世界、旧 exe、旧 manifest 或局部 viewer 证据冒充当前源码生产结果；
- 不把文档数量、测试数量、截图数量或配置字段数量当作业务能力进度；
- 不在当前 B01 任务中启动 FUT-01～FUT-06。

---

## 1. 权威仓库、分支、备份和工作区纪律

### 1.1 权威项目

- 仓库：`G:\difyllmwiki\GlyphWeave`
- 当前工作分支：`refactor/authoritative-baked-audit`
- 当前 HEAD：`fb36dc4f9fdd38e6aa9c5a89676a65b96e5001b1`
- 当前工作树：有大量历史 dirty/untracked 文件，**不得把它们当成单轮改动，也不得直接 reset 清理**。

### 1.2 已有 Git 备份

- 备份分支：`codex/backup-before-b-20260929-004242`
- 备份提交：`d7446179a23794ecc357293f0b33ab05d4179d89`
- GitHub 远端已有该备份分支。
- 本地完整备份：
  `G:\difyllmwiki\GlyphWeave-backups\pre-b-mainline-20260929-004242`
- 完整备份包括：Git 工作树、Git bundle，以及被忽略的证据/世界文件快照。恢复前先读 `git-checkpoint.json` 和 `backup-complete.json`。

### 1.3 F 盘项目禁止修改

`F:\modenmapbox` 是另一个项目，不属于本项目实现范围。不要在那里修改、构建、提交或清理。

---

## 2. 原始 A/B/C 目标

### 2.1 A：稳定底座

A 的目标是：

- Rust 是生成真源；
- Manifest、seed、版本和输入可追溯；
- 稳定 ID、对象关系和存量约束；
- 权威数据一致；
- 真实入口和访问图；
- 多 chunk、大图、分片预览与消费者可用；
- 生成结果可以被 HTML/Godot 等消费者读取，但消费者不能自行补造世界事实。

A 在历史文档中有正式签收记录，但仍保留内容质量例外。A 签收只说明底座合同在当时的验收范围内成立，不能证明当前 B 内容质量已通过。

### 2.2 B：真实街区/聚落

B 不是“增加一些楼”。核心是：

```text
道路
→ 可建设地块
→ 地块退界与建筑核心
→ 建筑体量与实际实体
→ 入口/通路
→ 公共空间与服务设施
→ 农田/自然/保护用途竞争
→ planned / realized / unresolved 账本
```

B 的核心验收必须看业务链是否真正增长，而不是只看 `edges`、建筑记录或 fixture 数量。

### 2.3 C：自然—工程—人文闭环

C 的方向是：

```text
自然骨架
→ 规划建设
→ 道路/平台/挖填/路堤/结构
→ 地形与水文反馈
→ 城市/农村/荒野完整覆盖
→ 大图消费、发布、来源和许可链
```

局部工程函数、道路 profile 或预览渲染都不等于 C 完成。

---

## 3. 当前真实业务状态

### 3.1 权威 b8 真实世界

当前已知最重要的真实世界是：

`G:\difyllmwiki\GlyphWeave\bevy\work\collaboration\ds-b-mainline-resume-20260929\runs\b8-current-a01\world`

它由当前源码生成，并绑定到 `frontage.rs` 的 provenance。已独立重读的事实：

- 场景：1000×1000m；
- road edges：46；
- profile 保存道路：20；
- roadside sites：22；
- frontage sites：1；
- development plan building records：22；
- realized buildings：21；
- urban homes：0/120；
- urban shops：1/24；
- land realized：334,257 / 1,000,000m²；
- unresolved：665,743m²；
- ledger status：`incomplete`；
- conservation 独立复算通过。

这些数字说明：

- 生成器确实能生成真实世界产物；
- 账本和保留/缺额口径存在；
- 但城市 frontage、住宅、商业和大面积用途实现仍明显不足；
- B 不能签收。

### 3.2 b8 质量审计

独立复跑结果：

- `scale-audit.json.status = fail`；
- `quality-report` 进程可能 exit 0，但 JSON `status = fail`；
- 主要缺口包括道路规划/栅格差异、建筑计划/实体锚点不一致、住宅最低需求不足、生态缺额、landcover 未实现、大量 rural/urban unresolved 和 yard 未实现。

**质量判定必须同时看 exit code 和 JSON `status`。只看 exit code 是已发生过的错误，今后禁止重复。**

---

## 4. 当前生产链分层盘点

本节采用 HY4 建议的能力分层：

- 生产可用；
- 已接模块但输入受限；
- 仅 fixture；
- 已有接口但未闭环；
- 只有设计；
- 尚未开始；
- 状态未知。

| 环节 | 当前判断 | 真实含义 |
|---|---|---|
| 输入/Manifest | 已接生产入口，但组合覆盖有限 | CLI、Manifest、SceneSpec 已存在；不是所有输入组合都已验证 |
| 输入解析 | 生产可用 | 能把输入转成场景、地形、用途、规则等内部对象 |
| 环境与规则 | 已接模块但默认/硬编码较多 | zoning、water、保护和自然约束存在，统一 RegionRulePack 尚未完成 |
| 地形 | 生产可用 | heightfield、毫米高程和 chunk 链已有真实使用 |
| 水文 | 局部可用 | 有水体/保护/地形查询；不是完整水文、排水和工程反馈系统 |
| 基础道路 | 部分生产可用 | base road 能生成；profile 生产消费链正在修复 |
| 道路 profile | 已接实现但尚未闭环验证 | RoadEdge profile 已写入当前工作树，但测试编译和下游基线合同尚未修完 |
| 地块 | 已接模块但输入受限 | parcel/退界/可建设核心存在，frontage 供给不足会显著限制结果 |
| 建筑/设施 | 真实生成但覆盖不足 | b8 有真实实体，但住宅/商业需求远未满足 |
| 入口/访问 | 模块已接，真实覆盖不足 | access graph 和入口逻辑存在，profile 缺口会使 station/frontage 失败 |
| 工程结构 | 局部生产可用 | 道路 profile、填挖、支撑原语存在；自然—工程—水文闭环未完成 |
| 用途/地表 | 账本可用，内容 incomplete | planned/realized/unresolved 能记录，但大面积缺额仍在 |
| 来源/账本 | 证据机制较强，业务内容未闭合 | manifest、provenance、ledger 能追溯；追溯完整不代表世界质量通过 |
| 审核 | 框架可用，生产结果 fail | 必须保存输入、hash、结果 JSON 和未观测项 |
| 保存 | 已接生产链，但需按候选重验 | world、manifest、sidecar 等产物存在；不能复用旧产物冒充新验证 |
| 消费 | 局部消费者可用 | HTML/Godot/局部 viewer 能消费部分权威事实；完整大图消费未完全证明 |

最脆弱链路是：

```text
base road → profile → entity grounding → stations → site → building → ledger
```

---

## 5. 当前 B01-R1 T2 状态

### 5.1 原任务书

原任务书：

`G:\difyllmwiki\GlyphWeave\docs\collaboration\dsh-b01-r1-t2-opt-b-base-edge-profile-task-20260929.zh-CN.md`

任务书 SHA-256：

```text
05D098A881F74B5E27C9F4FA69A17E76381016A898E0D0806FFCE9AF58E39305
```

### 5.2 已做的代码接入

当前工作树中已接入：

- `RoadEdge.profile: Option<RoadProfile>`；
- serde 默认缺省，旧 JSON 可读；
- `entities()` profile 优先级：
  1. edge 自身 profile；
  2. `extensions` 旧扩展路 profile；
  3. 无 profile 的 legacy 行为；
- 扩展路显式 `profile: None`，保持 `RoadExtension.design` 为扩展路单一来源；
- `base_edge_profile`：使用真实节点端点、`CorridorRequest`、`AlignmentContext::design` 和 `RoadProfile` 校验；
- `attach_base_edge_profiles`：在 level 确定后为基础边尝试挂 profile；
- 继承的道路工程合同未改变：8m road、3m shoulder、6000mm max fill、80mm/m max rise、8m junction；
- 没有制造平面 profile；
- 失败边保留 `profile: None` 的意图。

### 5.3 当前不能宣称完成的部分

1. 测试编译仍有缺字段错误；
2. helper 收集的拒绝列表在生产调用处被丢弃，未进入权威产物；
3. `auto_roads::base_only_plan` 用 `road_profile.is_none()` 判断基础域；profile 接入后会排除真实基础边；
4. `trusted_base_contract` 仍拒绝带 profile 的基础实体；
5. 没有对生产 `entities()` 生成的实际实体完整调用 `validate_entity()`；
6. 高程边界、checked arithmetic、NaN/Infinity 和几何方向边界需要补强；
7. 最小非空消费 fixture 尚未通过；
8. 没有 T3 离线门证据；
9. 没有当前源码对应的 1km 真实生产验证。

### 5.4 独立检查结果

独立检查报告：

`G:\difyllmwiki\GlyphWeave\docs\collaboration\b01-t2-opt-b-independent-review-20260929.zh-CN.md`

独立检查证据目录：

`G:\difyllmwiki\GlyphWeave\bevy\work\collaboration\inspector-b01-t2-continuation-20260929\`

独立检查只执行了一次测试编译：

```text
cargo test -p glyphweave-core --lib --no-run --message-format=json
```

结果：

- exit 101；
- 5 个 `RoadEdge.profile` 缺字段错误；
- 4 个文件；
- 没有执行测试、世界生成、CLI 或浏览器。

实际错误位置：

- `settlement/agriculture.rs`：1 处；
- `settlement/development.rs`：2 处；
- `settlement/landcover.rs`：1 处；
- `worldgen/road_parcel_feasibility.rs`：1 处。

此前交付报告中的“28 处/8 文件”与当前独立编译输出不一致，必须在新报告中更正，不得凭记忆补造缺失站点。

### 5.5 续行任务书

下一步任务书：

`G:\difyllmwiki\GlyphWeave\docs\collaboration\dsh-b01-t2-production-chain-continuation-20260929.zh-CN.md`

该任务授权：

- 机械补齐实际缺少的 `profile: None`；
- 修复拒绝诊断持久化；
- 修复基础道路身份和 trusted base contract 逻辑；
- 对实际 entities 运行完整 profile/entity 校验；
- 补数值边界和异常输入；
- 完成混合成功/拒绝、旧 JSON、扩展路和序列化 fixture；
- 跑 T3 离线门；
- **不运行世界**。

续行任务书 SHA-256：

```text
20DA5EE4DF294220E51D04A5E868A3D7A70BB0850616B7FBE1D3D4D1CCDA1542
```

---

## 6. 当前 B01-T2 的详细执行顺序

### C0：新轮基线

必须先：

- 新建证据目录；
- 重新记录当前源码 hash；
- 机械同步文件逐文件记录 pre-edit hash；
- 把旧轮的首次 pre-edit hash 与本轮起点 hash 分开；
- 不修改旧报告、旧 manifest 和旧台账；
- 保存当前 dirty 状态摘要。

### C1：恢复测试编译

按当前编译器真实输出补字段。机械同步必须：

- 只补 `profile: None`；
- 不改断言；
- 不改测试语义；
- 记录每处文件、行、旧 hash、新 hash；
- 保存修复前后 compiler stdout/stderr。

### C2：拒绝和实体验证

必须完成：

- 把每条 profile 拒绝保存到权威 plan/report；
- 保存 edge ID、拒绝分类、原始原因；
- 序列化和重新读取后仍能看到拒绝；
- 成功边可消费，拒绝边不能产生 station；
- 对生产 `entities()` 返回的实际实体执行 `validate_entity()`；
- 不用手工复制 EntityInstance 代替真实消费者。

### C3：修复 base 身份和 trusted contract

不能再用“有无 profile”判断基础/扩展身份。

必须区分：

- 基础道路来源；
- extension 机制新增道路；
- edge ID 和节点拓扑；
- edge 自身 profile；
- extension design profile。

trusted base contract 要保持：

- 固定节点；
- 固定自然 hash；
- 基础边身份；
- profile/endpoint/geometry；
- 不可在候选阶段被静默重写。

禁止为了通过测试而：

- 清空 profile；
- flatten；
- 删除 trusted 检查；
- 伪造无 profile 基线；
- 放宽工程、保护区、水体或坡度合同。

### C4：最小真实离线 fixture

至少需要：

- 非零 origin；
- 一条合法 64m 以上基础边；
- 两个真实 `RoadNode`；
- 真实毫米地形；
- 合法 zoning；
- profile 成功；
- `entities()` 只有一个对应 road entity；
- `validate_shape` 和 `validate_entity` 成功；
- `frontage::stations()` 返回 `Some`；
- 至少一个 candidate/site；
- 混合成功边和拒绝边；
- 拒绝边不产生 station；
- base + extension 混合不重复；
- serde round-trip 保持 profile 和拒绝诊断；
- 相同输入得到稳定 profile、拒绝顺序和 ID。

### C5：六类拒绝

至少覆盖：

1. 节点或 scene 不匹配；
2. 非水平/垂直轴、奇数长度、长度越界；
3. 工程参数越界；
4. 高程缺样或编码非法；
5. slope/fill/protected/water 不可行；
6. entity endpoint/center/elevation/shape 不一致。

每一类必须记录是 helper 还是下层 solver 拒绝，不能把“构造了一个常量”说成生产路径真正注入了该参数。

### C6：T3 离线门

定向测试范围：

- `road_profile`；
- `road_alignment`；
- `road_network`；
- `road_extensions`；
- `auto_roads`；
- `frontage`；
- `frontage_supply`；
- `road_parcel_feasibility`；
- `parcel_geometry`；
- `services`；
- `development`；
- `landcover`；
- `agriculture`；
- `landuse_ledger`；
- `zoning`。

T3 通过标准：

- 测试编译通过；
- required 定向测试 exit 0；
- 机器可读结果没有 fail/notObserved；
- build 与当前源码 hash 绑定；
- 没有未授权 N/A；
- 所有失败保留原始日志；
- 只在 T3 全绿后考虑 T4。

---

## 7. 资源、额度和账目

### 当前世界额度

- 512m：6/6，已耗尽，不释放、不抵扣、不重跑；
- 1km：5/6 成功，最后一次成功额度保留；当前未使用；
- 6×10km：0/2，当前禁止启动；
- 当前续行任务：世界 0，CLI 0；
- 独立检查员额外测试编译：1 次，与执行者历史声明分开记录。

### 记账原则

每个进程分开记录：

- attempt；
- start；
- processId；
- exit；
- command；
- 输入 hash；
- 输出目录；
- 是否真实启动 Chrome/世界/CLI。

不可恢复的历史次数不估算充数。超预算历史必须保留原样，不抵消后续额度。

### T2 续行软计划

- cargo：软计划 12 次；达到约 75% 时报告用量和剩余诊断；
- 产品 CLI：本轮先不运行；
- 世界：0；
- 浏览器：0；
- 归档：按实际记录。

便宜的离线 cargo 可以继续，但不能无新诊断重复同一失败；硬边界和真实证据纪律优先于“跑满次数”。

---

## 8. K3 与 HY4 外部意见的整合

外部意见已分别归档：

- K3：
  `docs/collaboration/external-review-abc-roadmap-20260929.zh-CN.md`
- HY4：
  `docs/collaboration/external-review-hy4-production-chain-20260929.zh-CN.md`

综合执行计划：

`docs/collaboration/abc-follow-up-execution-plan-20260929.zh-CN.md`

### 现在作为 ABC 共同护栏

- 并行生成确定性；
- chunk 子 seed、稳定排序、归约顺序；
- 坐标、单位、来源和稳定 ID；
- 生成器—消费者几何/性能合同；
- 存储、加载、内存数字；
- 普通地形、道路、河流和实体接缝；
- 黄金场景回归；
- planned / realized / unresolved / conflict / excluded；
- 机器可核验交付；
- Patch parent、版本、输入快照和影响范围字段。

### B 后续窄任务候选

T2/T3 和真实 1km 业务增量确认后，再按依赖选择一个：

1. 轻量人口/需求分母和服务覆盖；
2. 第一版 RegionRulePack；
3. 城乡过渡带 + AbsurdityRulePack + 冲突分类；
4. 基础市政走廊/点状设施；
5. 黄金语料与分布诊断；
6. 稳定命名。

### C/C 后和 FUT

- 完整自然—工程—人文反馈；
- 完整市政网络和动态访问；
- 来源层消费者可视化；
- 大图连续消费和发布链；
- 局部自然语言 Patch；
- 开放 Recipe/规则；
- 跨 chunk 复合设施；
- 环境事实/推断/假设实验；
- GIS 分层重建；
- procedural/assisted/local-edit 重放；
- 历史演化、分支、what-if、diff、merge；
- 完整导出互操作。

---

## 9. FUT-01～FUT-06 交接摘要

| FUT | 当前已有基础 | 最小下一步 | 现在不要做 |
|---|---|---|---|
| FUT-01 局部 Patch | 已有 Patch/不可变修改方向的需求登记 | 先做结构化低风险 Patch、影响闭包、原子提交、回滚 | 任意全域自然语言编辑 |
| FUT-02 开放规则 | 已有强类型对象/规则/Recipe 方向 | 组合原语 → 受约束 DSL → 隔离生成器 | LLM 直接生成并无审执行代码 |
| FUT-03 复合设施 | 已有 chunk/实体/工程方向 | 先定义整体 ID、走廊、端口、局部变换和静态跨 chunk 设施 | 先做动力学、轨道力学或大规模装配 |
| FUT-04 环境背景 | 已登记来源五态和事实/推断/假设分离 | 做可替换环境假设实验，证明参数影响结果 | 只增加不影响生成的装饰字段 |
| FUT-05 GIS | 已登记许可、CRS、单位、NoData 等输入合同 | 先做小区域、许可明确、观测/推断分离的导入包 | 全市重建、把推断冒充观测 |
| FUT-06 重放 | 已登记 procedural/assisted/local-edit 区分 | 冻结已批准 Plan/Recipe/Patch/输入快照的离线重放 | 要求换 LLM 后仅靠 seed 得到同一世界 |

---

## 10. 证据判定层级

所有后续报告必须明确证据层级：

1. **static**：源码/结构静态检查；
2. **compile**：源码能编译；
3. **unit**：单元或定向测试通过；
4. **fixture**：受控最小输入真实代码路径通过；
5. **production-world**：当前源码生成固定尺寸真实世界；
6. **cross-scale**：512m、1km、最大图或多场景验证；
7. **consumer**：权威产物被消费者读取且不补造事实；
8. **independent**：独立检查者复核输入、hash、结果和失败分类。

以下不能互相替代：

- compile 不能替代 fixture；
- fixture 不能替代 production-world；
- production-world 不能自动替代最大图；
- consumer 画出来不能证明生成语义正确；
- 文档完整不能替代机器产物；
- 旧世界不能证明新源码；
- 通过测试数量不能证明需求面积已实现。

---

## 11. 交接者第一天操作清单

### 先读

1. 本文；
2. `docs/collaboration/abc-goals-realignment-and-roadmap-20260929.zh-CN.md`；
3. `docs/collaboration/abc-follow-up-execution-plan-20260929.zh-CN.md`；
4. `docs/collaboration/b01-t2-opt-b-independent-review-20260929.zh-CN.md`；
5. `docs/collaboration/dsh-b01-t2-production-chain-continuation-20260929.zh-CN.md`；
6. `bevy/work/collaboration/ds-b01-r1-t2-opt-b-profile-20260929/checkpoint.json`；
7. `bevy/work/collaboration/inspector-b01-t2-continuation-20260929/inspection-facts.json`。

### 再确认

- 当前 HEAD 和 dirty 状态；
- 当前 T2 源码 hash；
- 旧轮目录没有被改写；
- 当前没有正在运行的 cargo/CLI/世界进程；
- 续行证据目录尚不存在或为空；
- 任何要编辑的文件先加入新 write-set。

### 然后执行

1. 建立新 T2 续行证据目录；
2. 保存本轮基线和写集；
3. 补实际 5 处字段；
4. 测试编译；
5. 修拒绝持久化、base 身份和 trusted contract；
6. 完成 C4 fixture；
7. 跑 T3；
8. 写 checkpoint/report；
9. 等独立检查，不自行宣布 B 或 C 通过。

### 如果必须中断

先写 `checkpoint.json`，至少包含：

- 当前源码和输入 hash；
- 当前任务项状态；
- 实际 cargo/CLI/world 账目；
- 最近命令及 exit；
- 最后错误；
- 已完成和未完成步骤；
- 恢复后的第一条命令。

---

## 12. 常见错误和禁止的“捷径”

### 不要把这些当作完成

- `RoadEdge.profile` 字段存在；
- `cargo build` 成功；
- `entities()` 返回道路实体；
- `frontage` 配置存在；
- fixture 通过；
- viewer 能显示；
- 旧 b8 世界有建筑；
- ledger 有 planned 面积；
- quality 进程 exit 0；
- 文档写了“可复现”；
- 设计写了“支持未来扩展”。

### 不要用这些方式“修复”

- 把基础边塞进 `extensions`；
- 复制两份 profile 形成两个真值源；
- 清空 profile 适配旧 trusted contract；
- 用平面 profile 替代 solver；
- 放宽 slope/fill/protected/water 合同；
- 删除拒绝项或把拒绝计为 realized；
- 把 `unresolved` 改名成“有意留白”；
- 用旧世界或旧 exe 验证新源码；
- 把 `status=fail` 的质量报告按 exit 0 接受；
- 为了通过而修改需求、zoning、reserved、尺寸或比例。

---

## 13. 下一次独立裁定标准

下一次检查重点不是“文件是否变多”，而是以下问题是否由当前源码和固定输入实际证明：

1. 基础道路 profile 是否真的进入 `entities()`；
2. `frontage::stations()` 是否真的从该 profile 生成 station；
3. candidate/site 是否实际增加；
4. 拒绝边是否仍保留为具名、可读、不可消费的缺口；
5. base 和 extension 是否没有重复实体；
6. trusted base contract 是否同时保护基础边、节点和 profile；
7. final terrain、站点、入口和工程支撑是否一致；
8. 相同输入是否稳定产生相同 profile、ID 和拒绝顺序；
9. T3 定向测试是否全绿；
10. 是否仍然没有误用世界额度。

即使这些全部通过，也只说明 B01 当前切片达到交付检查条件；不等于 B 整体通过，更不等于 C 或 FUT 功能完成。

---

## 14. 交接结语

当前项目并不是“只有小渲染包”，也不是“所有大世界能力已经完成”。准确描述是：

- A 的稳定底座有历史签收和可复用基础；
- B 已有真实生成器和账本，但真实街区内容链仍有显著缺口；
- C 的自然—工程—人文闭环还未整体完成；
- 局部预览工作已经提供了消费者和读图经验，但不能替代生产世界；
- 当前最值得投入的最小增量，是让基础道路 profile 真正贯通 frontage、地块和建筑链；
- 交接者应优先保护证据真实性，少做无关扩张；
- 每次任务都要回答“当前代码、固定输入和真实产物到底多走了一步”。

**请严格按后续任务书完成任务，不得因为其他任务无关问题擅自中断或停止部分步骤；上下文压缩、会话切换、摘要丢失或可修复的驱动问题不是跳过必测步骤的理由。**
