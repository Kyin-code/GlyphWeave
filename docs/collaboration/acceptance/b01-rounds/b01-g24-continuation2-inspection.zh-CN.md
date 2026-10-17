# B01-G24 continuation-2 独立检查（2026-09-21）

## 1. 裁定

**维持partial，但下一轮不推翻已修好的核心；阻断集中在新CLI审核的包合同、字段覆盖、只读I/O与证据绑定。** 不是只补两条CLI即可验收。

旧缺口对应BoundScene私有绑定、完整实体/接入保存、保守bounds并集与闭边界、道路关系、窗口相关块筛选、权威HeightPayload复用已有生产实现及具名反例。本次检查员以当前源码跑engineering_过滤 **82 passed / 0 failed / 545 filtered out**（36 context + 原46），补了worker未跑的保护矩阵。它不等于任意输入安全性证明。

基线：core engineering_context.rs 104911B / 69fbd3a61334ace886250330be7d7d458e1d844e59a7d5c1a8a4364259844d69；CLI engineering_context.rs 19603B / eb18860b3f658d9538228750335e8209f604a8bcd4a440edb2416dcbf285cbb2；lib.rs a031f15c…/516B；main.rs 07a054d6…/134612B。不要再把lib注册称为未发生的既有变更；它是上轮已有、本轮保持。

## 2. 独立生产观察

检查员当前源码构建CLI后实跑 **18次**（不是占用worker18次）：
- 固定256,32,64,64新目录导出、audit四段通过。
- 缺完成标记：当前最终版仅contextCompleteness失败，其余三段通过。
- worker缺natural fixture仍有BOM；原样运行拒于JSON解析，不能算缺natural验收。检查员新目录副本仅去BOM后，出口明确报natural height payload is missing: chunk(0,0) names no natural height payload，无成功产物。
- 自洽自然样本篡改，保留其他数值字面量后，仅terrainParity失败。
- 来源无法重建：sourceBinding fail、terrain/entity notEvaluated，但contextCompleteness pass，复现worker自报阶段缺陷。

因此两个遗漏用例已由检查员为当前版本补证，不要求worker为修复前版本再补历史运行；下一修改版本仍须按最终受影响矩阵验证。

## 3. 新CLI阻断

### P1-A 格式完整性没有审核实际声明（CLI 274–324、445–488）

manifest读取后只检查JSON可解析，marker只检查status和contextSha256；marker.manifestSha256、各schemaVersion、contextFile/manifestFile引用、manifest的sourceBindings/contextSha256/packageFiles等没有被验证。

**独立反例**：
1. manifest换成空对象{}，marker.manifestSha256同步为该空对象哈希 → **exit0、totalPass=true、四段pass**。
2. 只将marker.manifestSha256改成64个0 → **仍四段pass**。
3. 添加未声明文件undeclared.txt → **仍四段pass**。

这不是来源绑定正确就可豁免的声明问题。格式本身的必需集合必须独立要求，严格检查类型、唯一性、版本、固定引用及hash/bytes关系；不能再用“待审清单还剩哪些就检查哪些”。包根三必需文件、来源外部文件集合和报告输出应明确分工。不能将外部来源文件缺失错误地冒充包内文件删除负例。

### P1-B 未比较字段可伪造状态或版本（CLI 401–435）

compare_stage仅覆盖少数根字段；schemaVersion/readOnly/checks/limitations/accessScope/entityKindCounts/unmodelledKinds和未知额外根键没有审核归属。

**保持其他数值文本不变的独立反例**：
- schemaVersion从engineering-context-v2改成v999，同步context/manifest/marker哈希 → **totalPass=true**。
- readOnly改false，原notEvaluated检查全部改pass，删除limitations，新增safetyVerified=true，同步哈希 → **totalPass=true**。

说明包能删除“兼容子树未验证”的限制、把未重新求路宣称通过而仍获总通过。下一轮必须建立所有输出字段→指定审核阶段的覆盖表，缺字段/错误类型/额外未定义字段均拒绝；特别是未知版本不应先按已知schema做部分比较。

