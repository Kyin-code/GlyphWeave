# B01-G10-R1 独立审查（2026-09-19）

## 裁定

T1 access 合同纠正与 T3 profile 内容绑定：**accepted，关闭**。
G-1 / H-G10R1-1：**execution-spec needs clarification；不批准自然执行**。
这不是否定 G-1 可行性，而是待测规则与验收范围尚未确定。B partial，C 不推进；无真实增量。

## 独立核验

- R1 receipt 8330 字节，SHA256 `2ebcd94ce54d71da253d1f2364e233c3af173693780354931bee2249f2d817b2`，9 项 evidence 绑定全部一致。
- source-hashes.txt 11 项全部一致（其中 5 个源码）；G10 原 receipt 与 attribution 两项保持。
- auto_roads.rs 生产前缀 UTF-8 81685 字节，SHA256 `ba6deb959c3e3b913ed2c65a887f4860b1cddf505cfd2ae2fb71758997224d34`。
- 检查员一次 no-run 编译，随后当前测试二进制分别 exact 执行 3 项合成测试：g10_r1_preflight_profile_content_binding、g10_preflight_identity_binding、g9_preflight_provenance_schema，全部通过。无自然输入、无自然重放、无 realize/bake。
- HEAD `fb36dc4f9fdd38e6aa9c5a89676a65b96e5001b1`，staged 为空。哈希只证明所核文件一致，不把它扩大为对全部历史动作的证明。
- 机器核验和合成日志：`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g10-r1-20260919/`。

## 已关闭的技术项

`G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/worldgen/access.rs:334-337` 的 0.30m 仅约束 start/facade 两点；未采样路径不能从 x268 两点外推出全局不可达。R1 的撤回正确。

`G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/worldgen/auto_roads.rs:8788-8803` 已比较 RoadProfile 全部所列字段及逐点 elevations；非端点非中心 +1mm 反例通过。此项不再要求修订或重复测试。

## 待执行规格中的三个问题

### 1. G-1 栅格不是现行规则自动生成的栅格

`G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/worldgen.rs:4315-4326` 生成 anchor 到 road point 的矩形连接 carve，常量 target、blend2、priority2，没有 parcel/路面平带的作用域裁剪。

因此必须选择：现行规则诊断，或 G-1 测试专用反事实生成规则。前者不能证明 G-1；后者即使不改生产文件，也改变了待测候选的生成规则，必须标明 proposed-rule，不得称生产通过。

G-1 还需给确定性定义：路面平带与 parcel 是并集还是交集；连接硬支撑与“只限 parcel”如何兼容；仅修改连接还是也修改 pad/apron；矩形拆分还是格点 mask；边界/priority/blend 怎么处理。“不得改写旧 support”是验收条件，不是生成算法，不允许事后把失败点恢复来冒充既定方案。

### 2. 四点不代表全支撑保持

`G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/worldgen/frontage.rs:540-585` 检查候选影响区与既有实体保护区的全部相关整数格，要求 before == after。四点只能作为哨兵，不能把其它格标 pass。

完整 G-1 验证应使用与该规则一致的受影响交集检查及完整 building+2m pad 检查，并在运行前声明其采样范围/预算；只做四点可作为局部诊断，但验收状态必须 partial，不能当作真实增量前检通过。不可偷偷扩成全场扫描。

### 3. access 失败类别必须有观测依据

`G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/worldgen/access.rs:196-287` 的 route 返回 Option，不能仅凭 None 断言耗尽6000、长度120m、坡度或水障碍中的某一个。直接路径某段失败也不证明全部绕行失败。

保留生产 link.status/path/road_id/length；更细分类只有在有实际计数/采样依据时输出，否则 unknown/not_instrumented。测试专用诊断须独立标注，不改变生产寻路与阈值。

## 给 v4.1 的回复 / 下一步（只补一张执行卡，不再写一轮泛化设计）

T1/T3 接受关闭；旧回执与已接受技术项不重做，台账不重开。H-G10R1-1 暂不批准自然执行。

请以一个新增执行规格文件收敛 G-1：

1. 固定 ST18-R1、北/南两个候选和现有高程；选择并明确 current-policy diagnostic 或 proposed G-1 rule，不能混称。
2. 用伪代码/坐标集合写出唯一规则，含组件白名单、mask/边界、priority、blend、规则来源与 revision；明确生产默认不变，以及未来五处消费者如何共用。若没有确定性规则，直报尚未具备实现条件，不编造可配置入口。
3. 列出完整旧支撑受影响交集及 pad 验证范围；四点仅作哨兵；access 返回值及失败细分的可观测边界写清。
4. 给一个默认关闭的测试专用实现白名单与单次预算，失败即停；本次只写规格，不实现、不 cargo、不自然、不 carve 重放、不 realize/bake，不改旧证据。提交后再决定是否批准实现和一次自然验证。

不要求重写全部历史文档，不要求重复三项已通过合成测试，不批准 C-1 或其它参数搜索。不得把一次失败扩大为所有连接几何不可行。

## 当前项目位置

道路两段 append 和共享路口匹配已有通过证据；station18 no-cut 已解除；目前卡在候选连接 carve 改变已有 BC 保护带采样。局部机制和来源已查清，实际 access 仍未验证。尚未取得首个真实增量，24 地块目标未完成。A 保持既有签收；B partial；C 不推进。