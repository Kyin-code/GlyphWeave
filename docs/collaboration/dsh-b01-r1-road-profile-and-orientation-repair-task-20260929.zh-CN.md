# DSH执行令：B01-R1 道路profile供给与非对称退界修复（窄修复）

日期：2026-09-29。对应原始要求：B01真实地块/建筑/入口闭环、B02道路供给前置接口；不进入B02整体、不进入C。生产ID/任务号：B01-R1。

## 0. 严格执行与恢复

**请严格按任务书完成任务，不得因为其他任务无关问题擅自中断、停止或跳过任何步骤。上下文压缩、会话切换、摘要丢失、其他任务状态或可修复的驱动问题都不是删除必测项的理由。**
如需中断，先写checkpoint，包含当前源码/输入/exe hash、T项完成与剩余、实际账目、最后错误、下一条具体命令；恢复后继续。不允许重复同因失败两次而无新诊断；不得把checkpoint当交付、不得伪造未观察项。

## 1. 范围和决定

仓库`G:/difyllmwiki/GlyphWeave`，新证据目录`bevy/work/collaboration/ds-b01-g34-r1-b01-profile-repair-20260929/`，create-new；历史B主线目录和G34资料只读。
不修改F盘、不Git写、不提交/推送/切换/回退、不修改A冻结世界或历史manifest，不启动子代理。允许修改仅：
- `bevy/crates/core/src/worldgen/frontage.rs`；
- `bevy/crates/core/src/worldgen/road_network.rs`、`road_extensions.rs`、`auto_roads.rs`中必要的生产profile接线；
- 必要的`development.rs`/`worldgen.rs`机械接线；
- 定向core/CLI测试与一个新B01 manifest/审计脚本。

**禁止修改land-use zoning语义、`landuse_ledger.rs` reserved规则、自然/保护区判据、用户需求数量/地块尺寸、reachDistance、urban ratios、zoning cell大小、G32-G34 viewer/渲染材质。**

决策基础：夜间b8明确，frontage只在profile road上取站位；当前b8有20条profile road但frontage只在1条road-auto上实现，base generated.road-network roads无profile。此任务修供给链，不把分区跟随道路当默认修法。

## 2. T0 写集和基线闸门

修改前保存每个待改文件原始bytes/SHA256，`write-set.json`必须唯一列每个文件；修正夜间v2重复的`farm_program.rs`和漏列`worldgen.rs`，把说明性round注释与语义代码分栏。原hash缺失必须继续披露，不能补造。记录当前HEAD/dirty清单但不覆盖其它未归属改动。
冻结输入：`examples/b01-1km-probe.manifest.json`与对应source hashes；保留control `runs/b8-current-a01`只读；绑定当前debug/rebuilt exe。离线闸门未过不得启动最后一次1km世界。

## 3. T1 修复非对称ParcelInsets方向回归

当前`buildable_core(b,insets)`只在世界X/Z轴减front/rear，实际`ParcelInsets.frontM`沿建筑front方向。使用非对称`front=4, side=2, rear=12`，对yaw 0/90/180/270建立手推期望，并分别测试：
- boundary方向旋转正确；
- buildable core沿local forward/backward与左右side缩进；
- envelope purpose coverage、entrance corridor、yard/lane不因旋转交换前后；
- 既有对称legacy `[2,2,2]`四方向回归；
- 负例：错误前后退界、core越界、非四向旋转、邻地/保护/water仍拒绝。
测试必须调用生产函数或其明确纯函数，不是只测试手抄expected。不得把75%规则的已知业务变化当成修复；coverage policy不变。

## 4. T2 生产base road profile供给链

先读清`RoadNetworkPlan::entities()`、`road_extensions::profile`、`auto_roads`和frontage `profile_roads/stations`的生产路径。设计要求：

