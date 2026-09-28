# G31最终独立裁定：展示可读性限定验收，历史和文档例外保留

日期：2026-09-27（北京时间）。**G31 = accepted_scoped_with_process_exceptions。采用冻结修订包 `g31-r1-entity-layering-1` 作为本保存场景的推荐展示基线。** 不再开G31补测轮；G30-R4原推荐版本保留可对照，不覆盖旧包。

## 1. 采用范围与边界

接受固定场景窗口、已记录相机/两面/视口/交互状态下的展示参数改善及必要防退化，不宣称所有场景、设备、色彩空间或真实城市普遍改善；不是工程安全/路线复核/默认世界或工程B完成。

推荐入口：`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g31-r1-entity-layering/cli-runs/c01-export/preview3d.html`。

- viewer：265f42c6ef9f8a881489ba35657ad4c043dc9d8cc5850c38156111530e1e8dfd
- model：922d404cdbd413e567d5ab89094d69ef75f785ab73e469c5676a79dc025f23ff
- HTML：44364c4a54907aa894f355b9023403bf4f5e85e211bff2b8924bf0a56dac926c
- Rust：7e4a72b00e2391075697101b89ca2e0eb3df00e31e504edba35c1a9d2a9afed3
- exe：c09ff0c87a85ff6cc4b5f0ca3e762c5df965c54aa3f66fd6b70a02d389871044

A冻结，B总体partial，C不推进。G31收口只是展示消费者的限定收口。

## 2. 本轮独立验证

检查员目录：`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g31-final-20260927/`。

本轮没有cargo/产品CLI/浏览器/worker驱动执行；只读hash/原始记录、实际查看两张修订俯视图，以及独立@napi-rs/canvas解码保存截图进行统计复算。

### 2.1 C3/C2补证成立

六张现场截图和六份先于截图记录的元数据全部匹配原始browser-record。四张baseline/G31图与原G31-a03图SHA完全相同，支持原场景一致性，不是仅凭文件名匹配。

| 场景 | 基线terrainMid | 修订terrainMid | 独立比值 | 暗份额 | 裁切份额 |
| --- | ---: | ---: | ---: | ---: | ---: |
| final俯视全开 | 0.35945960784313724 | 0.3499286274509803 | **0.9734852535** | 0→0 | 0→0 |
| natural俯视地形单显 | 0.5191639215686275 | 0.5403874509803922 | **1.0408802086** | 0→0 | 0→0 |

按原ROI、原编码空间加权亮度算法和原阈值，C3中位≥基线×.95、暗占比不增加；C2裁切≤基线且≤.5%，两组均通过。用户摘要/receipt中的final比值0.973314是手填算术错误；机器记录c3.ratio与独立复算均为0.9734852535，不影响结论。

我查看修订final与natural俯视图：无压黑或过曝、结构可辨认。统计量相同不代表所有坡面像素不变，前轮已否定“朝上系数精确不变”的泛化推论，此处不复活该说法。

### 2.2 最小运行事实

a01为full，14结果/11required均pass、exit0。其中7条preflight，其余为场景/运行检查；不是14个独立浏览器会话。现场6角色状态均WebGL2/GL错误0、索引23814/三角7938/顶点4096；全开8022/solid7、单显7938/solid0，面与图层符合原定义。

a02为rearchive、browserAllowance0，8结果/7required、exit0。两次aux与一次浏览器符合本轮明确额度；没有本轮新增超支证据。

## 3. 如何继承之前的证据

- 修订C4：前轮独立实体几何遮罩1.1050554825×，pixel-set SHA相同；本轮只引用，不再测。
- C1 final：前轮对修订现场图独立后验1.1001121477×，仍是边际通过；natural在R1同会话记录成立。
- C3与俯视相关C2：本轮新现场与独立复算补齐。
- 其它场景/选择/图层/布局、防退化证据依G31/R1原始记录和两项参数白名单变更承接；不能写成所有C1–C7在同一次完整矩阵重新通过。
- 最终model/HTML不变、viewer与当前source字节一致、c01/c03六文件确定性与四段audit按此前绑定保留。完整cargo运行异常的历史缺失日志不被这些证据“补回”。

## 4. 归档：原R2仍有一项漏列，独立附加绑定完成现存范围的检查闭合

R2 manifest声明仅排除顶层manifest/receipt，实际绑定new36+R1引用82=118条，全部现存列项hash匹配。**但report.zh-CN.md事后创建且未列入manifest，是新增1项漏列。** receipt的postManifestDocumentation只是披露，不是扩展排除授权，不能据此称原闭集完整。

“重新绑定需要第三次aux，额度已用完”的解释不充分：任务§4明确哈希/只读枚举/文档管理记管理日志，只有执行统计/夹具/浏览器驱动计aux。完成report后做纯归档校验不必运行第三次full。将管理脚本与验证运行耦合，是执行者应避免的流程设计问题。

为避免再让用户往返一个纯文书任务，检查员新增 `adopted-evidence.json`，只读绑定**R2现存39文件（含report、manifest、receipt）+R1现存82文件，合计121**，两个scope零排除。检查员自身manifest再绑定该清单。本次独立采用范围的现存文件全部有明确hash，旧worker文件未改。

这不追溯修正旧manifest，也不宣称缺失的build-archive.cjs或cargo原始日志恢复。其缺失历史与R1aux+2违规继续作为例外。搜索4份宿主记录未找到原脚本，只能称“在已搜索来源未恢复”，不能证明所有可能备份均不存在。

## 5. 报告生成的重复错误应停止

R2 command-log现写“aux花1、a02是唯一执行且启动浏览器”，并紧接browserAllowance0；与真实intent/results和budget矛盾。正确时间线为：a01 full一次Chrome，a02 rearchive无Chrome，共aux2/browser1。独立裁定以原始run记录为准，错误command-log不回写，只在本报告纠正。

未发现足以判断主观“不积极/偷懒”的证据：本轮真实补齐了现场观察，也按规定没有再次启动浏览器。但**文档晚写漏列、把管理工作错误当作额度阻断、重复模板生成错误日志、手填比例算错**均为明确交付自查不足。披露不是修复，文件数绿灯也不能代替对实际目录的最后枚举。

不再为这些发G31-R3补证。下轮要求从单一事实台账生成报告/预算/日志，先写完报告再封存；后续交接只能写新目录。

## 6. 下一任务

G31限定验收已收口。授权G32“相机平移与导航恢复”，只改善用户在固定保存窗口里的浏览操作，停止继续微调同一组颜色阈值。新增操作不修改保存模型/几何/光照/拾取求交，仅改变相机目标；与原旋转/缩放/点击选择清晰区分。

任务：`G:/difyllmwiki/GlyphWeave/docs/collaboration/dsh-b01-g32-camera-pan-task.zh-CN.md`。G32独立预算，不继承或抵扣G31/G30旧余额及超支；当前未执行。
