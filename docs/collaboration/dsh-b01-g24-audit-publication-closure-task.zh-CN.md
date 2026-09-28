# B01-G24 同包续作：审核覆盖、只读发布与证据绑定收尾

日期：2026-09-21。用户要求“检查并给出下一步任务”，本文件为明确执行授权。无需先回确认稿；一次完成限定修复、测试、最终CLI与绑定后交检查。仍是G24，不重置过去12次cargo/18次CLI成本。

## 0. 验收基线和范围

依据检查报告：G:/difyllmwiki/GlyphWeave/docs/collaboration/b01-g24-continuation2-inspection.zh-CN.md。

接受旧指定核心修复进展，不重做BoundScene、双地形裁剪、实体保留、道路关系或楼梯。检查员当前源码82项engineering_过滤全绿，缺marker最终版拒绝与去BOM缺natural已补证；不得冒充worker本轮观察，也不补跑历史二进制。

本轮只修：
1. 格式完整性与所有声明字段的审核覆盖；
2. 四阶段状态、数字语义和有界输入；
3. audit报告只写新路径、导出原子所有权与失败清理；
4. 最终原始运行记录的递归绑定。

源码基线：
- core engineering_context.rs：104911B / 69fbd3a61334ace886250330be7d7d458e1d844e59a7d5c1a8a4364259844d69。
- CLI engineering_context.rs：19603B / eb18860b3f658d9538228750335e8209f604a8bcd4a440edb2416dcbf285cbb2。
- core lib.rs：516B / a031f15cb29d725541309a887939f9a921abfce2e2873f0ef8d69a39239f1429。
- CLI main.rs：134612B / 07a054d68436cc7bc0f31050a8ba136754350bbd678e924dd5e6c3475c66e374。

白名单：core/CLI engineering_context.rs及其具名测试、必要的模块内辅助文件。main/lib仅必需注册；尽量不动。不得改engineering_connections/support_coverage/access/auto_roads/engineering_preview/viewer、既有高度格式、几何/安全阈值或固定来源。不得因修审核引入默认世界消费者或第三条用户命令。

## 1. 格式规则不是待审声明的剩余集合

先在同一代码规则中明确当前包合同，导出与审核共用路径/角色/版本规则：
- 当前三个必需包文件：engineering-context.json、manifest.json、COMPLETE.json；完成标记最后发布、不循环自哈希。
- 外部原来源文件集合与包内文件集合是两回事。sourceBindings/requiredFileSet必须对独立重建结果校验，不把外部来源缺失伪装成包内文件删除实验。
- 逐项检查schemaVersion支持性、字段存在/类型、路径规范/固定引用、声明唯一性、context哈希、marker的manifest哈希及所有声明关系；必需项不能因为删除声明而消失。
- manifest={}、marker哈希不符、未知schema、错误contextFile/manifestFile、缺字段/重复条目/非法类型/未声明额外文件必须失败，不能只检查JSON可解析或文件个数。
- 包根读取同样应有根内/链接约束及大小帽；报告不属于被审包，不让audit输出变成包内未声明文件。
- 不把合法数字文本表现或当前软件版本无关的差异误认为几何篡改。支持历史格式时必须显式分派，未知版本拒绝，不能先按当前版部分比较。

优先固定检查员反例：空manifest+自洽marker、marker.manifestSha256全0、额外文件。新鲜导出为正对照。

## 2. 每一个输出字段都必须有审核归属

在报告或测试资源列“输出字段→规则/指定审核阶段”完整映射，不仅比对现在选中的几组字段。

至少覆盖遗漏字段：schemaVersion、readOnly、checks、limitations、accessScope、entityKindCounts、unmodelledKinds；已有sceneBinding/crop/entities等继续保留指定重查。未定义根字段（如伪造safetyVerified）不能被忽略。

要求：
- 检查员“checks由notEvaluated改pass、删除limitations、readOnly=false、添加safetyVerified=true”的自洽包必须被拒绝；版本v999亦拒绝。
- 新增参数化单字段反例，而不只保留一份组合篡改；同源重新导出仍通过。
- 检查字段的值，不只确认有此键。unknown/缺字段/错误类型不默认空集合或pass。
- 保持sourceObservationOnly/notReverified、未消费规划子树限制及无工程安全验证的口径。
- 同一份context的所有语义字段均有归属，避免每轮换一个漏字段继续通过。

## 3. 四阶段真实状态、数值和一次重建

- 只有独立检查实际运行并满足条件才pass；阶段未执行为notEvaluated，已确定的错误为fail。每个阶段恰一条，不因expected window冲突出现两个sourceBinding。
- 独立来源在一次audit内重建一次并复用结果；不用两次读取得到不一致的状态。
- 来源无法重建时，依赖它的parity和完整性子检查不能pass。若包内必需标记已明确缺失，contextCompleteness仍应fail，不能被notEvaluated掩盖。把依赖图和聚合优先级写成测试：已证失败优先，其次未评估，全部完成才通过。
- 输入window用checked范围转换，拒绝i64→i32截断、负尺寸、超帽；源/包大小和路径检查失败结构化拒绝。
- 建立统一的类型化或精确规范化比较。8.0与8、正常JSON重新编码不应自行制造实体几何差异；不得用宽松epsilon、字符串格式强制或有损整数转浮点掩盖真实改变。毫米样本与身份ID保持准确。

至少测试：缺source、缺marker、两者同时缺、坏context、未知版本、expected-window不符、数值等价表示和真实数字变化。完整四段报告形状应稳定；不能依赖CLI退出码替代阶段证据。

## 4. 只读audit和创建所有权发布

### 4.1 报告路径

