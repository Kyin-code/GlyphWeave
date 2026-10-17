# B 主线夜间收口独立检查：B01 partial，拒绝分区语义试验，批准窄修复候选

日期：2026-09-29。裁定：B01 = `accepted_subtask_partial`；B整体仍partial，B02-B05不启动，C不推进。本报告不修改产品或历史证据，不运行cargo/产品CLI/新世界生成；只读取夜间产物，并在独立目录运行只读反例与复制世界审计。

## 1. 夜间交付可以接受的部分

- `B-STATUS-OVERVIEW.zh-CN.md`、`reviewer-checklist.zh-CN.md`、`requirements-progress.json`、`checkpoint.json`相互一致，能够恢复夜间工作。
- `b8-current-a01`确实是当前源码绑定的1km生成世界：run记录runtime hash与source hashes，exe当前hash一致；不是把旧世界伪装成当前结果。
- 独立重读process ledger：20条CLI进程，6个512m成功世界、5个1km成功世界；与报告“512硬上限用尽、1km尚有1次成功世界额度”一致。失败尝试另有记录，未把快速拒绝误算成功世界。
- 十一个已有世界的frontage/land ledger conservation关系独立复算通过：`attempted = realized + rejected`、`unfilled = requested - realized`、土地`realized + unresolved = scene area`。
- 当前b8的真实量应精确写成：development plan有22个building记录，但其中21个realized；roadside sites 22、frontage sites 1、46条road edges、20条带profile的保存道路；urban-homes 0/120、urban-shops 1/24；土地实现334257/1000000m²，status=incomplete。
- 512m权威测量8/16住宅、0/8店铺及此前5→8的局部提升仍可保留；但不能说整个38轮生成量从未增长。准确表述是：B1早期阈值/共享改动曾使512住宅从5提高到8，后续当前1km修法和round16-32的候选没有带来当前1km材料增量。

## 2. 必须纠正的报告口径

### 2.1 “生产代码只有测试夹具写road_profile”过强

当前b8实际有20条带profile的保存道路；`RoadNetworkPlan::entities()`从`plan.extensions`通过`road_extensions::profile`派生profile，且`generated.road-auto.*`承担了profile道路。准确根因不是“生产完全不写profile”，而是：**base `generated.road-network.*` edges通常没有extension/profile，`frontage::stations()`对没有profile的道路直接返回None；因此大量合法基础道路永远不给frontage。**

这是结构性事实，但要把“哪些代码直接构造profile、哪些从extension派生”分开，不拿测试fixture断言替代生产路径。

### 2.2 b8不是完整质量通过

我把b8世界复制到独立目录，运行当前CLI的`scale-audit`与`quality-report`，原始世界零漂移。结果：
- `scale-audit` JSON `status=fail`，具体为`generated.road-auto.scene-0.5`的longitudinal contract无效；
- `quality-report` JSON `status=fail`，包含road plan/raster差异、housing plan entity anchor mismatch、生态1个缺额、663135m² landcover未实现、剩余土地无计划、urban/rural unresolved等；CLI进程exit 0不能把JSON status=fail改成通过。
这不是本轮另造产品缺陷结论，而是当前b8不能签B的明确边界，后续候选必须重新处理或如实携带。

### 2.3 账务和write-set问题仍要修正

`write-set.json` v2重复列`farm_program.rs`两次、漏列当前实际变化的`worldgen.rs`；`frontage.rs` pre-edit hash未记录。这是证据质量问题，不否定夜间业务结果，但下一轮必须在修改前重新生成唯一写集并解释已有脏差异。current source相对夜间保存点的7个真实变化中，worldgen只增加了被否证实验的注释/空白，不应继续称为“未变化”；应明确区分语义代码和说明性变化。

## 3. 我不同意的三项请求与决定

### 请求1：最后一次1km跑“分区跟随道路”

**暂不批准该候选。**它改变land-use zoning语义，且上一版全量跟随道路在reserved ledger上真实被拒。当前证据更直接地指向profile供给断链，先修这个窄问题，不先改分区定义。最后一次1km额度可以批准给“基础road-network edge获得合法local profile并进入frontage候选”的候选，前提是离线闸门全绿、修复不改变zoning语义。

### 请求2：释放512m额度

**不释放、不暗超。**任务硬上限已用尽；已有512世界可作历史/回归基线，但新源码候选的512端到端结果必须标`not_observed`，不能用旧世界冒充新代码通过。若要重跑512，需要用户另行批准新的资源合同，不靠解释“释放余额”绕过硬上限。

### 请求3：接受B01=partial+完整证据

**接受，但名称必须是`accepted_subtask_partial`，不是B01 complete，更不是B通过。**证据包完整到足以决定下一步，真实业务缺口仍在：城市frontage需求未闭合、60~75%面积未解决、b8质量审计失败、B02-B05未开始。

## 4. 下一步任务已发布：B01-R1 窄修复，不做分区改语义

任务文件：`G:/difyllmwiki/GlyphWeave/docs/collaboration/dsh-b01-r1-road-profile-and-orientation-repair-task-20260929.zh-CN.md`。

工作内容：
1. 修正非对称`ParcelInsets`在0/90/180/270下的buildable-core计算；保留前/侧/后语义，不接受把75%覆盖规则继续当成默认真理而掩盖朝向bug。
2. 为生产base road-network edge研究并实现合法profile接线；必须使用真实保存/工程高程，不能伪造flat profile或绕过`RoadProfile::validate_entity`。不改变zoning语义、不碰nature/reserved区。
3. 离线测试、非空fixture和所有frontage/road/services/ledger回归先过；再使用最后一次1km世界额度跑同一B01 probe，比较profile道路数、frontage站点、homes/shops、buildings、账本、scale/quality audit。
4. 新候选若没有frontage实质增长，停止世界级尝试并交原因；不再试urban ratio、扩attempt budget、缩zoning cell、zone-follow等已排除/越界手段。
5. 512m新结果不作假：旧512世界只作基线，新增源码在512尺度的整世界验证标notObserved，直到有新资源授权。

## 5. 结论

夜间执行者没有懈怠：它找到了真实供给链根因、保留失败和额度账，也主动撤回错误归因。需要纠正的是“production only test fixture profile”的过强表述、b8 plan/realized数量混淆、quality-report exit0与JSON fail的口径、write-set重复/漏列。
本轮结论：**B01 partial可接受；禁止最后一次额度用于未经批准的分区语义改变；批准下一轮只做profile供给+朝向核心修复，之后再决定B02。**
## 6. 交付记录中的新增矛盾

夜间 `review-receipt.json` 写 `productSourceModified=false`，但与执行前已保存的 raw working-tree checkpoint `G:/difyllmwiki/GlyphWeave-backups/pre-b-mainline-20260929-004242/working-tree/`逐字节比较，当前实际有7个文件变化：`frontage.rs`、`frontage_supply.rs`、`auto_roads.rs`、`landuse_ledger.rs`、`farm_program.rs`、`road_network.rs`、`worldgen.rs`。其中至少前六项是B轮修改/测试接线；`worldgen.rs`当前相对备份主要是被否证实验的注释/空白，但也不能写成字节未变。`write-set.json`重复列`farm_program.rs`、漏列`worldgen.rs`，而且`frontage.rs`的pre-edit hash缺失。故本轮不接受“无产品源码变化”的说法；接受的准确说法是“B轮源码变更已绑定在write-set中，但write-set需下一轮更正后才可作完整交付绑定”。

这不改变当前B01 partial裁定，但属于必须修正的执行记录问题。