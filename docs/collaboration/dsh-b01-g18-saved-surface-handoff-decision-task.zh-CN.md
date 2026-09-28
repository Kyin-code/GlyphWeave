# B01-G18：固定 G-W1 保存面交接诊断与最小修复决策整包

日期：2026-09-20。主检查员依用户“请编辑好任务给DSH”授权本包。工作根：G:/difyllmwiki/GlyphWeave；不要在 F:/modenmapbox 执行项目工作。

## 0. 目标、边界与执行方式

一次完成：输入核验 → 分段清单冻结 → 采样器自检 → 保存面交接诊断 → 图表 → 至多一个修复提案/停止决策 → 一次交付。普通白名单内脚本错误可自行修复、复算并保留日志，不需用户逐小步转达；不交接收确认或中途勘误包。

唯一问题：固定七块 G-W1 的预定交接链在**已保存最终工程面**上有哪些具体坡度不连续，现有材料是否足以支持一个值得后续检验的局部修复？不是重新判全目标可达性，不追求证明唯一入口、最短路线或全域无解。

本包只有诊断对象，无新的执行候选。G17 固定候选仍按 access_rejected_at_fixed_candidate 关闭；其 D1 仅指已执行旧 support、pad 与现有前检，不概称完整工程合法。G16A 证明线关闭，原北窗可达性 unknown。A 冻结、B partial、C 不推进。

**硬禁止：自然/计划重建/真实或反事实 terrain、carve 合成/生产 route、access_for_entity、build_access_graph/measure/candidate_final_terrain/realize/bake/世界发布全部 0。** 不实现替代路由器或离线 A*，不执行候选搜索、消融、新参数栅格或自然补档。不修改任何产品源码（含 cfg(test)）、合同、默认值、schema；不运行会 create_new 历史证据的 cargo 回归，不做 Git 写操作。

## 1. 必读与输入绑定

先读以下绝对路径，不重读/重跑全部历史：
- G:/difyllmwiki/GlyphWeave/docs/collaboration/b01-g17-inspection.zh-CN.md
- G:/difyllmwiki/GlyphWeave/docs/collaboration/dsh-inspector-reporting-protocol.zh-CN.md
- G:/difyllmwiki/GlyphWeave/docs/collaboration/dsh-b01-g17-west-entry-engineering-bundle-task.zh-CN.md（只取冻结几何及历史范围，不能继承其自然额度）

输入目录 G17 = G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g17-west-entry-engineering-bundle/：
- surface-natural-mm.bin、surface-GW1.bin、access.json、run.json、rule-table.json、authenticity.json、d1-matrix.json、c0-crosscheck.json、executing-source-auto_roads.rs、清单与回执。
- protection-domains-derived.json 只能标记事后派生来源，不得冒充当批完整实体/保护域快照。

输入目录 G12 = G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g12-access-cause-decision-bundle/：surface-G-1d-north.bin（C0）与必要时 surface-baseline.bin，引用其清单。

只读生产采样依据：G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/worldgen/access.rs。开工记录实际版本与 hash；预期 SHA256 e62d41bf6e1603800b969804b581c01b9736ad7846018828924f169b85bf8174，若不符停止一致性判断，不默用新版。

G17 真实自然 BLAKE3 88cca57680e7b7cf6b183abd9217e4fc6e1ed959d905d454a14051ea0e54f08c；仅限同冻结输入复用。保存面为 GWHM/version1/512×512/16B 头+i32 毫米，坐标原点(0,0)，每面1048592B。核验实际头、单位、长度、hash，不用文件名代替身份。只核验本包消费的旧文件，不再全面复制历史目录。

## 2. A：先冻结有限分段清单

七块参数以 G17 rule-table/run 逐项核对为准，顺序不变，priority2、blend2、profile=None、disturbance160：

|块|cx|cz|halfW|halfD|targetMm|
|---|---:|---:|---:|---:|---:|
|T1|260|162|1.5|7|2390|
|T2|257|162|1|7|2160|
|T3|254|162|1|7|1930|
|T4|251|162|1|7|1700|
|T5|248|162|1|7|1470|
|W1|210|162|38|7|-10000|
|W2|171|165|1|9|-500|

诊断必须覆盖四组：①出口(270,162.85)→T1；②T1→T2→T3→T4→T5→W1，含间隙及重叠；③W1→W2 转弯；④W2→指定 BC 西端的末段。末端坐标须从已绑定 BC 几何推导，区分 pavement、工程顶面、保护矩形，不能随便选最近可过点。

允许基于固定几何制定一个代表性折线及必要的交接横断面，**在读取沿线高度并据此挑选路线之前**冻结 segments.json；每段记端点、方向、分组、选点几何规则、与七块的关系。最多32个有向段，诊断折线与横断面分别标记，不拼成虚假实际路径。不得按结果移动端点、扫描平行线找通过带、自动增加分支；清单设计错误可另存修正版本，但不得以提高通过率为理由修订。

代表性折线不是生产返回路径，也不是所有可选路线的穷尽；若沿某块中心连线穿建筑，不据此宣称七块不可行。本包不要求证明“该折线必经”。转角处直线三带通过也不能替代转弯扫掠宽度/障碍审核。

## 3. B：采样口径自检与保存面复算

在新包中实现独立只读脚本，不向 auto_roads.rs 追加测试。按当前冻结 access.rs 的实际运算顺序复刻 terrain 高度转换、双线性插值与坡度部分：整数毫米→生产 f32 米→f64 插值；三带 lateral=-0.75/0/+0.75m，n=ceil(length/0.5).max(1)，实际 step=length/n，逐相邻样本与生产阈值/容差比较。不得一律用0.125m判断1m格差，不得把 NaN/缺格替成0。