- --report必须是全新文件，使用create_new；预存即拒绝且字节不变。
- 在任何写入前排除源根与被审包根之内的报告路径，处理规范化、相对路径、大小写/平台别名和重解析点，不只比较传入字符串。
- 明确拒绝report指向engineering-context.json、manifest、marker或任何源文件。即使审核得出pass，也不得写坏输入。
- 受控测试仅用本轮自有临时副本和哨兵，不在真实固定来源上做破坏实验。

### 4.2 导出事务

- 去掉exists→create_dir_all→fs::write→失败整目录remove的所有权假设。
- 原子取得新输出目录所有权，payload文件逐个create_new；创建冲突不登记所有权。
- 只清理本次实际创建且仍可确认归属的对象，不能递归误删后来出现的外来文件；无法安全清理时保留不完整状态并明确报错，但绝不留下可验收的完成包。
- 完成标记最后写；真实write/sync失败和中途失败须被处理。故障钩子或测试抽象落到实际文件原语，别仅在全部写完前提前return伪造清理证据。
- 创建冲突/外来哨兵/中途失败用确定性测试，不靠竞态概率。禁止在Windows跨shell拼接删除命令。

## 5. 测试与最终CLI矩阵

先完成具名单测与参数化字段覆盖，再最终构建CLI。测试可在CLI模块或局部core审核helper，不能只有PowerShell比对期望字符串。保护回归用engineering_过滤，不运行写固定历史路径的auto_roads全套。

本版最低20次CLI（按真实进程逐次计数；预期失败也计入）：
1. 固定256,32,64,64新目录export。
2. 新包audit四段pass。
3. 第二新目录同源同窗export，确定性比较。
4. manifest={}且同步marker哈希 → completeness拒绝。
5. marker.manifestSha256错误 → completeness拒绝。
6. context未知版本、同步哈希 → 版本检查拒绝。
7. 自洽伪造检查状态/限制字段 → 指定规则拒绝；单字段细分在核心测试覆盖。
8. 未声明额外包文件 → completeness拒绝。
9–12. 自洽natural、final、删实体、错roadId各一次 → 指定parity拒绝，其他不相关阶段不出现伪失败。
13. 缺完成标记 → completeness fail，其他可执行阶段如实。
14. 包内必需文件删除并同步修改声明 → completeness fail（不能拿外部source删除代替）。
15. 无BOM缺natural受控fixture export → 缺自然源明确拒绝，无完成包。
16. 不可重建来源的audit → sourceBinding fail，依赖项notEvaluated，不得completeness假pass。
17. 预存export目标 → 拒绝、哨兵不变。
18. 预存report文件 → 拒绝、哨兵不变。
19. report别名指向待审context → 拒绝、包字节不变（检查员自副本反例形状）。
20. 越界window export → 拒绝、无成功产物。

最多4次余量供一次真实发布原语故障CLI（若选择暴露测试钩子）、数值等价重编码CLI或必要失败重试。源根报告别名、重复阶段、单字段状态篡改、参数类型与发布故障均须有具名测试，不能因CLI次数有限而不验证；不强制为测试新增用户参数。

检查员现有缺marker/缺natural观察可以引用，但新代码改动后的受影响矩阵必须绑定最终二进制。不要复制旧report充数。保持所有失败和重试的原始记录。

## 6. 新预算授权与累计口径

旧G24：cargo **12/12**、CLI **18/18** 已耗尽；辅助 **2/6**。历史超支G23前端5、G20 cargo1继续保留。

本次明确新增：
- **cargo 8次**（不重置旧12，G24累计硬帽20）；
- **CLI 24次**（不重置旧18，G24累计硬帽42），单次≤30s、累计本轮≤300s；
- 辅助驱动：旧剩4一次性转入＋新增2＝**本轮上限6**，G24累计辅助上限8；不得把转入4在别处再使用；
- 浏览器/WebGL/工程build/自然生成/ST18/默认集成/bake/realize/世界发布 **0**；
- 本次新增归档≤64MiB，源只读引用，必要受控fixture避免重复复制大型源。

先预算测试/迭代/最终构建/最终CLI，不等用完再想到保护矩阵。超限前partial停止，保留工作，不再靠“验收必须”超支。无需将每个纯读/hash命令计为验证驱动，但矩阵脚本与其中每条CLI分别归类记账。

## 7. 证据绑定一次做完整

续作目录建议：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g24-saved-scene-context/continuation-3/（先确认不存在）。

- 原continuation-2保持原样：它的command-log清单声明1207B，实际1351B；旧清单只有12顶层文件，未逐项绑定runs等111个文件。本次报告引用差异，不倒写旧清单抹掉历史。
- 新清单递归列入本轮实际交付的run-record、stdout、stderr、audit报告、fixture、产品包、源码hash、脚本、report和progress。目录计数不是目录内容哈希。
- 引用旧证据可单独列明确路径/bytes/hash，不能伪装本轮运行；不要求无意义复制全部旧世界。
- 逐run创建合法JSON并立即保存实测值；不要手工拼未转义路径。若记录器有问题，保留原稿并另写修复记录，不覆盖原始观察冒称自始合法。
- 所有日志/报告/progress完成后再manifest→receipt→只读重算；清单生成后不要再append finalised文字到被绑定的command-log。
- 验证清单匹配、未列文件分类和回执hash/bytes绑定，再复核冻结来源/保护源码。源码变更范围与白名单一致。

## 8. 停点

正分支：固定正例通过，格式及语义自洽篡改被指定阶段拒绝，audit不写坏任何输入/预存文件，创建失败不形成成功包，最终完整矩阵与递归绑定闭合 → delivered_for_inspection，待检查员验收。

负分支：准确partial，区分已实作、已测试、未测试和未实现；不另开范围，不自动推进世界消费者。无确认稿，不派子代理，不自批里程碑。

**A冻结、B partial、C不推进。**
