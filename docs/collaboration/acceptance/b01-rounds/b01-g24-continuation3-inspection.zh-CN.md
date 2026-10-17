# B01-G24 continuation-3 独立检查（2026-09-22）

## 1. 裁定

**G24继续partial：已接受完整性、只读报告和递归绑定的实际进展，但不是只余观察性补测。checks/limitations内部语义及manifest来源/身份的一致性仍有生产假通过。** 不推翻已修好的BoundScene、局部地形/实体/道路核心，不重做G22/G23。

最终代码：CLI engineering_context.rs 59331 B / 67ff41dfb1123d93838443dee3cde561b376641b29645c2bf42084003fd34d22；core 104911 B / 69fbd3a61334ace886250330be7d7d458e1d844e59a7d5c1a8a4364259844d69。lib a031f15c…/516 B、main 07a054d6…/134612 B均未变。

## 2. 独立验证与接受范围

检查员当前源码实跑：
- cargo test -p glyphweave-cli --offline --bin glyphweave：**21/21**。其中14项engineering_context测试，另7项既有CLI测试；不是21项都属于本次上下文单测。
- cargo test -p glyphweave-core --offline --lib engineering_ -- --test-threads=4：**82/82**、545 filtered out，不是全库。
- cargo build -p glyphweave-cli --offline：通过；随后17次独立CLI（1导出+16审核/路径拒绝）。

本版生产CLI接受/补证：
1. 固定256,32,64,64新鲜导出，四段audit通过；整份JSON重新编码的8.0→8仍通过。
2. final样本篡改仅terrainParity fail；删实体/改roadId仅entityAndRoadParity fail。三者均为本版自洽hash包，补上worker m10/m11/m12最终版观察。
3. 缺marker时仅completeness fail，其余三段pass；缺natural来源时sourceBinding fail，terrain/entity/completeness均notEvaluated。
4. 全新report路径位于待审包根内或检查员自建source根内均明确拒绝，输出不存在，不再由“预存即拒”遮蔽。worker m19的旧措辞缺口因此有独立补证，不能仍当本版唯一阻断。
5. 未知context版本被completeness拒绝。注意源码仍可重建并给其他阶段当前格式下的比较结果；不得把其他阶段pass解释为支持未知格式，总结果拒绝成立。
6. 原子create_dir、create_new、写后sync、登记文件清理与非递归目录移除已有实现，确定性单测通过。不声称本次动态注入了实际sync I/O错误或任意并发替换攻击。

## 3. 阻断一：字段有归属，但内部事实仍可伪造（P1）

位置：G:/difyllmwiki/GlyphWeave/bevy/crates/cli/src/engineering_context.rs，validate_context_semantics，388–435行。

### 3.1 非正式检查项可变为通过

代码只找STAGES四个正式审核项，未校验其余checks。

独立04号反例：仅把accessRouteReverification和engineeringFeasibility改成pass，并改detail为已验证路由/工程可行，同步context、manifest、marker哈希。

**结果：exit0、totalPass=true、四段pass。** 原任务明确未重新求路、未生成EngineeringInput，这不是允许的观察性差异。

### 3.2 重复审核项可藏第二个假pass

独立05号反例：保持第一条sourceBinding=notEvaluated，追加第二条同名sourceBinding=pass。

**结果：仍四段pass。** 使用find只核验第一条，未检查名称唯一、完整集合、值和类型。

### 3.3 limitations只验证关键词，不验证事实

独立06号反例：把全部limitations替换为单句“Unrelated records are not verified; this engineering context is safe to build.”，同步hash。

**结果：仍四段pass。** 仅contains("are not verified")不证明源漂移的限制被真实保留，也不能保住未建模水/保护/碰撞、不重新验证通行等约束。

修复范围仍为原字段覆盖任务：checks和limitations需与独立重建的确定事实比对，或建立同等强度、完整的结构化语义规则。检查每个check的唯一名称、状态、detail/其他声明字段、完整集合及未知项；limitations不能靠非空和关键词过门。保留可接受的表示规范化，但不能让源事实被文本替换成相反结论。

要求新增上述三个独立反例，以及缺项、重复、伪造状态/内容的参数化测试；测试用真实生产context作为正对照，不只用缩水的合成包证明validator满足自身较弱假设。

## 4. 阻断二：manifest和重建事实仍未逐项核对（P1）

位置：同CLI文件validate_manifest 449–532行、audit_context 995–1027行。

### 4.1 相同条目数的全假来源通过

独立07号：manifest.sourceBindings所有条目的path改为invented/N、role改invented-role、bytes=1、sha256全0，保留原条目数，同步marker.manifestSha256。

**结果：exit0、totalPass=true、四段pass。** context内部sourceBindings正确并不替manifest独立声明作证；当前只做类型与长度检查。

