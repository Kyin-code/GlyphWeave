# V4.1 B01-G2-R1：纠正三提案粗筛，仅做小范围设计修订

## 1. 先读、不要开发

先读`docs/collaboration/b01-g2-design-inspection.zh-CN.md`及检查员targeted-geometry/nonjunction-contacts.json。当前3提案不能进入实现：P1既违反必要高程包络又与旧路支撑非节点重叠，P2/P3终点是旧edge中段。设计A方向保留，具体坐标需修正。

本轮只读分析/文档，不改任何Rust/CLI生产或测试代码、不调用planner/solver/harness、不bake、不128扫描、不carve诊断、不时钟扫描、不改全局carve。不Git写操作、不启动agent、不动F盘/A，B不签收、C不推进。

## 2. 先重算已知三案，不广搜

复核检查员证据：
- P1 station46，(185,142)自然871，固定根10327/前8m plateau/80mm每米下降给下界7287，fill给上界6871；原P1标blocked，不再首选。
- P1接入支撑与auto.0非端点节点重叠。
- P2终点(352,180)、P3终点(160,180)均是旧道路中段，非shared junction。

小脚本可以实现**只拒绝明显不合法方案的必要条件**，但不能把它叫生产审核/solver。不得复制求解器或据“无粗筛错误”就宣称engineering pass。

必须按真实字段区分：
1. 建筑可视bounds；
2. development完整parcel/yard所有权；
3. road_extensions现行横向width/2+3、纵向length/2+4的闭区间逐点支撑；
4. 固定图节点/edge端点；
5. 其它保护/水体。

路口豁免只适用于双方共有端点节点附近且符合生产半径/高程合同；禁止无条件跳过新段两端9m或忽略旧义务。边界相接与正面积/格点重叠分别说明。

固定根下界应沿整条接入段对全宽natural+fill上界检查（可用简单必要不等式，无需求共享B）；同时给自然下界与从固定端可达上界的必要比较。必须计入现行junction平台和端部padding，不只检查端点/中心线。

## 3. 最多3个局部替代坐标方案

只围绕现有z180窗口带及已识别的合理固定根做局部修订（例如缩短街段避开旧edge中段，或选择完整支撑不相交的接入），**最多6个预先列出的局部变体**进入只读筛选，最终保留0–3个。把初始表写成不可覆盖版本，修改另起文件。禁止再次37946条全场枚举。

每个变体输出：A/B/C坐标、各段宽长和固定根高程、是否已有节点、是否接触旧支撑、相交实体/edge/坐标、必要不等式的首个拒绝点；通过者写coarse_constraints_pass、production_engineering=not_run、realized=not_run。

如果必须切分旧edge、移动旧路、扩大junction合同、改变既有承诺才能用，当前任务不能擅自实现/批准；应标requires_contract_decision，不充当可执行提案。

不强求3个提案；没有任何通过必要粗筛就交design_blocked。不要为凑数字换seed、zone、固定入口、建筑需求/大小/setback/院落/fill/坡度。

## 4. 正确计算目标窗口而非声称24容量

- 从原有效请求读取全部尺寸/insets/spacing，不硬编码另造一套。三个原提案虽然每束10窗，跨束去重24仍部分重叠；检查员对原坐标求得不重叠上限16（未扣工程）。
- 新提案分别列raw窗口数、完全重复去重数、互不重叠可同时选择数，再扣旧parcel/路支撑/新接入段与兄弟段占用。不同方案若互斥，不将窗口并集宣传成可同时放置容量。
- 原16住宅+8店铺仍保留，两个请求共用同尺寸窗口不双计；完整parcel过用途覆盖才可计粗候选。
- per-window方向/zone由坐标和生产几何约定导出：z157北行yaw180、z203南行yaw0；原窗口zone实际urban.5/.6。
- 修正“auto路无profile”旧误述，固定35边中4条graded（2显式+2auto）都有站点。

## 5. 文档和设计决策

新文档必须区分：已知不可行条件、粗筛通过、完整设计未运行、实际实现未运行。P1的原推荐撤回。对剩余案只建议进入最多12个路束状态的后续受控实验，**本轮仍不实现/执行**。

设计B仍风险备选，未经用户批准不变更保护/合成合同。A粗筛失败不自动证明B可行。

修正统计语言：几何枚举确实发生，无规划器调用属实，两者分别计数；script写出用wx拒覆盖，不再默认覆盖后声称create-new。旧文档只追加勘误，旧receipt/分析JSON不改。

## 6. 写入范围与成本

- 新`docs/collaboration/b01-g2-r1-design.zh-CN.md`。
- `docs/collaboration/b01-g2-design.zh-CN.md`只追加勘误。
- 新`bevy/work/collaboration/ds-b01-g2-r1-design/`，只读分析脚本/不可覆盖候选表/结果/hash。

Rust/CLI源码及已验收测试全部只读。数据分析累计120s内、最多6个局部坐标变体、planner0/harness0/bake0；不重复全场枚举或已关闭修订测试。

交付原三案纠错、0–3替代案及必要条件证据、实际枚举数/耗时、输入源码hash、局限和下一步决策；doc-tree:check/git diff --check。完成停止等批准，不直接执行下一轮草案、不提交Git、不启动agent。
