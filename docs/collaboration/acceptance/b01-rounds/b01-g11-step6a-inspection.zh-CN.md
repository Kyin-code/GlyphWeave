# B01-G11 step6a整包独立复核（2026-09-19）

## 裁定

**accepted-counterfactual-support-pad-pass-access-negative**。四个固定候选在测试专用生成规则下通过旧支撑完整受影响域等值、255格pad及所执行D1前检，生产有界access未找到连接。不是生产候选已通过，不是整个连续地形上全局不可达，也没有真实地块增量。

G1b的肩部硬覆盖问题在G1c/G1d中确实解除；新的已观察阻断在access。不重跑G11补通过，不进入production集成/realize。A不变，B partial，C不推进。

## 独立核验

- 清单17/17、receipt7/7字节与SHA256匹配，执行快照与当前auto_roads逐字节相等。
- 原始源码BLAKE3、规则表BLAKE3与run冻结值匹配；预约文本匹配。
- 生产前缀81685字节/SHA256 ba6deb959c3e3b913ed2c65a887f4860b1cddf505cfd2ae2fb71758997224d34保持。
- 一次no-run编译后，对6项新增g11_preflight逐项--exact独立执行，全PASS。未自然重跑、未修改源码。
- 逐态检查support checked==domain（北20275/南23861）、pad255且mismatch0、完整反事实面读回mismatch0，四候选link connected=false/roadId=null/status=no_walkable_route_within_budget。
- 源码中support域确实枚举全部proposed组件的影响域×所有旧实体保护域；pad和完整面同用base++proposed的生产apply/夹限与毫米编码，access使用完整entities，不混用旧carve重建的生产final-terrain。
- 证据目录G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g11-step6a-20260919/。

## 非阻断修订，合并下一包，不单开文档回合

1. run.json四态firstObservedFailure实际为none，而人读表为access。代码只在D1计算该字段；D2返回后须补上access/technical/budget分类，不能用none概括已发生失败。保留旧run，只附机器更正映射并修下一入口。
2. 发布前新建WallClock::start()，使g1b_publish看到的是新时钟而非整批原始epoch。必须复用最早整批时钟或传递绝对deadline，阶段检查与发布门同源。实际6.022s没有超限证据，故不否定当前四态，但不可宣称晚过期保护已正确集成。
3. graphLinksTotal=88包括4条候选link，其它实体为84条。5个512x512完整面统计成立，另有4个D1局部pad栅格（每个819格）应另列，不能把full-scene计数当所有编码工作总量。
4. g11_access_check当前保存pathPoints而非完整path；本轮失败path均空不影响证据。下一轮保存实际path数组，即使为空也明确标注。

## 下一步

G11任务的D3分支只有support/pad/access都通过才进入最小生产集成/realize提案。本轮四态access均失败，不应把结尾“只交集成提案”当作下一步。应该一次查清生产access的实际失败路径并交一个最小修复提案；不先改阈值或猜全局不可行。

已准备可转发整包任务ds-v41-b01-g12-access-cause-decision-bundle-task.zh-CN.md：观察钩子/差分合成→四态一次自然诊断→搜索终止与局部几何证据→离线必要条件→单一修复提案→归档。普通合成修复和预期失败不反复询问用户；本检查没有执行下一自然任务。