# B01-G19 工程连接MVP独立审查（2026-09-20）

## 裁定

**implementation_present / synthetic_counterexamples_found / acceptance_blocked**。

接收已有生产模块、CLI、实际网格和原17项测试的工作进展；不能签收工程连接v1，更不能据此接入ST18、SceneIndex、preview/world-quality-gate。下面人工反例表明builder和verifier共同接受物理冲突、无效身份与错误通行/路径度量。不是自然试验失败，也不是汇报措辞问题。

用户随后明确“穿楼、穿墙先限制”。因此P1-F1为默认硬拒绝，不得以穿楼特例或v1免责声明豁免。远期穿楼/隧道设计不提供当前许可。A不变、B partial、C不推进；G17/G16A状态不变。

## 方法与可复核证据

目录：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g19-20260920/。

- initial-integrity.json：清单191/191 SHA256+字节匹配，5份在盘生产源码与回执源码hash一致；审查前后源码未变，auto_roads仍fca893dc…。
- 重新cargo build glyphweave-cli成功；cargo定向执行17项新core测试全部过。另直接运行2项既有/新增access回归，均过。没有全库测试、自然、ST18重建、旧世界bake/realize或真实route。
- 使用新编译生产CLI，构造独立小型人工输入并执行24次CLI（构造/读回/预期错误），所有输出在检查员新目录，不修改worker产物。2次cargo；另2次测试二进制运行。
- probe.cjs/probes.json：9个第一组人工案例（含正控制）。extra-probes.cjs/extra-probes.json：hash负控制、发布冲突、范围溢出；rise-probe.cjs/rise-probe.json：实际爬升超限但获通过。
- 每个case有input.json、bundle/report（若产生）及stdout/stderr。资料与用户确认的生产源码版本绑定，不是只看worker PASS日志。

## 阻断发现（按优先级）

### P1-F1：结构/通行未统一检查实体碰撞，实际穿墙仍通过

源码：core worldgen/engineering_connections.rs build_straight_stair/build_landing/build_switchback_stair、build_ground_edge约2363–2440、build约3110–3149；input.obstacles只在ground边环节读取，未用于楼梯/平台/支承碰撞；生成结构也不参加ground边阻挡。

复现：
1. stair-inside-solid-obstacle：在原成功场景楼梯中段加入明确墙体[x17..18,z23.5..24.5,y0..8]，不碰两头ground边。仍build exit0、route=found、verify=verified。
2. ground-through-solid-platform：平地ground边穿过新建实体接地平台[x5..9,z6..10,y0..1]，仍found/verified。
3. ground-overhead-obstacle：路上明确障碍bottom=.5/top=1.5m；因为仅按两端地面y=0过滤障碍而跳过，仍found/verified。现代码没有可验证的人体净空范围。

这是任务要求的障碍/净空/实际结构一致性缺失，不能在limitations里声明后照常验收。修复要让结构体积、行走体积、基础、旧障碍以及新结构在统一范围受检；合法接触和体积穿入分开。缺数据拒绝，禁止自动打洞/消障碍。审核端必须从保存几何/输入重查，不仅重跑同一个漏检builder。

### P1-F2：分能力坡度用端点平均值，局部陡段被错误允许

源码：build_ground_edge约2424–2443用(b.y-a.y)/horizontal传CapabilityRules::for_edge；已计算的maxGrade未用于能力判定。for_edge约1071–1105。

复现wheelchair-local-hump：两端同高，途中上升1m再下降，局部坡度0.2000000179；产物verification.detail.maxGrade自己记录了该值，仍将边标记wheelchair/rail可用，wheelchair查询found且verified。违反模块自述0.05轮椅、0.04轨道规则。原terrace成功包的部分ground边也有同样“maxGrade约.2但授予wheelchair/rail”的潜在错误。

应按实际逐样本/侧带、能力对应宽度/净空审核。没有真正实现某能力的几何判据就明确unsupported，不能凭pedestrian检查给vehicle/rail自动背书。

### P1-F3：缺失环境和不存在的surface/level被当合法输入

源码：EngineeringInput约387–410将protection/obstacles/water/bearing默认空；build约3036–3090主要校验数量、端口坐标和capabilities，未验证引用对象身份；ground端校验高度不等于引用合法。

