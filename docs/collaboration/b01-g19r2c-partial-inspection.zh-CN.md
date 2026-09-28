# B01-G19R2C阶段复核与收尾裁定（2026-09-20）

## 结论

**verified-partial-progress / final-geometry-and-regression-gates-open**。

这次已验证的进展明显：合法楼梯正例恢复，低实体误放行、原钻自身楼梯反例和预算内绕路均按预期修复；label独立前驱和保存栅格重采已在源码落地。保留这些成果，不重开旧实验、不回退为纯文档。

仍是partial：端口真实轮廓/方向净宽未完成，有限变体、原17断言映射、修复hash后独立重采负例尚缺；发布的已测故障与所有实际I/O故障不能混称。暂不签收整个工程连接能力，不接ST18/SceneIndex/preview或默认世界。A不变、B partial、C不推进。

## 独立核验

目录：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g19r2c-20260920/。

- integrity.json：现盘清单57/57 SHA256+字节匹配，回执对清单hash/bytes绑定匹配。core=256679B/a277fd480ed9789e42bc5406347ef1ffb37ceb6e31f100552cd8ead9d74636ae；CLI=17417B/4bd91443e2dfbd293b0d611f47a9552f6769abdb36c6e4b5ac8ba299f3ea2881。access/auto_roads与前轮不变。
- 测试目录实际22个输入+1个expectations文件，不能把23个JSON都称23个场景。检查员保存了所有资源当前hash，worker清单未绑定这些回归输入；最终收尾应引用相应版本。
- 检查员2次cargo：重编CLI成功，定向core实际5/5通过。矩阵含22case，不宣称已恢复原17项全部独立断言。
- 检查员9次生产CLI，均在新目录和人工输入上执行：positive-terrace built/found→verify verified；wheelchair-low-solid拒绝；terminal-own-stair-solid拒绝；hump-with-valid-detour found（18m平坦绕路）→verify verified；views/report/bundle三处注入失败均无残留bundle/views。
- 未运行自然/ST18/旧世界生成，未编辑产品源码或worker旧证据。

## 不应重做的已完成项

1. 逐间隔检查的有向band已替换旧“在采样点放纵向厚片”方式；统一0.25m实体豁免已删。旧低障碍与钻楼梯反例均复现为拒绝。
2. 每模式独立坡度带和水域检查代码已在位；当前矩阵与CLI记录覆盖原车辆侧水/轮椅域外坡度反例。
3. SearchLabel包含自己的父node、父label index、入边和方向，labels追加存储，已不再只用node前驱。
4. verifier已从stored.surface重采ground各能力带，读取保存实体做体积检查，再对比授予能力。它不再仅重读saved facts。
5. 3个已实现发布故障钩子均已独立重测；应保留，不必再为复述结果重跑一整包。
6. 本轮现盘清单/回执本身已由检查员只读核验，不必把“重新制作一份确认包”当下一工程。

## 收尾前仍需处理

### A. 端口真实支持几何（必须实现，不是文档项）

validate_port_membership仍用part.aabb包含，再用max(xExtent,zExtent)作为member_width。对旋转矩形、窄长平台、有多个同高部件的结构，标签存在+包围盒内不等于端口处于该层真实支持面。还需要把命中的member和level_id、端口方向及连接方向绑定。

用真实上表面四边形/局部坐标做最终包含，核验端口截面与接驳带在支持面中的覆盖，而非只有中心点；AABB只能粗筛。新增方向语义若必要应在新工程模块合同中明确，不能暗改旧世界接口。

### B. “终点落脚线”代码与表述尚不完全一致

corridor_blocker约3296起，terminal取第一/最后采样区间；owner_structures是两端结构集合的并集。landing_line条件只检查terminal+part.role+structure ownership，然后continue，并未单独计算交集是否只位于端点线/真实接触面。

现有反例已修复，但“只认落脚线、不豁免其他体积”的普遍命题还不能由该代码和22个case推出。有限变体必须覆盖：始末端分别不同owner、倾斜接驳、末区间内但不在端点线的实体、转角和反向。若违反共同接触面规则须改实现，不能把terminal区间换个名字当证明。本轮没有新运行这些变体，不将静态风险冒称已复现的新误放行。

### C. 独立重采要有可归因的篡改负例

“只改payloadHash被拒”不能证明重采工作。需要修改能力声明及相关facts，再用公开生产hash函数形成自洽包，断言hash门通过，而stored_capabilities_resampled或相应几何重查明确失败。若只被builder全量diff抓住，应如实说独立重采证明尚缺，不能混算。

只改解释字段但能力未变时，可由字段一致性审核拒绝；该情况与伪造可通能力的重采反例分开设计。

### D. 发布故障钩子不等于真实写入失败全覆盖

CLI目前write_new(out,bytes)成功后才created.push(out)。若create_new已创建最终文件，write_all/sync_all再失败，该路径尚未进入清理集合。现有bundle注入是在调用write_new之前，因此未覆盖此分支。此处是静态可见缺口，本轮未模拟磁盘满/进程崩溃。

最终文件还应采用完整序列化/写入临时物后原子发布或等效严格提交方式；如保留直接写，必须保证部分创建文件有明确归属并清理/不可消费。最终deadline检查应覆盖report序列化和最终提交，不能只检查前面的时间点。不要为追求committed字符串“只出现在目的地”而牺牲实际发布原子性。

### E. 剩余回归/映射

平移/旋转/反向/宽度边界/采样点之间障碍/分支顺序需要成组、有限、可复现测试；正例和拒绝例都要保留。原17项有效断言逐项列保留/替代/废弃理由；5个函数里覆盖多个case可以，但函数数本身不证明覆盖。

## 接续信息必须覆盖旧progress，不应照旧nextAction机械执行

- progress.json和ledger.json仍记core fbe8a58a…、本轮cargo10/CLI0、剩余8/40；progress还说label/重采/发布/CLI未做。这些是阶段旧快照，不是当前接续入口。
- 当前receipt和在盘源码对应a277fd48…，receipt自报本轮cargo15/CLI13，总cargo27，剩余3；不能再按旧progress取8次cargo。
- runs-attempt1-args-collision中另有1份exit=1的实际CLI调用日志，最终13次未包含它。按可见执行记录，CLI累计至少14，最多剩余26，而不是27；失败参数调用也要计数。不能通过“这是调脚本”排除已经启动的CLI。
- cargo27目前来自worker自报及部分日志，尚无完整逐命令账本；不把其说成已经独立确认的精确历史数。若发现漏计进一步扣减，不重置。

下一接手只需要在新收尾记录中明确标记“supersedes旧progress/ledger”，给出当前源码绑定、真实待办和累计量；旧冻结文件不倒写，无需另交确认消息。

## 建议执行顺序（沿用G19R2C，不另开自然轮）

先完成端口/接触几何和发布失败归属；一次补齐有限变体、facts自洽篡改、原17映射；然后合并运行测试/编译和必要CLI，最后出统一收尾清单/回执。不要逐项运行一轮cargo后立即交接，避免把剩余3次花在重复已完成项上。

最多剩余cargo3/CLI26（如漏计扣减），这不是承诺一定能在剩余额度里完成。超限或无法闭合时如实partial，不临时放宽规则/删反例，也不假装完成。检查员独立2cargo/9CLI另列。未有新明确授权，不自动增加次数或自然额度。