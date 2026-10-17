# B01-G19R2C收尾候选独立审查（2026-09-20）

## 裁定

**component-progress-verified / closure-not-accepted / execution-budget-overrun-disclosed**。

已确认新helper、部分有限变体、独立重采篡改反例、9项测试与人工CLI进展，保留这些工作。但不接受“五项收尾全部落地”或将剩余问题仅归为旋转/复杂形态：在轴对齐平台上，方向净宽仍会误放行；真实最终写入错误的清理仍未修到生产分支；原17断言表有无效替代，不能以文件存在测试代替回归。

不回退整包，不新增功能要求；以下属于原任务尚未闭合的不变量。暂不接ST18/默认SceneIndex/preview/audit，不签收B。A不变、B partial/C不推进。worker自报超支2次cargo如实记录，不追认授权内执行；原额度已耗尽，不能沿旧nextAction继续追加运行。

## 独立执行与绑定

检查员目录：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g19r2c-close-20260920/。

- 清单84/84 hash+字节匹配，receipt的manifest绑定匹配。
- core268941B/SHA256 3406e3f72645858f805b3ef338ef740d6fe3458279f8ea47a6e0b6e28ec2580b；CLI18300B/e543062097467abe0f503a80d351feace7ed967b026078473760e6eb975ad2cc。access/auto_roads保持前轮版本。
- 检查员2次cargo：重新编译CLI成功、当前9项core测试全部通过。包括45° MemberFrame helper测试、4种有限变体、重算hash的能力伪造被独立重采拒绝；这些不再列作“尚未写”。
- 检查员5次CLI：独立窄长平台与合法宽平台各build+verify，另final-write注入一次。均为新目录小型人工输入；自然/ST18/真实世界生成全0，产品源码与worker证据未改。

## P1-F1：生产方向净宽仍未接入，有轴对齐误放行

代码：core engineering_connections.rs约512–610 validate_port_membership已用member_frame.contains检查中心点；但原端口宽度比较被删除，没有实现端口截面覆盖。MemberFrame::clear_width_m约3209只在helper测试调用；refresh_walkable_capabilities约3684仍是clear_width_m: structure.width_m。

生产CLI反例narrow-directional-deck：
- 16×16平坦人工地面；单一接地platform top=1m，centre=(8,1,8)，widthM=6（x方向）、depthM=.6（z方向）。
- 两端口(6,1,8)→(10,1,8)，声明宽1.6m，沿长边x方向行走；实际横向z只有.6m，当前行人要求1.5m。
- 结果built/found/verified。把depth改成1.6m的wide-directional-deck-control也found/verified。

这不是旋转测试没写，而是判定用的是输入尺寸字段而非路径方向的真实支持宽度。helper算出了矩形横向宽度，并不代表生产边消费了它。

必须把边/端口的实际截面或通行带投影到真实支持多边形，核验整带受支撑；包括端口靠边、平台转向、跑段/平台的方向变化。不能只取一个结构width或一个固定的member轴，更不能缩小所需通行宽度。

独立验证端同样需要重算结构边的支持面覆盖，不能只检查ground带宽而对landing/stair信任生产授予能力。此反例已由当前verify误接受。

## P1-F2：接触判据的剩余AABB并非可忽略装饰细节

build_ground_edge约3009起仍从SolidBox.min/max构造轴对齐terminal_frames；corridor_blocker约3431对terminal区间、owner集合、部件角色作匹配，再用任一frame包含固体角/中心及任一端点退化band命中决定continue。

这不是“该端口→该部件→该层→该真实接触边界”的一一绑定，仍可能把结构中的另一个frame用于当前solid，也未精确限制豁免交集仅在实际接触面。

本轮未追加旋转误放行实验，不将代码风险冒称已复现；但这是原支持范围的待验收条件，不能靠remaining里写一句AABB就允许真实接入。与F1共享同一真实支持几何实现后，补有限旋转/反向/始末两owner/邻近部件对照。只需覆盖本模块宣称支持的形态，不要求做机场桥梁等新设施。

