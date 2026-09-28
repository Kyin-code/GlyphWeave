# DSH：G21同包收尾——接线、输入有效性与真实见证

用户转发后按原G21范围继续，不新开设施包、不新增额度；无需确认稿或单独勘误包。

先读G:/difyllmwiki/GlyphWeave/docs/collaboration/b01-g21-inspection.zh-CN.md及G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g21-20260920/chain-probe.rs和日志。

## 已接受，保留不重做

25项现有测试通过；新结构求证器拒绝侧向补宽/包络反例；最小直跑/折返/宽平台生产build/route/verify通过；几何自洽篡改及缺字段指定审核拒绝。保留事件扫描方向和楼梯生成几何，不再次改坐标或放宽规则。

## 必须闭合

### 1. ground结构承托接线（原任务第3节，不是新增范围）

build_ground_edge与verifier当前都在同一engineering_connections模块调用旧band_coverage_detail。相同CollisionContext下旧路径报2m，新结构路径对真实1m侧条窗口拒绝。

只对实际依赖结构的承托区间用新求证器和事实，生产/审核一致；普通裸地保留权威地形承托，不要求整条ground由一个owner支持，不改access的地形/水/障碍合同。提供合法裸地、合法结构接驳、缺口或窄承托拒绝的成对回归。

若确需范围外修改，列出具体调用链/文件/必需原因并partial停止；不再把未完成项列incomplete=[]。披露不是免验收。

### 2. 求证器入口和缺口见证

- centre.x=NaN或零along/across的MemberFrame目前返回certified。按任务书有限/非退化前置条件结构化拒绝，不能丢边界后当无限支持。区分helper契约失败与完整CLI输入可达性，不夸大。
- 原侧条缺口witness=(x4.06,z0.5)落在中央面闭边界，已被覆盖。生成真正未覆盖且位于要求带内的点或带开闭信息的缺口区间，再用独立检查断言其不被有效层支持。
- 浮点epsilon实际用于近值合并，不能宣传绝对exact/从不闭缝；保持保守数值失败，不引入大依赖或阈值放宽。

### 3. 验证与证据收尾

保留已通过语义。测试实际25，不是26，按日志具名计数。旧final-write-sync日志实际output_conflict；检查员已分开复现真实sync钩子与预存保护，worker下次汇报应明确哪些为检查员观察、哪些为本次观察，不复制标签混用。没有源码改动的writer不要重写。

源/资源最终版本、测试、CLI绑定明确；计数/来源修正随完整结果交付，不另发确认稿。不回写旧台账、回执或清单。

## 预算和边界

自报G21已用cargo5/12、CLI21/24：最多余cargo7、CLI3，漏计扣减；检查员消耗单列，不给worker增加额度。先规划必要的最终回归/构建与受影响CLI分支，3次不能覆盖则如实partial停点，不能省略门后宣称正分支。

白名单沿G21；不改楼梯正例几何、不改access/auto_roads、默认世界、碰撞/净宽阈值；自然/ST18/bake/realize/发布/新设施0，无Git写操作。

正分支只能在原支持范围及实际最终版本验证完成后提交检查；否则partial。A冻结、B partial、C不推进。
