# G-1b step3b 独立验收（2026-09-19）

## 裁定：accepted-synthetic-diagnostic-tool

上轮唯一阻断解除，G-1b诊断工具在本任务规定的合成验收范围内通过，工具阶段关闭。不是自然工程通过，不构成首个真实增量；A不变，B partial，C不推进。不要求再作一轮裁定确认文档。

## 独立复验

- 清单9/9、receipt6/6匹配，payload为日志原始字节子串。
- step1e/step2b旧receipt绑定的非源码证据全部保持，auto_roads源码绑定仅因授权后续编辑取代。
- 生产前缀81685字节，SHA256 ba6deb959c3e3b913ed2c65a887f4860b1cddf505cfd2ae2fb71758997224d34保持。
- 检查员一次no-run编译（0.26s），11项命名回归+1项总验收分别--exact运行，12/12通过。自然/冻结世界carve回放/realize/bake均0，源码未改。
- 原四条after记录中blendApplied只包含环内项；旧完全覆盖高台在coverApplied。overrideLoweredValue=true而blendErasedByOverride=false、erased为空。新增ring-only反例则指名真实环填方来源并满足subset与几何条件。
- 对照生产代码，抬升记录已经分别写在full-cover和ring分支内；擦除来源只取实际抬升且高于最终值的环内项，不再由循环值降低直接推断blend来源。
- HEAD fb36dc4f，staged为空。证据：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g1b-step3b-20260919/。

## 下一步建议（不是本轮已执行动作）

建议另行批准固定G-1b一次局部自然诊断。明确授权文本见本文件下节。用户转发为任务后worker方可执行；本次检查没有自然放行调用，也未代执行。

### 可转发的单次局部诊断任务

工具合成验收接受并关闭，旧结论/台账不重开。不再要求先回一篇确认文档。批准范围仅限以下固定问题，不能转为真实增量搜索：

1. **输入固定**：沿用G8/G9的ST18-R1，A=(288,96)@8000、B=(288,180)@3110、C=(176,180)@-382，12m、junctionLength9、shoulder3、rise80、fill6000；所有冻结输入一致。G-1b北/南connection为cx270、cz168/192、half_w1、half_d5、blend2、priority2、target2390。不新增B/C、坐标、宽度、mask或carve语义。
2. **基线及候选**：一次重建冻结基线，按既有生产叶子复原AB/BC并核对保存profile（85/113点）。保留原有建筑/义务实体；“无候选基线”只排除本次两栋候选，不是删除全部建筑。北/南用station18同一请求（固定homes），分别从同一staged道路基线追加，不能累积。不得把合成fixture的8000mm人工旧高台注入自然基线。
3. **唯一修改**：生产生成候选pad/apron/connection并验证身份后，只替换connection为G-1b；非connection全字段不变。用显式base和base+proposed列表，生产合成及frontage同口径夹限求值。自然值必须从真实栅格读取并传播缺格错误，不能仅检查scene边界或硬填一个f64。禁止借用candidate_final_terrain重建旧连接验证新规则。
4. **范围**：北只测(270,171)、(268,171)，南只测(270,188)、(268,188)，共4个唯一坐标。每点保存真实natural、raw before/after、夹限before/after、rawChanged/supportSampleChanged、zone/protection、几何outside/full、全部相关carve身份与顺序、priority/profile及来源列表。baseline hash/profile/身份一致性检查与预算计数同包输出。不扩到整片保护区或路线。
5. **预算**：自然harness最多1次、冻结计划重建1次、AB/BC候选阶段design各一次/append各一次（总design≤2、append≤2；准备阶段另列），不infer、不扫参数；候选2、唯一坐标4、整次自然诊断墙钟≤90s。measure/access寻路/candidate_final_terrain/baseline realize/candidate realize/bake/全场scan均0。生产叶子的内部调用未计量就标not_instrumented，不能宣称0。
6. **实施与运行**：可在auto_roads.rs的cfg(test)增加最小ignored诊断入口，生产文件/默认值/审核不动；先完成编译和必要人工fixture检查，精确合成允许修复重跑。自然入口必须create_new预约先行，记录冻结规则表与执行源码hash；不使用旧证据目录覆盖文件。
7. **停止规则**：输入/hash/profile/身份不一致、缺采样、序列化失败或超时，输出incomplete/错误并停止，不自行重跑。预期的supportSampleChanged=true是正常诊断结果，不是执行异常：记录并在预算内完成另一候选，然后结束；不得改参数重试。
8. **结果解释**：raw变化不等于支撑变化，支撑判定只用夹限后整数毫米。任何已测保护格变化即可否定对应候选的保持性；四点全部相等也仅为局部未观察到变化，不能宣称完整support/pad/access通过，更不能发witness。不得直接推进C-1或全局不可行结论。
9. **证据**：全新轮次目录，规则/预约与源码版本先冻结；运行一次后产物完成→清单→回执→只读验证。所有内部外部计数口径分开，保留失败产物；旧证据不倒写。提交结果后停止，B partial/C不推进。

若无法在上述白名单内完成必要调用，不得绕过权限扩大生产接口，先指出具体阻断；不得以“自然额度未给”反复请求本任务已经明确批准的同一范围。