# B01-G20独立复核：最小楼梯几何通过，完整支持覆盖仍未闭合（2026-09-20）

## 裁定

**最小直跑/折返生产实例与指定几何篡改复查接受；G20整体保留partial，P0完整支持覆盖不验收。** delivered_for_inspection仅是交检状态，不自动转为正分支验收。

与此前不同，本轮确实完成了可运行的楼梯几何，而不是只修了固定拒绝例。无需推翻已通过生成器改造，不返回G19R3C的A/B讨论，不另增新设施。仍有一个可复现的原支持合同缺口，以及测试/证据口径需要同包修正。

## 1. 独立复核范围与绑定

证据目录：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g20-20260920/。

- G20清单78/78 bytes与SHA256匹配；回执绑定3007a2ccbee29729a10478cbde793e6226effe6339673566e0b4c4af50fd6918 /12408B匹配。
- core333785B /71a8188185ec0d037281ce661fe24ca3d9e93c2b6b9a63e5db5296e4c636d6ec；CLI源码99c90e46…；access、auto_roads保持此前绑定。
- 独立重新构建CLI，消除了worker所述“最后test-only编辑晚于二进制”的执行版本疑义，不修改源码。
- 19项测试中独立直接运行18项，18/18通过。另一项r2c_saved_geometry_tamper_is_caught_by_the_support_recheck会写入冻结worker目录，检查员明确skip，改在独立目录经相同生产API重现其构造、几何变更、重算hash和指定重查断言；该语义通过。不能写成检查员原样运行19/19。
- 独立11次人工CLI：窄平台拒绝；宽平台built/found/verified；真正原偏移端点拒绝；最小直跑built/found/verified；最小折返built/found/verified；墙穿梯段拒绝；保存平台缩窄并重算hash后tamper_detected，payloadHashMatches=true、stored_capabilities_resampled=false；保存支持事实篡改被整包差异拒绝（见§4）。
- interpenetrates、structure_obstacle_blocker、structure_pair_blocker、walking_volume_blocker与G19R3C审查快照函数体比较不变（只归一化行尾）；结合实际正例，不发现通过新增普遍碰撞豁免来获得本轮成功。
- 旧G19R3C清单中的41项包内证据仍逐字节匹配；外部在用源码/测试资源已按新任务修改，不应误报整份旧清单全部仍匹配。

## 2. P1：内部折点被跳过，事件子区间的采样最小值不等于真实最小宽度

定位：G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/worldgen/engineering_connections.rs的structural_support_detail约3595–3697行。

子区间只采严格内部点；steps=2时，(1..steps)给出的中点又被samples.push(mid)重复加入，所谓“至少两样点”实际上可能只有一个不同坐标。最后的端点检查只检查整条polyline的首尾，未覆盖内部折点的入向和出向。

### 生产helper实测

单一矩形平台：centre=(8,1,8)，x向长6m、z向宽1.6m。要求行人带1.5m。全路径同层且在平台内：

    (7.9,1,8.2) → (8.0,1,8.3) → (8.1,1,8.2)

直接链接当前生产rlib调用structural_support_detail，返回：

- narrowest band=**1.5556349186104186m**；
- unsupported endpoints=**0**；
- 两个SupportInterval均carried=true，layer=1.0，parts=[deck::deck]。

但用同一库的station_band_width测第一段末端：

| 位置 | 实际宽度 |
|---|---:|
| 段起点 | 1.6970562748477163m |
| 段中点（现有样点） | 1.5556349186104048m |
| 段90%处 | **1.4424978336205596m** |
| 内部折点 | **1.4142135623730983m** |

后段及折点真实带宽不足，却被支持函数判为满足1.5m。不是仅浮点误差，也不是未来曲面、新设施或阈值追加；当前矩形与有向折线即可复现。

另保留较长缓折线探针，函数1.50449m而折点1.49849m，作为同一漏检的对照；以短折线明显不足的证据为主。

**证据层级：生产helper级，不声称新的不安全完整CLI路线已被生成器产生并verified。** 指定最小楼梯实例实际通过的结果不因此撤回；但“有效层完整通行带支持一般闭合”的命题不成立。

### 必需修复范围

