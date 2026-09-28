# V4.1 B01-SUPPLY-R3：接齐真实final上下文与旧parcel来源

## 1. 两项限定目标

先读`docs/collaboration/b01-supply-r2-inspection-20260917.zh-CN.md`。A事务、B滚动承诺、C1共享道路审核已经独立验收，冻结保留；不要继续改这些算法。

只在G:/difyllmwiki/GlyphWeave；不Git写操作、不启动agent、不改F盘/A冻结材料、不签收B/C。旧world/receipt只读；不重跑128追加或carve诊断、不改全局carve/自然算法/fill/坡度/支撑容差。

## 2. C2：可信生成上下文必须到达实际供给入口

目前auto_roads两个realize_candidate调用仍传0，独立诊断传42；不能以诊断pass替代真实入口接线。

### 实现要求

- 从正式WorldManifest/scene上下文传递真实effective seed（world.seed XOR scene.seedOffset）、origin、自然来源/必要精度，沿worldgen→road_network→auto_roads→realize_candidate同一路径到最终编码器。
- 不从样本反推seed，不从style中另造用户可任意覆盖的“权威seed”。类型化内部context或小型factory均可，保留现有legacy调用语义；v3需要但缺上下文时明确拒绝，不静默使用0。
- 删除17点匹配来决定production/synthetic的来源选择。合成测试明确传SyntheticRaster后端/工厂；生产支持解析seed源时显式使用该源；已编辑/导入raster若有既有合法路径则按明确来源处理，不以解析重新生成替代传入源。尚无接口则fail-closed/unsupported，不能猜测。
- 正式audit重建也需使用同一上下文，不使来源复算重新进入v3时换seed。保持共享nonrecursive审核避免递归。
- 不改数学地形/carve和量化阈值，仅接已有生产生成路径。若v3已发布语义/身份需调整，明确版本边界；A无v3路径保持既有revision。

### 验收（小fixture，不是独立helper演示）

1. 通过实际v3入口运行至少一个非零seed、有非零scene.seedOffset的生产天然小场景；观察baseline/candidate实际收到的seed和finalSource，不能仅直接调用realize_candidate传正确seed。
2. 同候选计划：供给final与正式编码路径在全部小fixture栅格或其完整hash一致，包含origin与毫米量化；不通过增加容差。
3. 缺context/错context明确拒绝；只改17探针以外一点的天然raster不能被错误当解析源覆盖。Synthetic模式显式可用且不伪称production-encoder。
4. 保留前轮正例，无需重跑全部道路矩阵/80时钟扫描。

## 3. C3：完整旧parcel基线，不拿frontage信用集合充当全部旧资产

### 实现要求

- 区分三个来源：进入阶段时原有全部development parcels；本阶段已承诺新增parcels；正在评估的本次新增parcels。
- 从可信baseline development/realization建立全部旧parcel ID、owner、yard缺额表，不仅session.commitments（后者只含frontage需求）。每轮成功后增加新承诺，失败不变。
- 请求realized count仍只由明确request地块贡献；原21义务加入保护表不等于满足16+8需求。
- 每个旧parcel缺额不得比其保护基线增加，旧parcel身份/owner不能偷偷改变；旧13m²可原样保留。每个新增parcel yard缺额必须0。
- 不能把旧院落全部标成已实现、不能删除旧义务、不能忽略审计map缺失项来假装0。拒绝不一致owner/source映射；正常真实0可通过明确来源验证。
- 非yard总量粗指标先如实保留，不扩展全地表逐来源系统。

### 验收

复用检查员yard-probe中两个旧缺额4和9：
1. 旧4/9不变，新增parcel yard0且其它验证全部成立→接受新增信用。
2. 旧全部修复，新parcel缺13→拒绝。
3. 一旧parcel减少，另一旧parcel增加，总量不变→仍拒绝发生增长的那一项。
4. 原21义务受保护，但frontage credit仍0；成功新增一个后仅相应request+1。
5. 两轮新增parcels都纳入保护，回退不改变既有A/B已验收事务。

至少一条从production development+landcover realize/verify取得真实parcel映射/缺额，再走守卫的fixture。可用已有只读world产生旧map，无须重烘焙或扫描道路；其它反例允许小型验证结果注入，明确范围。

## 4. 白名单

- `bevy/crates/core/src/worldgen/frontage_supply.rs`：明确final源/context、旧parcel快照、局部测试。
- `bevy/crates/core/src/worldgen/auto_roads.rs`：仅真实context参数传递、yard分类接线/定向测试，不改已验收事务核心。
- `bevy/crates/core/src/worldgen/road_network.rs`、`worldgen.rs`：必要context贯通/审核重建及小型生产入口测试；移除重复#[test]。不得大拆文件。
- `bevy/crates/cli/src/main.rs`：若独立来源重建需要，最小context接线。
- `bevy/crates/core/src/settlement/landcover.rs`：仅必要的parcel身份读取/附加信息，不改阈值、面积与所有权语义。
- 可新增`bevy/crates/cli/tests/frontage_supply_source.rs`自给小测试。
- 新结果`docs/collaboration/b01-supply-r3-result.zh-CN.md`；R2结果追加勘误。新证据`bevy/work/collaboration/ds-b01-supply-r3/`，create-new。

其他已验收R6模块/自然算法/资产/依赖/lock/渲染只读。若需要超范围变更先报告最小接口阻断，不自行全局改语义。

## 5. 预算与交付

只跑C2/C3相关测试与必要共享叶子小回归，不重跑80时钟扫描、全auto_roads、全CLI/worldgen/settlement/workspace、release、浏览器、最大图、冻结128扫描、carve诊断。

单测试120s，首次必要编译300s。小内存fixture优先；为实际生产来源等价允许最多一次最小合法尺寸≤512m TempDir bake，记录必要性和次数，禁止正式世界生成。缺context不要为尝试修复而反复大图。

纠正R2文档/新回执：冻结扫描确有1次、carve诊断1次，不能写0或false；不要改旧receipt字节。共享叶子同名测试2次是重复注册，不是两种用例；同坐标扰动不称两类覆盖。

交付C2/C3分别状态、真实v3入口seed/source证据、完整小fixture等价、旧新yard反例、旧文件hash、源码/命令/exit/log、bake次数与明确限制。通过doc-tree:check/git diff --check后停止，仍不签收B、不提交Git、不启动agent。
