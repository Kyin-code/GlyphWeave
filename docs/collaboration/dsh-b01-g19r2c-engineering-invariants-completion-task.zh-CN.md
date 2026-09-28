# B01-G19R2C：G19R2不变量修复续作整包

日期：2026-09-20。用户已要求“检查，发布任务”。本任务是G19R2未完工程的上下文接续，非新类型开发，非自然运行授权。工作根G:/difyllmwiki/GlyphWeave。

## 0. 接手目标与禁止事项

唯一目标：完成G19R2已承诺的工程连接不变量，使合法直跑/折返楼梯、平台及受控挖填成立，所有已发现误放行/误拒绝反例得到通用规则修复，并交付生产CLI+独立读回+可版本控制回归的完整人工闭环。

先读：
1. G:/difyllmwiki/GlyphWeave/docs/collaboration/b01-g19r2-partial-inspection.zh-CN.md
2. G:/difyllmwiki/GlyphWeave/docs/collaboration/b01-g19r1-inspection.zh-CN.md
3. G:/difyllmwiki/GlyphWeave/docs/collaboration/dsh-b01-g19-engineering-connections-mvp-task.zh-CN.md（只继承功能目标与白名单，不继承旧额度）
4. G:/difyllmwiki/GlyphWeave/docs/collaboration/b01-structure-aware-connectivity-direction.zh-CN.md 第9节穿楼/穿墙限制。

当前生产core快照245678B/SHA256 cdf8ae9aa51bfb73f467f532a3620402bbda4e0c24f3f61ea28fc270ae70ea2a；CLI仍为G19R1版。开工实际hash不同先查明谁已改动，不覆盖别人工作。旧源码/证据不得删除回退。

自然/ST18重建、旧世界bake/realize、默认SceneIndex/preview/audit消费者集成、世界发布全部0。禁止穿楼/穿墙/自动凿洞/移除障碍/缩小实体/虚构门洞。不得调宽旧坡度或通行阈值来过测试。机场/港口/复杂建筑/新交通类型不塞进此包。

## 1. 先修接触语义，不能机械应用worker未验证建议

### 1.1 不批准统一0.25m实体豁免

MAX_SUPPORT_STEP_RISE_M当前在所有能力共享的corridor_blocker中跳过全部低实体，已证实轮椅能穿过0.2m显式障碍，build+verify同时放行。不得保留为默认规则，也不得简单改成更低常数继续掩盖误判。

ground边只沿已验证工程支持面通行；楼梯通过stair边升降；landing在真实支持面内连接。合法的端面/上表面接触不等于可穿入体积。未实现且未经授权的跨步/爬越/门洞不能通过“接触”推定存在。轮椅不得继承行人跨步假设。

回归来源：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g19r2-20260920/wheelchair-low-solid/input.json 及同目录wheelchair-taller-solid。低障碍和高障碍都不能令轮椅ground路径穿实体。

### 1.2 接驳空间连续性，而不是采样点之间留缝

允许重新设计通行体积为有向几何，但不要盲目把正方形换成每.5m一个纵向厚.1m的片；单片之间的空白也是路径的一部分。

明确：每能力横向宽度/身体净空、相邻路径点间的连续检查范围、转向处的覆盖、ground与stair/landing的实际共同边界/端口。可以用连续扫掠或严格覆盖区间的保守体积，不要求某个固定实现，更不预先批准SLICE_HALF=.05。

成对验收：合法approach到楼梯脚能到达真实端口且不把第二级错误当正前方整段墙；薄墙/短支柱/低实体放在采样点之间或终端附近仍被检查；反向/斜向/转角不漏。障碍和新结构部件同等进入连续检查，不能只给obstacle补平面检查而漏structure。

## 2. 完成能力与端口真实几何

### 2.1 能力判据

保留当前各能力独立带宽和水域判断修复方向，补完整回归：车辆侧带涉水拒绝、轮椅带内平坦而车辆带外陡坡不应误拒、0.4m梯面拒绝、合法1.5m以上支持面不被误拒。

如果某一种能力实际不能支持，明确unsupported，不拿行人判据背书。不能因为ground边对行人失败，就不加解释地推断所有其它模式失败；说明当前共同支持面的限制及每能力独立判定结果。

### 2.2 端口归属与方向宽度

对每个非地面端口：真实surfaceId/levelId→具体部件的该层支持面；检查真实轮廓内位置、标高、端口方向、横向可用宽度及与相邻连接兼容。旋转件AABB可粗筛，不能最终用AABB内点代替实际面内点；不能把max(AABB宽,长)当净宽。

可以为端口补显式方向/连接面或由相邻边可靠推导，并在新模块合同中明确；不能更改既有世界schema/默认行为。新增字段要有版本/迁移规则；不明方向不得伪造一个最有利方向通过。

至少覆盖：远处端口贴真实平台标签、同x/z错层、旋转部件AABB内但轮廓外、窄长平台/踏步错误横向宽度、合法转向平台。地面端口保持真实栅格采样与有限数/缺格拒绝。

## 3. 完成状态绑定的搜索与独立验证

### 3.1 搜索

每条有向边的climb/descent来自同一实际折线，所有长度/升降与最终输出一致。非支配label分配独立身份并存自己的父label/入边/方向；不能仍用node全局前驱覆盖。