复现missing-context：删除protection/obstacles/water/bearing字段，仍built/found/verified。任务明确要求“缺失≠显式空集合”。
复现unknown-surfaces：平地两端口分别声称属于nonexistent-deck/floor-99与different-nonexistent-deck/floor-100，实际没有这些结构/楼层，仍ground边连接且verified。

须有权威可解析的surface/level/owner/road端口引用，拒绝悬空/错层/缺关键上下文，不能只把输入标签原样复制到输出再比相等。

### P1-F4：双向路径方向及累计升降/预算不一致

源码：ground边rise_m=(b.y-a.y).abs()约2441；dijkstra约2660–2685用edge.rise_m而不是行进方向的实际升降，rise_at在确认更优cost前即覆盖；单标签也不能完整表达带爬升预算的非支配路径。path_polyline约2857–2868从不按遍历方向反转。

复现：
- reverse-flat：查询b→a，path_node_ids确为反向，但pathPolylineM仍[2,0,8]→[12,0,8]（a→b），verified。
- pedestrian-hump-zero-rise：实际输出路径上升1m/下降1m，maxRise=0，仍found并报告cumulativeRise=0、verified。
- descent-zero-rise：全程从2m下降到0m、maxRise=0，错误budget_exhausted，因为绝对端点差被当成上升2m。

修复必须统一有向折线、逐段升降、最终几何与搜索预算；必要时采用有限非支配状态，或者明确受限求解语义但不能发布超限路径。读回要从保存有向折线独立复核端点、连续性和预算。

### P2-F5：发布不是事务式，CLI失败后留下可verified的成功包

源码：cli engineering_connections.rs build_command约125–194先write_new(bundle)，再写views、最后report。

复现publish-collision：预先存在report路径；CLI退出1且不覆盖旧report，但新bundle和views已发布。随后verify该孤立bundle返回verified。create_new只防覆盖，不能保证整包原子成功。

应完整预检/临时staging/最终commit标识或原子目录发布；失败产物不可被当成功接收，验证器识别不完整发布状态。保留真实错误，别删除旧用户文件。最终时钟门覆盖序列化和发布，不仅计脚本外部耗时。

### P2-F6：矩形范围算术可触发panic，无结构化拒绝

源码RectI::cells约96–101与apply_ground_work约526–527。

复现overflow-scope：x0=i32::MIN,x1=i32::MAX,z0=z1=1，调试CLI退出101，subtract overflow，未写结构化拒绝。发布构建也不能依赖整数回绕后的结果。

使用checked/i64算术，先验证矩形顺序、网格边界、长度/深度/数量、授权值和finite状态，再计量/分配。错误返回受控类别，不panic。其它origin+size/target-current/rise累计应同类检查。

## 验证器判定

修改payloadHash的独立负控制正确得到tamper_detected，此项保留，不误报hash审核失效。但“篡改能抓”与“原始设计合法”是两回事：本次物理/能力/引用/路径错误多数在原始输入构造时即被builder接受，verifier复建同样错误后也通过。两套对比都通过不能替代缺失的约束。

## 已有工作与非阻断纠正

- 17新测试和2access回归已由检查员实际复跑，成立；生产入口、mesh、受控挖填基本实现值得保留，不回退成只做报告。
- 精确编辑前access.rs并未丢失：G13包source-snapshot-access.rs为87390B、SHA256 e62d41bf…，现已与当前文件直接diff（access-exact-baseline.diff）。无需用多1字节的重建版当基线，也无需新增自然/修旧包。
- 回执sourceHashes匹配，但deliverables.manifest只是文件名，没有清单本身的hash/字节绑定。应在修复轮新回执补正确绑定，不倒写旧回执、不循环hash。
- “生产观察”应完整写为“生产API/CLI在人工输入上的运行”，不使读者误以为自然现场验收。
- 仅附加sidecar也不能自动完成世界一致性：未来cut/fill改变的工程面必须由render/physics/access/audit共同消费；“natural不变”不等于可让默认世界继续使用旧工程面。真实消费者集成仍不批准。

## 处置

暂停真实ST18/默认世界消费者接入；按G19R1同范围修复清单整包处理上述问题，重点先封住穿楼/穿墙和假通行。允许小型人工复现与正常源码修复，不新增类型、不自然试跑、不改旧地面阈值；不要求另交“已收到”或历史勘误包。