## P1-F3：final-write注入修复了测试分支，没有修正常写入的错误归属

CLI约317–328：
- 注入分支fs::write(out, partial)成功→created.push(out)→主动Err，所以cleanup能够删除。
- 正常分支仍write_new(out, serialized)?→created.push(out)。write_new内部create_new/open→write_all→sync_all，任何后两步Err都在created登记之前返回。

因此真实文件已创建后的write_all/sync_all失败仍可能留下未登记清理的最终路径。现有注入故意在登记后失败，不能证明正常分支已修。

检查员重新运行该hook，确认它确实拒绝并清掉视图/bundle；这里只否定由hook推导“实际写入失败也全部安全”，没有模拟磁盘满或中断。

修复应在同一生产写入原语内管理创建所有权/失败清理，或使用可靠临时文件+原子发布；不要在成功取得文件所有权之前把路径放入删除集合，以免create_new冲突时误删别人文件。故障注入应在实际写入原语的创建后/写入中/sync分支触发，不能另写一条更容易通过测试的路径。

最终时钟门已移动到最终写入后，代码修改成立；它仍依赖正常错误分支能正确清理，不单独当成事务闭环证明。

## P2-F4：17项映射是文字索引，尚不等价于恢复原断言

r2c_seventeen_assertion_mapping_has_its_cases约6934只assert相关JSON存在，不执行那些被列为保留的17条语义。当前矩阵虽执行22case，但报告存在逻辑不同的替代：
- “去掉填方后不可走”被wheelchair-low-solid/ground-through-solid-platform替代；这两项是碰撞反例，不是授权工作面缺失的采样/通行反例。
- “陡坎不可达”被ground-overhead-obstacle/stair-inside-solid-obstacle替代；实体障碍拒绝不证明地形坡度判据。
- 保护域/挖填授权、挡土/排水unsupported、承托越界等以“代码路径未改”“分支存在”作保留依据，不等于已运行回归。
- reverse-flat验证双向反向，不自动覆盖单向边不能逆行。

不要求17个原测试名字原样恢复；允许合并为少量测试。但必须执行原有效前置条件、动作和具体断言，尤其拒绝类别/工作面不半提交/支承/几何网格等，不能只靠资源文件存在或最终status枚举通过。

## 保留的有效进展

- MemberFrame使用顶面四角形成局部坐标，45°面包含helper正反例成立，端口中心点检测不再直接用AABB。
- 0.25m统一实体豁免删除、连续区间带、真实climb/descent与label前驱、各能力ground独立重采等前轮已核验成果继续保留。
- 新的能力篡改测试确实改rules和capabilities并重算提交hash，且断言stored_capabilities_resampled=false，测试路径有效，不再是仅靠hash错误拒绝。
- CLI只跑关键例、其余在core矩阵执行本身不是缺陷，不要求22case全部重复CLI。
- 不要求为本次验收引入新可配置采样分辨率。若合同固定0.5m，优先验证非整段长、不同采样相位/边界位置和支持范围；“未做多分辨率”不是额外扩功能门槛。

## 预算和交接

worker本次自报用cargo5而获准剩余3，累计已报32/30，超2，属执行偏差；如实披露可保留，不能将新一轮额度倒用于解释旧超支。CLI8是本次收尾批次，历史调用继续保留累计口径。

不自动新增额度，不默认继续在这个任务内重编直到通过。需要继续时，只批准一次明确范围的闭环修复：真实支持截面接到生产边与验证端；接触身份绑定；统一写入原语失败归属；恢复有效旧断言。不要重新拆成连续“修一个helper→交付→发现未接入”的小包，也不要扩展城市类型。

当前源码/测试资源与证据保持，不另交确认或历史勘误；这一审查直接覆盖“收尾五项都完成”的表述。后续仍以真实几何、生产调用和独立反例闭环为验收，而非helper存在或测试数量。