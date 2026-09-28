# B01-G22包装入口收尾独立验收（2026-09-20）

## 裁定

**G22专项修复通过并收口（accepted for the bounded G22 objective）。本次不再追加修复任务。**

验收对象为：原ground预算耗尽反转的双轴状态修复，以及随后明确要求的“无效frame在层筛选前拒绝”“空/全退化路径不因空区间默认通过”两个包装入口边界。不是任意几何的无条件形式化证明，也不是G19/G20/G21整体验收或真实城市集成授权。

A冻结、B partial、C不推进。旧包状态不倒写；G22当前范围可停止补丁循环，冻结本次结果。无需DSH另交确认稿、勘误包或额外补跑。

## 1. 来源与只读核验

独立审查目录：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g22-boundary-20260920/。

- 本轮清单54/54 SHA256+bytes匹配，合计5,270,970B；manifest8111B /9b92321c89255f4a2c583a0924dc8caa47bdcd629e5d6de65951ba2ed88bafb8，receipt绑定匹配。
- 上轮G22包94/94及其回执绑定仍匹配，未覆盖旧证据。
- core431267B /fe442d464c7c9d4863863a4085e337deb77e2dc89cf290c95e16d1f5afbb91f4。
- support_coverage32557B /a3ab64dcc2c8841429c03c51af8d8cac24145763c2bb7e6d9e114abb5d4e897b。
- CLI源码仍99c90e46…；access与auto_roads保持此前绑定。
- 本次审查期间源码、资源与worker包不修改；最终再次核对hash。

## 2. 两个边界修复独立复验

### 2.1 无效frame在分类前拒绝

检查源码确认structural_support_detail_with_budget在frame.top层比较前，对全部候选调用与certify_band共享的frame_is_usable。

直接链接本次生产库复现：

- top=NaN；
- top=+Infinity；
- top=-Infinity；
- 一个合法deck和一个top=NaN候选同时存在。

全部得到Unknown+Indeterminate，reason=invalid_frame，ground与structure拒绝函数均拒绝。不再把无法分类的部件当成已证明位于其它层。

此验证为生产API/helper层，不声称标准JSON可以表达NaN，也未新增CLI输入开关。

### 2.2 没有可评估路径不是已证裸地

空路径、单点、两点重合、非有限顶点路径均得到Unknown+Indeterminate/invalid_path。证据有显式未评估区间；检查员额外清空该区间数组后，两拒绝函数仍根据全局状态/空证书检查拒绝。

有效重复顶点策略同时验证：路径(6,1,8)→(6,1,8)→(10,1,8)在合法宽面上仍Structure+Certified，两支持门不误拒。

对照：正常无结构有效路径为NoStructuralSupport+NotApplicable，ground仅放过结构支持门、structure边仍拒绝；有效结构宽带通过；预算为0仍Structure+Indeterminate/coverage_budget_exhausted，两门拒绝。权威地形/水/障碍判据未被替换。

## 3. 最终源码的测试与CLI观察

### 核心测试

检查员清除测试导出环境变量，仅执行engineering_connections模块定向回归：**32/32通过**。未运行会触碰旧auto_roads证据路径的全库测试，也不宣称整个仓库全绿。

### 本版7项CLI——检查员直接重建并实测

| 项 | 结果 |
|---|---|
| 原ground窄承托控制输入 | rejected / ground_edge_unsupported_by_structure |
| 同一本地输入增加远处不连通构件 | rejected / ground_edge_unsupported_by_structure |
| 正常裸地build/route | built / found |
| 正常裸地独立verify | verified |
| 最小折返build/route | built / found |
| 最小折返独立verify | verified |
| 身份字段自洽篡改verify | tamper_detected，payloadHashMatches=true，stored_capabilities_resampled失败 |

这7项绑定本轮最终源码；不是用旧二进制补结论。worker声称的本轮观察范围与此一致。

### 没有声称重跑的范围

最小直跑/positive-terrace全CLI、原偏移端点/墙穿CLI、几何/覆盖/payload篡改CLI及final-write-sync在本次独立检查没有再整套执行。相应核心语义在本版矩阵保留，前版CLI证据按原版本保留；不把它们全部改标为本版CLI通过。

这符合本次“只修两个包装入口”的限域任务与明确7CLI预算，不再为追求同一完整矩阵重复追加执行。

## 4. 已通过几何与证据一致性

- 本轮裸地、折返的6个生产plan/elevation/mesh文件与上轮G22文件逐字节相同。
- 检查员比较直跑/折返生成函数、interpenetrates、structure_pair_blocker、structure_obstacle_blocker、walking_volume_blocker与前版函数体，仅归一化行尾后均相同。结合回归结果，没有用碰撞放宽或楼梯几何改动换取本次通过。
- 本轮状态封装/共享有效性检查与必要消费者适配为本次实际变更范围；不声称所有生产代码完全未改。

## 5. 台账与预算裁定

worker最终7项台账与各自report/产物结果相符。3项exitCode标measured、4项成功exitCode标derived；明确保留这种来源差别，不把推导改成历史实测。两次汇总驱动失败及被取代稿保留。检查员独立7次全部直接测量退出码，支持当前最终结果。

关于重复出现的PowerShell汇总缺陷：应作为后续任务的工具健壮性注意项，优先复用经过验证的记录器；本包不再要求单独修改或重交台账。没有证据表明底层CLI观察因汇总脚本问题失效。

worker自报本轮cargo4/4、CLI7/7，G22累计cargo12/12、CLI24/24用尽，历史G20超支1保持；检查员消耗另列，不替worker扩额度。

检查员本次：cargo2（定向回归、CLI构建）、rustc1、生产库边界probe进程1、CLI7；自然/ST18/bake/realize/默认世界/产品源码修改0。

## 6. 验收边界与停点

- G22目标及此次两入口收尾可验收，停止继续追补同一范围。
- 不把局部验收升级为整个工程连接系统在任意复杂输入下均已证明正确。
- 不自动接入ST18、SceneIndex、preview、audit或默认世界，不新增设施或真实世界生成。
- 下一阶段如用户要推进，应另行选择一个明确消费者/人工集成目标与预算；不能从本验收自动推导自然运行或发布许可。
- A冻结、B partial、C不推进；旧G19R3C/G20/G21历史状态继续按原裁定保留。

无需回复收到、补确认稿或新增证据包，保持源码及交付物冻结即可。
