# B01-G21独立复核：结构边求证器取得进展，地面承托接线未完成（2026-09-20）

## 裁定

**接受已复现的结构边求证器、最小楼梯及指定审核进展；G21整体应为partial，不能判为范围完成。**

最主要阻断不是要求新设施或再次更换算法，而是原任务第3节明确要求的地面边结构承托分支仍使用旧采样判断。披露弱路径优于隐瞒，但披露不能代替实现或将原要求改成范围外。若确实受权限阻断，原任务要求停为partial并说明，不是incomplete=[]。

此外，纯求证器无效输入处理与缺口见证需要局部修复；测试及故障注入台账有两处确切口径错误。不要推翻已通过的楼梯几何或事件扫描主干。

## 1. 独立复核与来源绑定

证据目录：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g21-20260920/。

- worker清单90/90 bytes+SHA256匹配；回执30d8ab5e038e84bc23daaffe5b03ab915bdc42681a57a1806df629727c840f8d、14220B匹配。
- core360383B /24ae5cd27afb39bae37171be0367c4697760d73be5b3e3a41b3b3c40fecd9f38。
- 新模块17752B /e7140fdc7280d4eabce4457c8102302ccaf55d2653373f455edf13df7c4c2917。
- CLI源码99c90e46…；access、auto_roads保持此前绑定。
- 清除本次进程测试导出环境变量后独立运行具名模块：**25/25通过**。worker自己的step01-tests.log也明确是25/25，不是26；不推定丢测试，只纠正实际计数。
- 重新构建CLI，独立15次人工调用：窄平台拒；宽平台build/found+verify；原正确来源偏移端点拒；最小直跑/折返各build/found+verify；墙穿拒；自洽几何篡改指定重查拒；payload篡改拒；分别缺adjacentLevelsM/区间requiredBandM指定重查拒；真实sync注入清理；另一次预存文件冲突保护。
- 当前新求证器通过具名横向补宽、仿射包络、变换与上限回归。生产库探针也确认原侧条反例在新结构分支被拒，而非仅测试名称通过。
- 未修改生产源码、测试资源、worker脚本或交付物；不是完整城市集成或所有数值输入的正确性证明。

## 2. P1：ground结构承托仍是原任务未闭合项

原任务：G:/difyllmwiki/GlyphWeave/docs/collaboration/dsh-b01-g21-full-band-coverage-task.zh-CN.md，第3节明确写明地面边中依赖结构的段也不得以弱采样绕过求证器，且允许在该模块中按权威地形/真实层关系分段；若确需修改access等范围外代码，停在partial说明。

实际位置均在白名单模块G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/worldgen/engineering_connections.rs：

- build_ground_edge约3148行调用band_coverage_detail；
- verifier约7228行再次调用同一旧函数；
- band_coverage_detail约3966行保持0.125m逐站采样，按structure_borne_stations聚合；
- 新structural_support_detail已经存在于同一CollisionContext。

目前没有交付“必须改范围外模块才能完成”的具体调用链或权限冲突证明。不能仅声明“超范围”就移除验收项。

### 同一个CollisionContext的生产库对比

检查员从合法宽平台生成的StructureRecord模板构造三个轴对齐支持part，仅用于独立CollisionContext探针，不调用整场景builder验证该修改后的mesh：

- central x=[0,10],z=[-1,0.5]；
- left x=[0,4.005],z=[0.5,1]；
- right x=[4.115,10],z=[0.5,1]；
- 顶面y=1，走线(1,1,0)→(9,1,0)，所需带1.5m。

同一context、同一owner集合、同一走线：

| 消费函数 | 实测 |
|---|---|
| band_coverage_detail（ground当前使用） | 最窄结构支持2m，128/128采样站承托 |
| structural_support_detail（新结构求证） | certified band=0，缺口拒绝 |

缺口处真实对称带只有1m，旧ground支持检查单独仍会放行其支持条件。

**证据层级：生产支持helper/调用链差分，不声称整条ground CLI路线已经通过其它地形/实体/端口门。** 足以证明ground未消费新证书且保留已知弱判据。

## 3. P2：无效矩形仍会返回certified，与indeterminate承诺不符

新模块G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/worldgen/engineering_connections/support_coverage.rs的certify_band只检查段长度、所需带宽有限性，没有校验各frame中心、轴、半尺寸等几何前置条件。无效corners被丢弃后仍可能由s_range留下极大区间。

直接调用生产structural_support_detail：

- 一个MemberFrame的centre.x=NaN，其他保持合法宽平台参数；
- 一个MemberFrame的along/across均为零向量，其他保持同样参数。

