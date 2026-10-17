# B01-G19R1：封闭穿墙/假通行漏洞的同范围修复整包

日期：2026-09-20。工作根G:/difyllmwiki/GlyphWeave。承接G19尚未通过的人工MVP验收，不是扩展新设施或申请自然额度。

## 0. 先读及目标

先读：
- G:/difyllmwiki/GlyphWeave/docs/collaboration/b01-g19-inspection.zh-CN.md
- G:/difyllmwiki/GlyphWeave/docs/collaboration/dsh-b01-g19-engineering-connections-mvp-task.zh-CN.md
- G:/difyllmwiki/GlyphWeave/docs/collaboration/b01-structure-aware-connectivity-direction.zh-CN.md 第9节（用户最新穿楼/穿墙限制）

G19状态为implementation_present / acceptance_blocked。保留已有生产模块、CLI和几何工作，不回退成文档；一次修复P1-F1至P2-F6并补真实反例测试、CLI读回与证据。不交“已收到”“请批准先修一条”或旧包勘误。

## 1. 用户硬规则：本轮不准穿楼穿墙

建筑实体、墙、楼板、平台/楼梯/支柱的实体占用空间默认硬阻挡。通道、行走体积、拟建结构和支承不能穿入。以下均不允许作为修复：移除障碍、缩小输入障碍、自动开洞/拆墙、把整栋楼排除、假设存在门洞、改类型绕过、因现实有特例而放行。

已声明真实入口的外侧边界可对接，不等于允许在实体内通行；本轮不增加室内导航、portal、穿楼、隧道或凿洞功能。输入请求这些功能也要明确拒绝/unsupported。未来若实现，另行专项授权。

不得把“不把结构当ground障碍”保留为可接受v1局限。本轮需真实修好，不能靠报告免责声明过关。

## 2. 直接使用的检查员反例

目录：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g19-20260920/。这些是独立人工场景，不是ST18，不可改写；复制到新包作回归输入。

- control-flat：应继续成功的完整正控制。
- stair-inside-solid-obstacle：原成功楼梯中段有明确实墙，旧builder+verifier仍通过。
- ground-through-solid-platform：ground路径穿接地实体平台，旧found/verified。
- ground-overhead-obstacle：0.5..1.5m高的横向障碍被仅地面y过滤漏掉。
- wheelchair-local-hump：局部坡度0.2、两端等高，被端点平均坡度当轮椅可通。
- missing-context：缺protection/obstacles/water/bearing字段被默认空。
- unknown-surfaces：两个不存在的surface/level被ground边连接。
- reverse-flat：查询b→a，输出折线却a→b。
- pedestrian-hump-zero-rise：实际累计上升1m，maxRise=0仍found且报告0。
- descent-zero-rise：实际纯下坡被当上升2m，误拒。
- publish-collision：预存report，CLI失败但新成功包已落盘，随后可verified。
- overflow-scope：极端i32矩形触发panic101，无结构化拒绝。
- tampered-payload-hash：目前正确拒绝的负控制，修复不能破坏。

详情见probes.json、extra-probes.json、rise-probe.json及每案input/report。无需重跑旧G18或再证明这些反例存在；修复前后运行结果要绑定版本。

## 3. 六组修复工作必须同包覆盖

### R1 碰撞/净空/支承的统一约束

将输入障碍、新结构所有实体mesh/保守体积、支承和有类型的可走表面纳入同一验证上下文。楼梯与平台构造时检查实际体积；所有路径边检查明确的通行宽度、身体/净空范围，不只检查地面高度线。

允许声明保守AABB碰撞近似并据此拒绝，不能在不确定时接受。区分合法外表面接触、在平台顶面通行与穿平台实体，不要用“忽略自己structureId全部零件”躲过自身错误；转向平台/楼梯跑段须实际有容纳空间。受控挖填结果同样可能与旧物体/支承冲突，要按明确范围重查。

新CLI构造和独立读回都要拒绝三项穿墙/穿平台/低障碍反例。补合法相邻接触、足够净空且无实体冲突的正例，避免所有结构一概拒绝。

### R2 按真实局部几何分配能力

每种已支持能力用自己的局部坡度/宽度/净空检查，不能把端点平均grade当每个采样都过。使用已保存/生产同口径三带采样；若某能力需要更宽走行体积，用相应宽度，不可声称3m车辆通过却只检1.5m带。

楼梯对wheelchair/vehicle/rail保持拒绝。当前并未真正支持的模式可返回unsupported，不要凭pedestrian检查直接授予rail/vehicle通行。保持旧ground0.25等默认合同不变；项目能力参数不是现实法规。

### R3 缺失/引用不明不得转成安全