- base `generated.road-network.*` edge若要承载frontage，必须生成可验证的`RoadProfile`，其start/end/length/width/elevations与最终road entity/engineering terrain完全一致；不能仅贴label或把profile写到不消费的plan字段。
- profile可从真实natural/engineered heightfield按道路方向采样；`validate_shape/validate_entity`必须通过；相邻样本斜率、max fill、保护/水/道路支撑合同不放宽。若某base edge不能合法profile，则保留拒绝并说明，不强行生成。
- profile进入`RoadNetworkPlan::extensions`或另一个已被`entities()`与frontage实际读取的权威字段，并持久化到scene/road plan；生产frontage `stations()`必须看到它。
- 不改变zone分类、urban/rural/nature比例和reserved ledger；不删除/移动authored或fixed roads。
- 先给一个非空最小fixture：一条普通base road、有效mm高程、至少一个station，生产candidate site成功；同一条无效profile应拒绝。

禁止把所有道路无条件刷成平面；禁止让profile地面与engineered final不一致；禁止改成“frontage忽略profile”，因为profile是道路高程/支撑合同的一部分。

## 5. T3 离线和回归闸门

先运行并留日志：parcel/geometry/frontage/frontage_supply/road_profile/road_network/auto_roads/services/development/landcover/landuse ledger相关定向测试；新增测试必须非空正例+负例。校验四朝向和base profile source path。
静态/fixture需证明：
- 不改zone/reserved语义；
- profile数、profile来源、entity geometry相等；
- 旧legacy all-8m与64m development guard不退化；
- `attempted/realized/rejected/unfilled`和面积账守恒；
- `quality-report` JSON status不是只看进程exit；
- old control read-only。

若离线测试发现`auto_roads`已有失败，保留“当前候选失败”，不得 checkout旧树伪造基线；用夜间备份分支/working-tree副本做静态对照，单独标历史unknown。

## 6. T4 最后一次1km世界运行（有条件授权）

只有T1-T3全绿才使用剩余一次`<=1km`成功世界额度。输入固定`examples/b01-1km-probe.manifest.json`，不得换seed、减需求或扩大reachDistance；输出create-new，绑定当前exe/source/input。
启动前确认：当前1km成功世界计数5/6；512m硬额度6/6，不得新跑；6x10km不得跑。若资源账冲突立即保存checkpoint，不暗超。

成功要求不是“exit0”而是业务差异：
- base road-network edges中合法profile道路数量真实增加，并且profile来源可追溯；
- frontage-carrying road IDs不再只有1条road-auto（至少出现base road或明确说明候选位置不满足高程/保护约束）；
- homes/shops frontage实现相对control有实质增长，不能只改变rejection计数；
- building/development/landcover/ledger conservation保持；
- `scale-audit.status=pass`，`quality-report.status`真实读取；若status=fail仍交partial；
- 512旧control不被覆盖，512新版本不宣称已观察；
- 若候选profile增加但frontage仍无增长，停止继续世界尝试，保留原因，不再跑zone-follow。

## 7. 账务、交付和停止

每个cargo/CLI/driver真实进程入账，写前路径守卫，world run前intent；失败保留。manifest/receipt不是本轮必要产品闭集，但必须有`report.md`、`write-set.json`、`checkpoint.json`、`business-delta.json`、control/candidate bindings和源hash。

额度只允许最后一次1km world；不释放512硬上限，不启动最大图。不准把b8 plan 22/realized21混写成21个完整建筑；报告必须分开。

交付状态：
- 若T1-T4均成立：`ready_for_review`，B01-R1局部通过但B01整体仍需主Agent检查；
- 若T1-T3成立、T4未跑/失败：`partial`，不自宣B01通过；
- 若发现真实profile/road工程缺陷且无法修复：准确`blocked`，保留下一条命令。

上轮已做不佳清单必须逐条回应：
1. write-set重复/漏列；
2. 75%覆盖语义改变但未先独立证明非对称方向；
3. 把生产profile链概括成“只有测试夹具”；
4. 1km b8同时存在22个plan buildings/21个realized，摘要混写；
5. quality-report exit0但JSON status=fail，不得只看exit；
6. 旧1km失败目录的规则/landmark/asset原因与真正产品缺口要分栏；
7. 当前10世界经验中“后续改动无效”与早期512提升历史不混称。

**请严格按任务书完成任务，不得因为其他任务无关问题擅自中断或停止部分步骤；上下文压缩时保存checkpoint并恢复继续。**