两者均返回band=1.5、unsupported=0、三个slice均certified。应为indeterminate/明确拒绝，不是“已证明覆盖”。

这是纯求证器入口契约的实测缺口。正常JSON输入/当前生成器可能已排除此类对象，本审查没有证明该无效对象能穿过完整builder或verifier，因此不夸大为外部可利用通道。但任务明确要求有限性/退化输入安全失败，新模块也作出相同承诺，应在入口闭合。

补齐：a/b及层值、frame中心、轴、半尺寸、轴独立性/尺度约定的验证；不确定几何不得丢弃边界后继续certified。原有事件上限回归保留。

## 4. P2：gap见证当前可能位于已覆盖闭边界

同一侧条反例新求证器返回缺口区间t=0.375625..0.389375，witness=(t=0.3825,s=0.5)。映回世界为x=4.06,z=0.5。

中央面闭支撑范围包含z=0.5，因此用生产MemberFrame.contains(...,0.0)确认该点**已被覆盖**。真正未覆盖的是其外侧至要求半宽0.75之间的区域。inspect_section把covered component的cursor直接写为缺口点，未选在开缺口内部。

拒绝方向正确，但不能称该点为“未覆盖见证”。修为实际未覆盖点或明确的开缺口区间/端点开闭证据，并在测试中独立检查见证确在所需带内且不被任何该有效层支持面覆盖。不要因见证修复改成忽略缺口。

另：EXACT_EPS=1e-12确实用于合并临近区间、放松带边界及近事件去重，不是“只合并完全相同项、永不闭合几何缝隙”。本轮没有以微观epsilon单独证明完整CLI漏检，但证书numerical='exact'及“非有限输入全拒”不能无条件宣称。需说明浮点界，无法安全决定时indeterminate，不要求新增任意精度库或全新设施。

## 5. CLI第14项不是sync故障注入证据

worker的cli-acceptance.ps1先在报告目标写入preExisting，再带--inject-failure final-write-sync运行。该路径在生产发布预检就失败：

    category=output_conflict
    message=...final-write-sync-report.json already exists

因此真实write/sync原语没有执行。台账却硬编码category=publication_failed，并以exit1+无残留+marker未变判“matches-expectation”。这只能证明预存文件保护，不能证明同步故障清理。

检查员独立分两次运行：

1. 所有目标为新路径，注入final-write-sync：实际stderr为“injected failure before the real sync (production primitive)”，bundle与views无残留。
2. 报告目标预存：实际output_conflict，预存内容不变，bundle/views不存在。

两个实现当前均正常，接受检查员这两项观察；worker不能把同一次早退冒称同时覆盖它们。后续台账据实际报告/日志生成，不硬编码预期当观测，不覆盖旧错误台账。

## 6. 本轮接收与不接收范围

接受：

- 新有限事件扫描方向及原指定横向补宽/包络反例的修复；
- 最小直跑、折返、宽平台的独立生产与verify观察；
- 原偏移端点、窄平台、墙穿拒绝；
- 自洽几何篡改与两项缺字段由指定支持重查拒绝；
- 当次实际25项核心测试通过；
- 已独立分开复验的写入sync故障清理和预存文件保护。

不接受：

- ground结构承托被改称范围外后incomplete=[]；
- 无效输入一律indeterminate、缺口点必为真实未覆盖点、数值一律exact等超出实现的总括声明；
- 26测试、一次预存冲突等同实际sync故障注入的台账口径。

## 7. 同包继续建议与预算

不新增设施，不重做已通过楼梯，不更换事件扫描主干。收尾优先级：

1. ground结构承托段正确消费新覆盖事实，并在verifier走同源检查；普通裸地承托不被误改成“必须有结构”。若具体依赖确实越白名单，报告调用链与必须改动后partial停止，不能自批完成。
2. 求证器前置有效性/数值拒绝、真实gap见证。
3. 保留最小楼梯和已接收回归，在同一完整汇报中更正测试数与故障证据来源。

原G21自报cargo5/12、CLI21/24，最多余cargo7/CLI3。检查员调用另列，不冲抵worker，不新增预算。先计划受影响路径与剩余额度；若不够验证最终版本，停为partial，不消耗最后次数后只靠旧CLI宣称新代码已验。

检查员本轮：cargo2（test、CLI build）、rustc1、生产库探针进程1、CLI15；自然/ST18/bake/realize/默认世界0；产品源码/资源修改0。新证据写在检查员目录，旧交付未改。

A冻结、B partial、C不推进。