明确环境必填字段；显式[]与missing不同，missing返回incomplete/input_invalid。验证端口surface/level/owner/road归属的真实引用：地面引用解析到权威grid，结构引用解析到实际结构及层/面。几何点高度对上不能替代引用存在；标签不能任意复制即verified。

允许在新模块内补必要注册表或明确的保留地面id规则，仍需版本/迁移说明，不修改默认world schema。对于尚不能绑定的外部入口/道路，明确不支持或incomplete，不凭一段字符串假称接到真实BC。反例unknown-surfaces应拒绝，同时ground/合法楼梯平台正例可用。

### R4 路径方向、升降计量与预算一致

根据实际遍历方向反转边折线、连接点和升降；最终path从query起点出发、到合法目标端口，连续且不跳层。从实际有向polyline累计升高与下降，不使用abs(端点差)，不能漏掉同端高的中间起伏。

搜索预算与输出计量使用同一口径。不得在确认更优/非支配状态前覆盖rise标签；若有多资源预算，使用有界非支配状态或明确的保守求解策略，但绝不把实际超限路径报found。反向flat和上坡/下坡/hump回归、环与单向回归都覆盖。独立验证端直接核对输出路径几何，不仅用同一route函数再得到同样错误。

### R5 发布事务与最终门

预检输入/输出冲突，使用staging+原子发布或有严格commit状态的等效方案。遇到已存在路径/序列化失败/视图失败/超时，不留下可当成功消费的包。不能删/覆盖用户原文件；清理只限本轮确定归属的新临时物。

验证器不能把未commit或不完整发布的孤立产物标verified。最后的预算/完成门应在序列化及发布前后正确衔接，保留机器可读错误；不能只依靠外层脚本运行很快。

### R6 输入范围和授权算术安全

对矩形次序/长度、origin+size、target-current、差值绝对值及计量使用checked/i64等安全算术，先校验再分配/循环。极值输入结构化拒绝，不panic；矩形反向、target长度不符/歧义、负授权额度、非有限数、过量对象也应处理。

不能通过clamp悄悄缩小用户工程范围来让测试过；输入非法就拒绝。禁止对象/工作计划重复id不得静默合并。累积操作的授权与保护语义须明确，失败不半提交。

## 4. 审核要求和功能保留

- 不仅测“改坏输出hash”，还要测“原始输入设计就不合法”。builder与verifier共享原语可以，但必须有独立约束检查；不是两次调用同一个builder就叫独立证明。
- 保留直跑/折返楼梯、平台、受控cut/fill的合法正例及几何输出。旧成功fixture若本身实际穿插而新规则正确拒绝，不降低规则迁就它：在新R1输入中明确更正几何并披露，旧fixture不覆盖，不伪称原输入仍通过。
- 保留17项现有核心测试中仍有效的断言；若原断言在依赖错误行为，说明具体替换原因。补此次反例和必要的反向/分层/缺失上下文测试。
- 检查员已找到87390B精确原access快照：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g13-offline-access-proof-bundle/source-snapshot-access.rs，SHA256 e62d41bf…。直接引用，不再重建或编辑旧基线文件。
- 新回执绑定本轮清单的hash和字节；清单不含回执，避免循环。旧G19回执不倒写。

## 5. 写集、预算和停点

写集沿用G19白名单，主要改新core/CLI engineering_connections模块及相邻测试。无auto_roads修改、无旧证据覆盖、无默认生成开关、无新设施类型、无真实输入重建。自然/ST18/旧世界bake/realize/发布全部0。

这是G19必需能力的自修，不是新自然批次。worker原G19已报CLI13/32，R1 CLI验收≤19次；尽量以一次定向core测试覆盖多反例，cargo追加≤16次，合并旧包可证命令仍不得超原40。检查员24次CLI/2次cargo为独立审查成本，另列，不混进worker成本或隐去。已发生的旧cargo记录不全应标unknown/约数，不倒填日志。

人工基准场景最多沿用原10个加检查员flat/hump两类（反例为明确变异），单面≤64²、结构/图上限保持G19。新证据目录G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g19r1-engineering-connections-repair/，本轮≤64MiB；CLI单次≤30s、累计≤600s。预算触顶交partial，不自行扩额度。

一次交付：六组修复→代码路径→精确回归→CLI正负例结果矩阵；全部输入、失败日志/执行版绑定、实际几何视图、清单/回执和只读核验。不再仅写“17项全绿”；必须说明本次每个反例如何被拒绝或正确处理，以及合法构造是否仍能通过。

完成前不接入ST18/SceneIndex/preview/audit默认世界，也不提出“先带病接入再说”。提交后等待检查员复核，A不变/B partial/C不推进。