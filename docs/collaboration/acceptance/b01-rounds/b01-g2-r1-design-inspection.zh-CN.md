# B01-G2-R1 设计修订独立审查（2026-09-18）

## 1. 结论

**accepted-design-for-bounded-experiment**：接受修正后的设计作为受控实验输入；只批准 V5 的测试态两段路束实验，不批准生产策略接线或正式 bake。不是 engineering pass，不是已实现 8 地块，更不是 24/24 或 B 签收。B 仍 partial，C 不推进。

当前结果不需要再独立做一轮设计收口。下列粗筛问题与措辞在下一实验任务内纠正，不改旧表/旧回执，不把脚本当通用生产审核器。

## 2. 独立复核（不是只阅读回复）

- HEAD：fb36dc4f9fdd38e6aa9c5a89676a65b96e5001b1。
- 本轮回执 SHA256：236015bb9fc39586f53430f63aad06b4ac9e6e570ea95d78d30b15d18adab6d3。
- 回执内 14 个文件 bytes/hash 绑定全部匹配；7 个冻结 W1 输入匹配；7 份历史回执匹配；6 个生产/CLI 源码与 G1 hash 全部匹配。结论限于这些登记文件，不扩称全仓库历史从未变化。
- 在检查员独立目录复制脚本执行一次，输出 screen-inspector.json 与 worker screen-r1b.json **逐字节相同**，hash 为 0af444e04fc538e56e3b283314461b364ce42e8769c4db1efa4b31ff4cfdc061。复制重放只能证明可复现，因此另外编写 independent-review.cjs 作独立必要条件检查。
- 独立检查从冻结 effective-programs.json 读取两请求真实尺寸/insets，确认 12×10、front4/side2/rear4、setback12、spacing24。完整地块 16×18；没有更改 16住宅+8店铺需求。
- 独立代码按端点/边身份构建旧路支撑；复算 V4–V6 的旧路相交、新兄弟段支撑相交、纵向端部各 4m padding 的自然/填方必要不等式。三案均未发现这层几何/必要高程矛盾。
- V5 四站 18/42/66/90，对应 x=270/246/222/198，z=157/203，共 8 个互不重叠完整窗口；旧路、旧 development parcel、新两段支撑均不与它们相交。两请求共用窗口不能双计。独立复核 zone 和朝向正确。
- B 点三带真正交集为 [1920,7692]mm，与报告相符。仍仅单站必要区间。
- 原 P1：115 格旧路豁免外相交（87+28）、41 站必要不等式失败；P2：接入28/街段228；P3街段266。V1–V3接入28。重放结果支持撤回原三案及 V1–V3。

| 提案 | 两段对旧路豁免外格点 | 新两段间豁免外格点 | 独立含 padding 的必要高程失败 | 几何窗口 | 实际实现 |
|---|---:|---:|---:|---:|---|
| V4 | 0 | 0 | 0 | 4 | not_run |
| V5 | 0 | 0 | 0 | 8 | not_run |
| V6 | 0 | 0 | 0 | 2 | not_run |

此表不是生产 mask/water、corridor solve、PlacementState、最终地形或 landcover/access 审核。

## 3. 未阻断当前固定输入，但必须纠正的事项

### 3.1 粗筛的三个有限实现问题

1. screen-r1.cjs:48 仍硬编码 insets；:216 的 margin 用 road width 而生产 frontage::stations 使用 request.width_m。当前两者碰巧都是12，且冻结请求确为4/2/4，所以8窗数值有效；这不是按任务要求读取全部 effective 参数的通用实现。后续直接用生产请求/stations/parcel函数，不沿用这套近似规则。
2. :307–308 的 intersects 分别检测 profile∩approach 和 profile∩street 非空，不等价于三带共同相交。例如 profile=[0,10]、approach=[0,4]、street=[6,10] 会误报。当前真实三带交集确实非空，不改变结论；复用前改为 max(所有下界) <= min(所有上界)，并加该纯区间负例。
3. 原 elevationCheck 只遍历0..length，不含纵向padding；本次检查员已对V4–V6补了-4..length+4的必要检查，未见矛盾。仍不得说脚本已跑生产 padded footprint 审核。窗口 inter 是闭区间格点相交，不等于文中“正面积相交”，当前8窗有间隔，结果不受影响。

### 3.2 共享 B 必须检查整片支撑高度，不能只比两个端点

V5 两段支撑交集为 [279,171,292,184]。在现 road_extensions::append_one 的逐格 profile 相等规则下，两段沿交集涉及的纵向样本都必须匹配：接入末端至少9m、街段开头至少9m需要同高程（profile 在端外clamp）。仅设置两段 junctionLengthM=8 且 B 高程相等，不自动满足这条规则。

独立交集自然给常值必要带 [1812,7715]mm，尚无粗矛盾；不代表生产确定性求解器会输出匹配曲线。下一实验维持已登记8m参数，不擅自扩大半径/改求解器；若当前生产解在第9米不匹配，按工程拒绝记录，不截断profile或改自然来通过。改变junction参数是新候选合同，应另提案，不偷偷塞进本轮。

### 3.3 证据与容量措辞

- 两份候选表同hash证明内容一致，wx证明该写入拒覆盖；二者不能独立证明全部历史上的预注册时间顺序。脚本当前控制流确实先写表后评估，可如实写这点，不宣称独立时间认证。
- unique路线9（变体6）；worker两次执行对应18次路线评估，不应把9同时解释为累计调用数。<1s为worker报告，本检查员未独立认证历史耗时。
- V4/V5/V6应称“本轮三套备选完整束不可直接叠加计数”。本轮最大单束粗窗口8成立；没有证明复用一条接入后设计东西两支的所有新网络都数学互斥。那是未评估的新设计，不在当前授权内。

## 4. 下一步决策

只做 V5：A(288,96)→B(288,180)→C(176,180)，12m local，两段共同B高程、固定A=8000mm。目标是找至少一个真实新增且不损害旧承诺的parcel；不要求本轮实现24，也不把24需求降成8。

具体见 ds-v41f-b01-g3-v5-bundle-task.zh-CN.md。先用测试专用 harness 调用已验收生产叶子；本轮不新增公开manifest开关、不接生产搜索策略、不改carve/保护/solver/审核。默认关闭由cfg(test)+ignored实现。最大12状态、最多3次候选完整realize，基线realize最多1次，总实验运行预算180s。

## 5. 检查员实际动作与证据

目录：bevy/work/collaboration/inspector-b01-g2-r1-20260918/。
- screen-inspector.cjs / screen-inspector.json / candidate-table-inspector.json / screen-inspector.log：原脚本一次只读重放。
- independent-review.cjs / independent-review.json：另写的绑定、输入和几何必要条件复核。
- review-decision.json：本次限定授权。

无Rust/CLI业务或测试源码修改，无planner/solver/harness/bake/128/carve/clock/browser/max/release运行；无子agent、无Git写操作、无F盘写入，旧worker证据未改。只新增检查员证据/审查/任务书，并向status追加本次结论。doc-tree与git diff --check另存日志。