保持有限状态/标签/扩展上限，碰上上限明确incomplete或预算状态，不升级为全局不可达。超爬升短路与预算内平坦绕路并存时，必须找得到后者；单向、反向、起伏、纯下坡、循环、分支枚举顺序、多个非支配状态均有回归。改变边顺序不能改变合法性结论。

### 3.2 独立审核

从权威输入+保存工程栅格+保存结构几何重算支持面、每能力三带坡度/水域/实体净空、真实端口包含、路径方向/连续性及预算。产物中facts和capabilities是待审核的声明，不是审核依据。

保持input hash与payload hash检查，但不能只有builder重建相等就verified。给原始非法设计与保存包篡改两类反例；即使改了facts并重算hash，真实采样/几何不符仍拒绝。几何事实缺失返回incomplete，不默认空/0。

## 4. 补回可交付测试与发布验证

### 必需资源与测试清单

把下列反例整理到受版本控制的tests/resources或由测试代码生成，worker work目录只归档运行记录。不要以旧work文件存在作为CI前提。
- inspector-b01-g19r1-20260920中：terminal-own-stair-solid、narrow-stair、vehicle-side-water、wheelchair-outside-band、misbound-existing-surface、hump-with-valid-detour及valid-detour-control。
- inspector-b01-g19r2-20260920中：wheelchair-low-solid与wheelchair-taller-solid。
- 合法直跑/折返/平台/cut-fill正例、原R1的13case仍需相应复核；旧fixture确有几何缺陷可在新资源中透明修正，不能移动待测障碍或缩测试范围伪通过。

把原G19的17项断言逐条列“保留/替代及精确测试名/明确废弃理由”。只合并测试函数名可以，但不得丢支持、单位、非法尺寸、单向/循环、篡改、事务等有效断言。

新增成组有限变体而非大量cargo单项调用：平移/旋转/反向/宽度边界/障碍放在两个采样点中间/路径分支顺序。保留成功与拒绝对照，不能只有拒绝测试。测试通过数量不替代不变量verdict。

### 发布

延续原任务未完要求：最终成功状态与完整发布一致；在staging提前写committed不能自动证明已发布。结构/视图/report/最终manifest之间的可消费合同要明确，未完成集合不能被正常验证当成功。

明确时钟门包含最后序列化及提交；输出冲突、序列化/视图失败、模拟提交失败均保留结构化拒绝且不覆盖旧文件。只清理确属本次的新临时物。测试可用本地故障注入，不需要自然运行或实际中断别人进程。

## 5. 写集与上下文管理

沿用G19白名单：新core/CLI engineering_connections模块、相邻小辅助模块/测试、core tests/resources/g19r2与新包证据；worldgen/main/access/baked_terrain仅原授权最小必要范围，若不需改就不动。禁止auto_roads、旧世界清单/默认值、历史证据、无关大规模格式化和Git暂存/提交/rebase。

允许把碰撞/支持面验证、搜索label或回归分到相邻私有模块，限于本包直接需要、保持公开入口；不为整理整个5000+行文件另开重构项目。优先读取目标函数和本任务，避免反复把所有历史输出灌满上下文。

开工在新包记一份简短progress.json：已完成/待完成、输入与执行源码hash、累计命令序号/次数、下一具体动作。重要阶段更新该进度，日志/执行版本用不可覆盖新名。上下文不足时可以在同一任务接续，但不得重置次数、擅增权限或把unknown改成pass；进度不是替代最终交付的确认稿。

## 6. 预算：延续G19R2，不重置

- G19R2原总上限cargo30/CLI40。按worker已报cargo12/CLI0为本次账本起点：**续作cargo最多18次、CLI最多40次**，所有成功/失败/自修复/验证累计。若发现旧轮漏计，扣减相应剩余额度，不把自报数变成已独立核验的历史事实。
- 检查员本次2cargo/3CLI单列，不计worker额度。不能把上下文限制叫作工程调用预算已用完。
- 单面≤64×64，结构/端口/边上限沿用G19；CLI单次≤30s，G19R2累计≤600s；新归档≤64MiB。core合并测试可以一次检查多个人工case；编译/测试耗时另列。
- 自然/ST18/旧world bake或realize/默认发布=0；人工结构构造、terrain工作面编辑、图搜索、CLI导出如实计数，不能统称全0。
- 新目录G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g19r2c-engineering-invariants-completion/；换目录仅用于隔离partial证据，不产生新额度。

## 7. 一次交付与停机标准

按顺序完成但不逐项请示：接触/连续空间→合法与拒绝成对回归→能力/端口→label搜索→独立重采→发布/原17映射→CLI与实际几何视图→一次绑定核验。

交付：
1. 完整代码与可复核本轮差异，开工/执行版绑定；
2. 支持面/碰撞/能力/端口/搜索/读回/发布的不变量矩阵、精确测试名及正负结果；
3. 全部输入/失败日志/调用账本，原17断言映射；
4. 合法直跑、折返和工作面变更的实际生产网格视图，不能用示意图代替；
5. 清单/回执（清单不含回执，回执绑定清单最终hash/bytes），doc-tree与局部diff检查；
6. 本轮真实状态：全部必需链成立才completed，否则partial并列剩余具体项。不得把“5个旧测试回绿”单独当成整包完成。

如预算或上下文再次不足，先保存可接续状态和已有日志，停止任何未授权动作；不新跑自然补档、不宣称没有半成品。A不变、B partial、C不推进。整包完成后一次汇报，无需先回复收到；未验收前不接入真实ST18或默认世界。