### 4.2 少条目已被发现，却仍pass

独立08号：manifest.sourceBindings从4条缩成1条并重封marker。

**结果仍四段pass；completeness.detail甚至同时写：the package declares inconsistent sourceBindings。**

根因：代码将失败与成功都写入detail字符串，随后靠contains("absent") || contains("requires") || contains("could not")推断失败；“inconsistent”不在名单，所以已知错误被读成成功。

这必须改成结构化布尔/枚举/Result聚合，诊断文字只呈现结果，不参与控制流。不接受只把inconsistent追加关键词的修法。

### 4.3 manifest身份和窗口被忽略

独立09号：manifest.sceneId=invented-scene、window=[0,0,1,1]，context仍是原scene-0及256,32,64,64，同步marker哈希。

**结果仍四段pass。** 生产者输出的身份与选窗声明没有审核归属。

修复：manifest及marker采用完整的受版本约束规则；对sourceBindings每条path/role/bytes/hash和集合唯一性、sceneId、window与独立来源及context进行一致性校验。固定引用/文件hash检查保留。明确每个生产字段、缺字段、未知字段和非法类型的策略，不能只在context根字段列16+3张表。

## 5. 原任务仍有的读取边界缺项（P2，静态发现）

read_package_file（230–239行）仍直接fs::read：未实施包文件大小/累计大小帽，未在读取前拒绝symlink/reparse或核对解析后的根内路径。validate_contract使用read_dir(...).flatten()，枚举条目错误会被丢弃而非拒绝。来源BoundScene的保护并不自动覆盖待审包读取。

检查员尝试在自有目录创建文件符号链接，系统返回EPERM，因此**没有动态证明越根链接被接受**，没有为此提升权限，未跑该次CLI。该观察边界保留在cli-review.json。静态缺项来自已授权任务第1节的“包根读取同样应有根内/链接约束及大小帽”，不属于新加范围。

后续沿现有受限读取方式补齐，不以大文件实测压垮机器，也不要求全新通用文件框架；边界错误必须结构化失败，不静默跳过。

## 6. 递归交付与历史口径

本轮 **164/164条bytes/hash匹配、0未列文件**（manifest/receipt按声明排除），manifest24669B / 4b5dfc3c4b8ec26766512adcd177407b464393246ff6bbe51369b220f1ba17de；receipt对manifest/report/progress/commandLog/sourceBinding/readOnlyVerification六项绑定全部匹配。此项可以接受，不要求重做历史清单。

continuation-2全部已记录124文件与检查员此前状态相同，原失配没有被倒写；continuation-1清单未变。固定四源、十项保护代码/资产匹配，正确support_coverage路径也已核验。旧G23R1/G23/G22的87/339/94/54条全匹配。

两点非阻断口径：
- CLI21测试实际是14个context+7个既有测试。
- 本轮7次cargo总数与累计19口径相符，但非零失败调用是两次：cargo-13三项测试失败、cargo-16 E0308。若“含1次E0308失败”专指编译失败成立，progress.failuresCounted=1不能读成全部失败调用只有一次。不额外扣一次额度，因为两次已经在7次内。

## 7. 停点与下一步方向（本次不追加授权）

接受已修核心、真实阶段传播、数值等价、只读报告和递归绑定；不接受“四项限定全部闭合”。后续应只收口：
1. checks/limitations完整可信语义；
2. manifest各声明与独立事实逐项一致；结构化失败聚合，绝不按诊断字符串判断；
3. 待审包受限读取边界及原合同负例。

不重做已补证m10/m11/m12或m19的旧版本观察，不改固定来源、不扩到默认世界。修改后的最终版本仍须按受影响路径测试。

worker当前账本cargo19/20、CLI42/42、辅助5/8按自报保留；剩cargo1、CLI0，不足以自行启动完整修复+最终CLI。**本检查不自动追加额度、不要求用最后一次cargo硬冲、不启动新的执行包。** 下一授权应按上述有界收口和实际调用另行明确。

检查员本轮成本：cargo3（CLI21测试、core82保护回归、CLI构建）、生产CLI17、矩阵驱动1；符号链接创建尝试1失败（无CLI、未提权），纯读取/hash核验另列。浏览器/工程build/自然生成/ST18/bake/realize/发布0。产品源码、worker证据、固定来源均未修改。

## 8. 证据位置

G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g24-continuation3-20260922/

关键：bindings-before/after.json、cli-tests.log、core-protection.log、cli-build.log、cli-review.json、04–09号audit报告、runs/、检查员新鲜包与自洽负例。全部后续清单绑定，不修改worker包。

**G24 partial，G22/G23限域接受不变；A冻结、B partial、C不推进。**