不接受把该缺口转成未经授权的“0.125m分辨率合同”。每段边界、内部折点入/出方向、真实支持边界及净宽变化须纳入判定。对已支持矩形给出保守的区间最小支持或完整带覆盖；不确定区间不能写carried=true。禁止仅缩小步长、重复样点或加特定坐标补丁。

## 3. P2：单元测试直接覆盖冻结交付目录

r2c_saved_geometry_tamper_is_caught_by_the_support_recheck约8371–8379行通过CARGO_MANIFEST_DIR拼接../../work/collaboration/ds-b01-g20-stair-geometry-support/runs，再std::fs::write geometry-tamper-bundle.json。

普通cargo test因此会写历史证据，且同路径没有create_new隔离。即使某次内容相同，也不是只读复验；本检查没有执行这个写盘步骤。

将单元测试保留为内存断言，或使用每次唯一的测试临时目录；需要CLI篡改输入时，由显式证据导出/脚本写入本轮唯一目的地。不能令常规测试依赖或改动collaboration交付目录。

## 4. 支持事实的保存与审核口径需准确

worker声称保存每个SupportInterval并逐能力比对事实，实际生产闸门只保存每能力的narrowestBandM、requiredBandM、unsupportedEndpoints、intervalCount及**uncarried**区间。成功区间的layer/parts/fromT/toT没有进入边的保存事实。

指定独立重查从保存几何重算支持与能力，但对保存事实仅比较可选narrowestBandM；缺失时跳过，没有逐字段比对所有事实。

检查员把15份能力事实的requiredBandM/unsupportedEndpoints/intervalCount改为假值，插入伪造uncarried（999m层、invented::support），重算hash：

- 完整verify=**tamper_detected**、payloadHashMatches=true；
- **stored_capabilities_resampled仍true**；
- 拒绝来自builder重建与保存包逐字段差异（92 findings）。删除supportFacts同样由整包差异拒绝，指定支持重查仍true。

因此**不存在“这项篡改绕过整个verifier”的证据**，不应夸大；但“指定重查逐能力完整对比保存事实”不符合实现。建议在同包明确序列化所承诺的成功/失败区间事实并由指定支持重查核对，缺字段不静默通过。已复现的平台真实几何缩窄专项确实由指定重查抓住，继续认可。

## 5. 两项来源/验收口径更正

### 5.1 原偏移端点不是terminal-own-stair-solid

G20 CLI台账将terminal-own-stair-solid命名为“original offset endpoint”，两者不是同一个反例。原例在：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g19r3c-final-20260920/endpoint-near-edge.json，是6×1.6m单平台，终点(10.96,1,8.5)。

检查员本次已独立运行，当前版正确拒绝。接受代码结果，但worker的CLI要求映射不得混用case名字。不要覆盖旧台账；在下一份完整结果中准确说明补验来源。

### 5.2 resource-hashes-before不是上一包验收版本

五个G20“before”hash均不同于G19R3C最终清单绑定的五个资源。例如positive-switchback：旧冻结e711ab1e…/208241B，G20 before为88672576…/207175B。

不推测差异来自何时/何人或是否脚本重跑；旧清单证据本身未变。需把G19R3C冻结起点、G20中间before、G20最终版本区分，不把中间hash声称为上一包原版快照。可引用既有记录如实声明归档缺口，不为补档重跑或倒写旧证据。

## 6. 下一步建议：只收尾G20，不推翻已通过几何

沿原G20范围修复§2和§3，并收实§4的保存/审核事实；保留最小直跑、折返、碰撞硬拒绝与指定几何篡改正负对照。不要重设计已通过的楼梯，不扩设施或真实世界消费者，不增加新的合同门。

worker已报cargo6/8、CLI12/20，最多余cargo2与CLI8；余额已含此前结转，不再把旧cargo1/CLI10加第二次。漏计扣减。能在原额度闭合则一次交付，否则准确partial停点，不默增预算。无需单独确认稿/勘误包。

检查员消耗单列：cargo2、rustc4、生产库探针进程4、CLI11，人工生产构造/审核均为独立小型输入；自然/ST18/bake/realize/默认集成0，产品源码/资源修改0。第一份facts探针尝试读取不存在的intervals字段，altered=0，明确不作为篡改证据；后续修正探针实际更改15份事实，结果如§4，原日志全部保留。

A冻结、B partial、C不推进。
