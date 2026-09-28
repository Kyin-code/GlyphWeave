# V4.1 B01-G8：station18首个真实增量的单次受控实验

## 1. 批准内容与目标

G7离线提案已accepted-proposal-for-bounded-experiment。先读b01-g7-offline-next-increment-inspection.zh-CN.md。本轮批准单次自然harness，不再修改台账；仅检验B=3110能否使station18北/南窗越过no-cut，并获得至少1个经过完整实现审核的新地块，或定位新的首拒绝。

原16住宅+8店铺需求、请求顺序/尺寸/insets/setback/搜索预算保持。目标1个真实增量是小步验收门槛，不是将总需求降成1或2。

## 2. 唯一固定几何与两态规则

A=(288,96)@8000mm；B=(288,180)@**3110mm**；C坐标=(176,180)。AB84m/BC112m，两段12m local/urban、shoulder3、junctionLength9、fill6000、rise80；junctionRadius仍9，旧35路27节点21义务/profile一律不动。

预注册至多两态：
- ST18-R1：新B=3110下生产infer_end派生C高程，记录returnedMinimum和chosenC。
- ST18-R2：C=B=3110。同值去重，不增加第三态。

不跑B3000旧对照，直接引用G6-R1。不得因拒绝临时改B为3111/3200、改C坐标/宽度/平台，不能加安全余量绕过恰好相等边界。H1+696、B+1002仍未批准。

预计station18起点传播给2390mm是条件推算，必须记录新production profile实值，不拿预计替代实测。AB新高程的完整自然包络及两段196格接点高度一致性仍走现生产append检查。

## 3. 先完成离线/合成发布自检，再启动一次自然入口

可以在auto_roads.rs现cfg(test)新增专用G8入口，复用已有生产叶子、candidate绑定、完整trace/profile输出及deadline封装；禁止修改生产frontage/solver/carve/审核/默认或开放内部类型。

自然前必须用已有G6-R1 JSON/合成fixture检查：
- 85/113点数来自请求长度+1，不硬写同一个长度；
- 普通geometry类型/无Ok或Err包装/单位整数mm；
- sample/实体/原始detail关联，空结果和失败路径也能输出；
- 全部必需字段、raw trace、profile、归属和预算汇总一次序列化成功；
- 只有realized有增量、commit_decision通过且未截止时才可输出best；延迟返回/未接受/提前终止均能如实输出。

新证据目录先写规则表和输入/源码hash，日志直接完整落盘。用create-new运行预约/启动标记防止无意重启；自然入口计数是整轮累计，不因进程/目录变化重置。测试先用精确名字，不宽过滤触发旧ignored自然用例。

## 4. 一次运行内的阶段顺序

1. 从冻结原manifest及可信链重建一次35边基线，核对natural/source/plan与旧证据，不只assert边数。阶段调用、seed/precision、输入hash记录齐全。
2. 每态在副本生产append AB/BC；任何失败保存原文/坐标/阶段，丢弃整个副本。第一段不能单独获信用。
3. 保存完整AB/BC request/design/profile/hash、共享196格、AB末端/BC站9及18值。继续用正确candidate_site而非road fallback。
4. 两段通过后对原BC的两请求按原station列表measure，最多4calls/32attempt。首选station18是观察目标，**不重排请求/站点来争取通过，也不只筛选有利窗**；记录所有尝试和原始detail。测量是预测，不算真实信用。
5. 预测零则不realize，按reason/note/at/natural/design/支撑极值/冲突entity/before/after交回。如果出现与G6-R1同类support-change，本次trace中保存完整字段即可，**不在同一轮另起carve诊断扫描**。
6. 首次需要完整验证才baseline realization≤1，建立全部原parcel/owner/geometry/access/逐parcel yard承诺；然后候选realization≤2，复用同一baseline。调用现production encoder/source声明、shared road audit、frontage/development/landcover/access和commit_decision，不改审核。
7. 新增必须planned==realized-with-access、新parcel yard缺额0、旧需求不退化/不交易、旧ID/几何/入口/access保持、旧逐parcel yard不恶化。仅有原13m²旧缺额时按现批准的逐parcel相对保护规则，不用总数替代。
8. 首个真实增量且证据完整立即停，不强求执行第二态或扩到24。只保存候选证据，不发布世界、不运行CLI生成。所有长阶段返回后及输出前截止，超时best=null/incomplete=true。

## 5. 硬预算与停止规则

- **整轮自然harness≤1次**，≤2唯一态，无重复；候选measure≤4、attempt≤32。
- baseline realization≤1、candidate realization≤2；整轮runtime≤120s，编译单列。
- 允许正常编译和精确合成测试；任何自然执行失败、断言失败、序列化失败、进程被杀或证据不完整，**立即停止**。保留失败原始日志，不能改代码后再次自然运行，必须等新授权。
- 0 bake（正式/TempDir均0），0旧自然对照重跑，0额外carve重放/128/clock80，0 CLI世界/gate/release/browser/max/workspace。
- 不Git写操作、不agent、不F/A、不改冻结世界/旧证据、不改生产合同/阈值/地形/需求。

## 6. 顺带勘误，不能变成新的研究支线

向G7报告追加：B东/C西；18m是blend初值非实际值，原R2 BC实际blend97.626m；纵向solver不包含carve合成；两个旧点natural可从冻结文件读取为-606/-535mm；冻结scene.json存在实体几何，精确before贡献者仍未定；北窗上界7726。旧回执不覆盖。

保留这些为勘误，不借机重放station90或研究全局carve架构。

## 7. 输出与返回

新目录G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g8-st18-first-increment/；文档G:/difyllmwiki/GlyphWeave/docs/collaboration/b01-g8-st18-first-increment-result.zh-CN.md。

交付：预注册/启动记录、输入源码hash、唯一自然运行日志、阶段/call计数和总耗时、完整profiles/raw traces、全部候选实体与支撑数据、baseline前后hash；如运行realize，附原baseline审核与candidate完整审核、真实增量ID及request/parcel/owner/access/yard绑定，不只摘要success。

SHA256与BLAKE3分别明示，不混算法。每个失败/中止运行也计实际预算，缺失计数unknown，不以最终一次耗时概括全轮。

结果分四类：road_append_refused / frontage_measure_zero / realization_or_commit_refused / increment_witness_ready_for_review。最后一类仅新增≥1且现审核全通过；无论哪类都不签收B、不推进C、不自行提交或生成最大图。doc-tree/diff/hash检查后停止等复验。
