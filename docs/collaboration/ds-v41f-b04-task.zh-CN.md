# DS v4.1f B-04：形态profiles、公共服务关系与现实来源

执行授权与总预算见 G:/difyllmwiki/GlyphWeave/docs/collaboration/ds-v41f-b01-b05-runbook.zh-CN.md。工程部分依赖B-01～B-03的安全接口；资料整理可以在前序遇阻时独立做，但不能把资料完成当生成能力完成。

## 1. 必须交付的工程能力

新增Rust-owned MorphologyProfile（名称可按现有类型约定调整），将背景/形态意图编译成现有frontage、parcel、道路、服务和land-use子程序，而不是在HTML里根据国家名改楼高/颜色。

至少定义并实际验证六个显式形态配置：
1. 紧凑混合街区：较小地块、连续街道界面、商住邻接；
2. 周边围合街区：外侧街道入口、内部庭院/开放空间，与B-01非对称边界一致；
3. 低密住宅：较深地块、前后院、局部服务节点；
4. 组团居住：住宅组团与公共设施/绿地关系，不只是均匀散点；
5. 受地形限制的紧凑布局：遵守现有坡度/水体/平台能力，无法实现时明确拒绝，不能冒充已支持香港全部山地高层工程；
6. 村落—农田：住宅/村口公共空间/市场或服务节点与田块、农路联系。

这些是形态族，不是“某国所有城市都这样”。数值可先作为清楚标注的工程测试参数；来源核实后才标observed，不得把heuristic伪装实测统计或建筑法规。

## 2. Profile字段和冲突优先级

至少包含：profileId/version/sourceCaseIds/evidenceLevel、适用terrain条件、地块宽深与前后侧退界、建筑高度/覆盖率/用途混合、道路层级/间距、公共空间/服务配额、入口/步行关系、农村田块模板和冲突策略。

- 所有米/平方米/比例/计数单位明确，拒绝未知ID、非法范围和无来源的“自动纠错”。层数/高度/楼面面积不能冒充土地面积。
- Profile展开仍由Rust决定坐标；Agent只选profile、背景、范围和需求，不能提供已经绕过验证的最终摆放表。
- 优先级：authored/固定合同与自然保护 > 用户显式最低需求/准确尺寸 > 显式program > profile默认值。冲突不能静默缩楼、删设施或改变土地目标。
- profile不得重复生成用户已显式请求的实体；稳定ID由profile/scene/region/request来源决定，不因插入无关profile或遍历顺序改变旧实体ID。
- 缺省无profile的场景仍走legacy路径；不全局替换原算法。可采用受限参数展开，不创建会随意执行代码的规则DSL。

## 3. 公共服务与开放空间

复用现有支持的convenience_store、school、church、town_hall、market以及frontage住宅/店铺；给设施可追溯的服务对象、步行连接和明确请求来源。

至少在组合fixture中证明：
- 学校/市场/市政等不是仅有一个模型，具有实际入口与服务片区的可达联系；
- 公共开放空间有地表几何、连续通行与归属，不能留一个reserved标签；
- 商业节点按明确街道/服务需求聚集，而不是全场均匀随机散点；
- 村落住宅与田块/农路及至少一个公共节点相关联；
- 显式服务半径若配置，以经过审核的路径距离检验，不以隔河欧氏距离冒充可达。

夜市/摊位/旅游带/滨水游乐等更细角色可登记扩展需求；已有类型不足时标unsupported，不偷偷把医院当学校、把桥梁当贴地road。此轮不导入新模型、不实现营业时间模拟或C桥隧工程；未覆盖的用户最终类型保留到后续列表。

## 4. 现实案例来源表（不是重新无边界调研）

先复用并核实 G:/difyllmwiki/GlyphWeave/docs/20260906-human-settlement-morphology-research.zh-CN.md 和 modern-mainland-morphology.zh-CN.md 的线索。既有“城市一行中顺便提农村/山地”不自动满足独立案例配额。

建立cases索引：
- 中国、美国、欧洲各12条可定位实际案例，分别5城区/4乡村/3荒野；
- 日本与香港历史“6个城区”表述有合计/各自歧义，本轮保守按日本6+香港6列目录，不用等待用户答复；明确这是本轮较宽的组织方式，不伪称用户已确认解释。

每条至少：caseId、地点/可定位范围、urban/rural/wilderness分类、地形/人工工程、道路/地块/建筑/公共空间关系、观察类型、来源URL或本地已引用文献、访问时间/版本、证据等级、适用profile、许可/可再分发边界。材料不足明确source_pending；不要捏造精确建筑比例。

仅只读查询官方规划/地图/开放数据说明、原始论文或项目来源。不超过30个新页面，现有资料可复用；不下载大GIS数据、卫星图、street-view图片包或复制受限正文。官方资料无法核实的参数写heuristic/pending，停止研究并完成可验证工程，不进行无边界搜索。互联网不可用时继续本地整理，不能说已联网验证。

资料覆盖不完整时B-04的researchStatus=partial；生成配置可独立为ready_for_review，但最终B整体仍由主Agent判断，不以存在48条空行冒充案例要求完成。

## 5. 白名单（G:/difyllmwiki/GlyphWeave）

- 可新增 bevy/crates/core/src/settlement/morphology.rs、service_relations.rs；settlement.rs仅导出；
- worldgen/frontage.rs、services.rs、joint_frontage.rs、road_network.rs；settlement/development.rs和B-03新增模块：仅profile展开和真实关系接入，不覆盖前序安全规则；
- worldgen.rs仅program解析/优先级/版本/接线；
- bevy/crates/cli/src/main.rs仅新profile输入验证/独立关系报告，不放宽gate；
- 可新增 examples/morphology/b04-*.json（自写小型profiles/manifest）、bevy/crates/cli/tests/morphology_profiles.rs；
- docs/collaboration/b04-result.zh-CN.md、docs/collaboration/b04-case-register.zh-CN.md和本轮work。

不改原研究文档结论来抹去不足，不引入外部代码/数据/资产或改许可证。HTML不负责选profile/补建筑。新增输出schema的最小读取适配按B-05合同处理。

## 6. 验收

- 六profile的参数展开、非法输入与优先级正负例；同seed确定性、无profile legacy行为、无关需求不重排旧ID。
- seed 7/42/2026的主要对照用Rust内存fixture，至少包含非零origin和受保护/水体阻断；不能只换标签而几何/关系完全无差异。
- 量化profile对照：地块宽深分布、前后院、building/parcel覆盖率、街道界面或组团关系、设施真实连接、农田与村落联系；给实际统计而不是渲染截图。
- 负例：设施计数达标却不可达；profile覆盖显式尺寸/最低需求；公共空间仅预留；跨水欧氏距离误判；重复生成显式设施；不适用地形强行降要求；来源pending却标observed。
- 最多2次≤1km完整bake/gate用于代表性组合；其他组合测试不另跑完整世界。生成几何、资料证据、未支持类型三个状态分开。

结束写checkpoint和b04-result。若工程接口稳定进入B-05；资料未核实项带入最终回执，不消失。