自检至少覆盖：非整步长、侧带而中心通过、非零插值邻格、恰在阈值及略超、缺采样拒绝。人工小数组是 fixture，不是新的真实地形合成，次数单列。重算 G17 保存的全部8条 grade 样本在 G-W1 面上的高度/delta/limit；JSON 十进制回读误差与 f32 运算差异分别解释，预先声明数值误差界，不通过放宽坡度阈值让复现通过。复现失败先修采样实现；剩余无法解释则按 technical_incomplete 停止新断言。

对清单全部段分别在 natural、C0、GW1 三面采样，保存每带完整样本与失败对；不可只保存第一个失败使中段缺证。每个失败对记录：segment_id、采样序号、真实坐标、带偏移、两点高程、水平步长、delta/limit/slope，以及全部非零权重邻格坐标、权重、三面格值。按段总结首个失败、最坏失败、失败对数与检查范围。

只对实际证据给出分类：已存高度差、三带坡度通过/拒绝、自然下限形成的局部约束、已知保护域交集、来源不足。保护域引用要列源文件/字段；已知 AB [279,93,297,183]、BC [173,171,291,189]、pad [262,150,278,164] 不等于所有实体域已齐全。不能仅因某采样端点在保护域内就宣称失败永远不可改；如给固定样本证书，必须核验全部非零权重邻格与对应不变性来源，并只限该采样对，不做新全域边界枚举。

W1 不是“恢复自然”的同义词。对三面差异仅作观察，不把整包差异按影响盒独立归因到某块；无消融/运行来源就保留因果 unknown。

## 4. C：记录覆盖审计与两张解释图

G17 access 只有计数及有限样本：467次grade拒绝中459条截断，76次obstacle拒绝中68条截断。读取实际字段核对；明确保存的样本 context 是 direct 还是 search_step。不得从样本截断区推测搜索前沿、节点坐标、最远到达位置或唯一根因。

保存一张局部平面图（固定块、pad、已知AB/BC保护域、固定诊断段、失败点）和一张沿代表性折线的三面/三带剖面图，图中标单位、方向、段号、数据来源。绘制非地形合成。图例明确“诊断折线，非生产路径”；unknown 障碍/未归档域不得画成已证空白安全区。用本地脚本生成 SVG/PNG 即可，不引入大型依赖。

## 5. D：同包完成一次修复决策

必须在本包中选择有证据支撑的一个分支，不再交“请批准分析下一小步”：

1. **localized_repair_proposal**：发现具体可讨论的局部交接缺陷，给至多一个书面修复提案。列精确块表/修改前后差异、保留/删除/新增对象、拟消除的具体失败样本、对自然下限及已知保护域/pad的影响、仍未解决的失败段/障碍/road_id/完整域缺口。明确是待验证假设，不宣称调整后最终面已通过；target 坡度不等于地形坡度。不得合成该提案或试多套参数。只有解释足够时才给后续唯一变量与自然预算建议，本包不执行。
2. **no_justified_local_repair**：多个相互独立问题、已知合同冲突或现有证据不足以支持局部修改，建议暂停该构造线并列出真正需决定的条件。不推论所有西端方案不存在，不强行为交付凑候选。
3. **insufficient_saved_evidence / technical_incomplete**：准确说缺哪个字段、哪项问题无法回答。已能完成的 terrain 诊断仍交付；不伪造现场表、不自然补档、不把缺少障碍表当理由跳过可以做的地形分析。

地形全段通过时，只能说这组段未发现坡度阻断，下一疑点可能在障碍、水、生产搜索约束或未覆盖段；没有材料不强选根因。至多一个推荐后续动作，不给用户一串模糊授权菜单。

## 6. 累计预算、写集与交付

- 自然及第0节禁止动作全0，产品/测试源码编辑0，cargo调用0；不调用外部agent。
- 固定几何诊断段≤32；真实保存面完整诊断执行≤8次（含自修复），所有离线脚本执行累计墙钟≤120s，含自检/读取/采样/图表/序列化；人工阅读及输入/最终hash核验耗时另列，不用拆进程重置预算。每段三带/三面对比单列实际采样数。耗尽时交 partial，不扩额度。
- 新证据写集仅 G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g18-saved-surface-handoff-decision/；若已存在，不覆盖，先辨认是否本任务续作，不能覆盖历史文件重新伪装首跑。脚本可在冻结前自修，执行版逐次绑定；日志和已冻结产物均新名/create_new。
- 保存量≤15MiB。原面仅引用，不复制，不保存大幅源码副本；保存实际执行脚本与输入hash、源码hash/相关行号即可。不要为清单数字复制全部旧证据。
- 必需交付：输入绑定、冻结segments清单、脚本与自检/失败日志、逐样本机器证据、分段矩阵、两张图、一个决策报告、累计命令/预算、清单及回执。清单不包含回执；回执绑定清单，避免循环hash。
- 最终一次只读核验输入未变、源码未变、新清单与回执匹配；执行 node scripts/generate-tree.mjs --check 和 git diff --check，如有既有问题如实区分，不全局改行尾或暂存文件。
- 报告按统一协议，一句话结果→带JSON路径的证据矩阵→新信息≤3→成本→一个决定→交付。测试PASS单列，不能当可达命题verdict。无需再交确认稿/勘误包；新提案未获自然执行授权，交付后停止。