### P1-C audit实际可覆盖已有文件和待审输入（CLI finish 525–530）

finish使用fs::write，没有create_new、源根/包根排除或报告别名保护。

**仅对检查员新副本实测**：
- --report指向已存在的DO NOT OVERWRITE文件 → 覆盖为报告，exit0。
- --report指向被审副本自身engineering-context.json → **totalPass=true后将上下文覆盖为audit报告**。

所有破坏性观察仅发生于检查员自建路径，未触碰worker证据或固定世界。真实源根同类风险是源码推导，未对真实source做覆盖实验。

导出亦有静态所有权缺口：out.exists预检后create_dir_all，不是原子取得目录所有权；payload使用fs::write，失败remove_dir_all整目录。不能据此声称“只清理本次创建对象”。本次未制造并发竞争，下一轮应做可控冲突/故障原语测试而不是依靠时间竞态。

### P2-D 阶段未评估、重复读取和数值表示

- BoundScene::read在同一次audit读取两遍，第二次失败被if let Ok静默跳过，从而来源不可重建仍completeness pass。应共享一次独立重建结果，无法完成的检查notEvaluated；已知缺标记等明确失败不应被notEvaluated掩盖。
- 原始context被常规JSON parse/stringify后8.0变8，当前Value比较报entities[0].depthM differs: source 8.0 vs package 8。这是表示差异，不是几何不同。首次检查员通用篡改驱动也因此触发无关parity错误，不能用那次失败证明版本/状态篡改已被抓住。
- 已用保留原数字字面量的负例隔离，A/B/C上述关键观察均有效。下一轮对类型化语义比较或精确定义的数值规范化收口，严禁用增加epsilon放宽安全或几何差异；整数毫米及ID不做有损转换。
- window i64直接as i32、expected_window不符可能追加两个sourceBinding阶段，也应在此状态机内按原有有界输入合同修正为checked conversion和每阶段恰一条。此两项为静态发现，不冒充动态复现。

## 4. 证据绑定问题

固定四源、十项保护源码/资产匹配；旧G24停点与continuation-1保持；G23R1/G23/G22清单87/339/94/54均匹配。

worker本轮manifest列12文件：其中 **command-log.txt失配**，声明1207B/eaddfe61…，实际1351B/442f553e…。末尾新增finalised记录在清单hash之后，故不能声称定稿后只读核验闭合。

本轮目录实际124个文件，12列项+manifest之外 **111个文件未列入绑定**，包括run-record/stdout/stderr/audit报告及实际包与fixture。manifest的runDirectories=18只是计数，不是文件hash绑定。18个run-record均能解析，流hash与记录匹配，这接受；但它们没有被交付清单递归锁定。不要重写旧清单伪装当时通过；新收尾包引用旧清单声明与本次观察差异并重新绑定必要引用证据。

## 5. 运行来源与成本

检查员：cargo2（82项保护回归+最终CLI构建）、生产CLI18、浏览器0、工程build/生成/自然/ST18/bake/realize/发布0；独立矩阵驱动3次（首通用驱动、一次literal驱动在任何CLI前因错误v1搜索串中止、修正为实际v2后的驱动）。失败驱动保留，首次数值规范化带来的无关失败也保留，不改预期冒充通过。

检查员后五次含缺natural副本去BOM；原workerfixture未改。所有探针输出都位于独立目录。worker额度仍cargo12/12、CLI18/18、辅助2/6，历史成本不受本次补证改变。

## 6. 下一任务

用户要求下一步任务，另发 **G24同包审核与只读发布收尾任务**，限定上述A–D和最终绑定，不重做已通过核心、不改G22/G23、不接世界。原额度已耗尽，新增授权明确列账，不靠旧余额续跑。

证据根：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g24-continuation2-20260921/。
关键文件：bindings-before/after.json、core-protection.log、cli-build.log、cli-probes.json、literal-probes.json、各*.audit.json与runs/、检查员生成负例。

**A冻结、B partial、C不推进；G24仍partial，接受旧指定核心修复，不接受当前包审核/只读发布已闭合。**
