# R2/W1 收口独立验收（2026-09-17）

## 结论

**修订一（显式轴优先）：accepted-subtask。**

**修订二（道路几何版本绑定）：accepted-subtask。**

结合前轮已复验的16m/12m生产道路见证，本轮关闭R2/W1收口队列。不再继续分派同类profile轴选择或道路版本加固。该结论不是B整体通过、不是whole-world-quality-pass；frontage0/24与六项内容缺额仍待后续业务任务，原自然案例仍unresolved，C不推进。

主检查员没有修改业务源码、启动agent、执行Git写操作或修改F盘。没有正式生成控制图、最大图、release或浏览器巡检。CLI定向测试内部执行一次自给TempDir bake。

## 1. 基线与绑定

HEAD：`fb36dc4f9fdd38e6aa9c5a89676a65b96e5001b1`。

证据目录：`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-closeout-20260917/`。

- 回执SHA256 `d42d90a3e0d10348be17d6f1e3dd9905243e6a00843d2e76f02e8b9d743b0568`匹配。
- 22项回执path/log/hash/bytes绑定匹配。
- C1 18项、R1 14项，共32项旧世界hash保持。
- R6源码hash保持 `fb5dd93504e29acd609d07643e9276081a30dd536f7a2ae4ec7fa06027baaa34`。
- 未将整个历史dirty diff当成本轮改动；未提交Git。

## 2. 独立测试

| 命令（统一 --manifest-path bevy/Cargo.toml） | 结果 |
|---|---|
| cargo test -p glyphweave-core morphology -- --test-threads=1 | 28 passed，3.79s |
| cargo test -p glyphweave-core road_geometry_revision -- --test-threads=1 | 2 passed，0.02s |
| cargo test -p glyphweave-cli --test graded_geometry_revision -- --test-threads=1 | 2 passed，3.45s |

未重复全core/workspace或道路矩阵。前轮道路生产见证35/35、误差0继续引用，未声称本轮重新生成。

## 3. 显式轴修复独立反例复验

复用检查员上一轮axis-probe.rs，只将新exe和结果写入本轮目录，链接当前生产core：

- 全显式width12/depth16/height10、insets8/16/26：**pass**，不再因默认宽26−32拒绝。
- 仅缺depth：**pass**，保留width12、只推导depth16。
- 仅缺width：**拒绝**，真实需要的26−32不合法。

源码确认缺省分支才调用footprint_width/footprint_depth；显式分支记录来源后continue，严格consumer校验没有移除。对称深度和原三例由本轮独立28项测试覆盖。来源/notes保留显式参数越出profile启发范围的说明。

详见axis-probe.json。

## 4. 版本绑定与A冻结身份

### 4.1 当前生成与审核接线

world_manifest_revision从原manifest编译effective，推导几何tag并纳入身份payload；WorldIndex发布roadGeometryRevision；validate-world独立重新展开期望tag，校验tag和world revision；world-quality-gate首先调用validate-world。

自给CLI测试独立执行：正常发布通过、改v1拒绝、删字段拒绝、恢复通过。不是仅定义常量或信任发布报告。

### 4.2 无profile显式16m输入

只读复用版本探针，内存移除W1控制的profile：

- roadGeometryRevision：`b02-w1-graded-band-v2`。
- 绑定后的revision：`a5961aa920feab600b446c4c6a1353142715fadbe72c2c4d64aa35a430c32392`。
- 无几何键旧payload：`383ec6de8d35ba8ed7cb2aae05af76335807555382c76393a65986f19d8751f7`。
- 两者不同；此前反例已修。详见revision-no-profile.json。

### 4.3 A冻结身份

实际只读文件：
`G:/difyllmwiki/GlyphWeave/bevy/work/milestone-a-final-r51-20260908-v2-abs2/glyphweave.manifest.json`

SHA256：`b023377b93092a21243f249d928c323ae928e5e24fabf215b895cddaf37bd183`。

同目录world.json SHA256：`833453465ca289d7263acd2e6a4dcc9a2f928660560cb1b5579a4156d020249b`。

当前推导tag为None；推导revision与存储值均为：
`bcc8328c0618ac78756a37bd7eaf013db8098a4f9a73aec19e583822242e52c0`。

这证明本轮身份兼容要求成立，不是重新认证A全部工程/浏览器质量。详见revision-A.json。

## 5. 兼容与说明边界（非阻塞）

1. 绑定前的部分控制世界现在会被新版validate拒绝，应继续保留旧证据并明确版本差异，不批量补写tag或篡改旧hash。
2. 实现对**任何有效morphologyProfile**保守绑定版本。文档“profile编译后必含16m collector”并不总成立：没有automaticRoadExtensions时不注入hierarchy；显式程序也可优先。这里应描述为保守绑定政策，而非每份profile输入确实会生成16m道路。该策略不会漏绑本轮需覆盖的道路输入，任务允许保守绑定，不作为新的返工阻塞。
3. quality-report独立入口不应被描述成完整发布签收；本轮版本发布门槛由validate-world及其先行调用的world-quality-gate负责。正式验收沿用组合入口。
4. 不需要为检查员外部诊断把内部AlignmentContext/ZoningMask全面公开。后续确需预检可在现有模块内复用生产路径或提供最小门面，不能建立第二套规划器。

## 6. 下一阶段

返回B-01/B-02的真实生成缺额：可通过道路工程审核不等于有足够完整地块。下一任务围绕**既有需求、道路与合法地块供给**，复用生产frontage/工程/yard/access链，停止重复无关checker强化。

任务书：`G:/difyllmwiki/GlyphWeave/docs/collaboration/ds-v41f-b01-l1-supply-task.zh-CN.md`。
