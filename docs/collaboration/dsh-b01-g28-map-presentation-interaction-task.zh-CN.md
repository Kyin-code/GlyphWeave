# DSH B01-G28：地图呈现与交互体验提升——正式执行任务

日期：2026-09-23。**已批准开工，原草案转为生效任务。立即执行，不再等签批；本文件替代原“待R1后启动”措辞。**

## 0. 独立裁定与目标

G27B-R1闭集独立核验通过，G27B按只读保存场景真实3D/WebGL消费者限定范围收口；两个已知UI问题明确转入G28。裁定：G:/difyllmwiki/GlyphWeave/docs/collaboration/b01-g27b-r1-final-verdict.zh-CN.md。

G28目标：让同一份真实场景“选择结果看得到、信息读得懂、视角操作顺手、地形与道路更易辨认”，而不是继续只有内部状态正确。此次必须交付实际产品变化、可复现操作和人工画面评审，不只出计划或调参说明。

既有正确性不能回退：world/context/v1独立重建、来源链、闭集格式、实际几何、natural/final独立、真实索引三角拾取、无高度不补Y、access永为观测而非通行证明。总体A冻结/B partial/C不推进。

## 1. 独立预算（正式生效）

| 类别 | G28上限 | 当前已用 |
| --- | ---: | ---: |
| cargo test/build等调用 | 8 | 0 |
| 产品CLI/服务入口进程 | 8 | 0 |
| 浏览器运行含失败启动/冒烟 | 6 | 0 |
| 辅助驱动执行 | 6 | 0 |
| G28归档 | 64 MiB | 0 |

不复用、不重置G27B账。失败调用、失败浏览器启动和辅助进程逐项计数，预先保存完整stdout/stderr和退出码，禁止只留下最终绿日志。尽量把浏览器场景合并在同一运行，用页面/context隔离，不把每个断言开成一次进程。

沙箱/权限遵守当前执行环境：优先使用已支持的浏览器工具；需要运行许可时按工具真实政策处理。历史spawn EPERM不证明本次必然失败，不机械地先失败一次再重试；没有提权接口/政策不允许时不能绕过。产品授权不等于绕过环境权限；遇到具体工具阻断如实记录，不能说“每次固定需要批准”而不检查当前能力。

建议cargo安排：实现迭代4次、最终CLI测试1次、core工程保护1次、最终build1次、修复预留1次；不是必须用满。编译消费者不算禁止的工程build。地图生成、ST18、默认世界、EngineeringInput、route/bake/realize/世界发布全部0。

## 2. 写入范围与基准

主要允许修改：
- G:/difyllmwiki/GlyphWeave/bevy/crates/cli/src/context_preview3d.rs（viewer/HTML/HUD/展示辅助与相关测试）；
- 必要局部展示helper/JS/CSS/测试模块，可合理拆分，不大范围重构；
- G:/difyllmwiki/GlyphWeave/bevy/crates/cli/src/main.rs只允许必要help接线，不改旧命令行为。

不改core G24来源/裁剪/地形事实、G22安全算法、G23 viewer和路径A；不另写弱化来源parser，不为美观修改实体位置/尺寸/高度或重新求路。不派子代理、不Git写入。

确认不存在后新建：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g28-map-presentation-interaction/。所有G27B及更早包/证据冻结。G28可以修改现有3D消费者源码，这是本任务明确写入许可，不是允许重写已交付G27B包。构建可能更新target中的exe，因此事先记录G27B最终exe/hash；新证据绑定新exe，旧receipt不改。

固定world：G:/difyllmwiki/GlyphWeave/bevy/work/joint-r27-verified-baseline/。
固定context：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g24-saved-scene-context/continuation-4/runs/c01-export-clean/package。
固定v1：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g25-context-preview-consumer/runs/p16-final-package/package。
前版3D包：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g27b-context-preview-3d/continuation-3/packages/clean-3d/。
窗口256,32,64,64，不换场景凑效果。

优先仅改变显示层，让render-model的保存事实和几何继续一致；计数由已绑定模型重算用于HUD，不静默改变源事实。如果确需修改派生状态字段，列清差异并更新audit/tests，不能声称模型逐字节未变。

## 3. 实施内容：连续完成，不分段等审批

### 3.1 先修用户可见错误

- 真实鼠标实体命中后，高亮、state和Selection面板同一entityId；显示entityId/kind/assetId/worldXYZ/rotation/scale/savedBounds。
- 明确“保存字段”和“派生显示命中点/三角形”两组。地形命中展示surface、triangle、源样本ID与派生坐标，不称新坐标为保存高度。
- 点击真正没有可拾取对象的背景或按“清除选择”，面板/state/高亮一起清除；隐藏已选对象所属层时选择状态不得悬空。点击地形不是无对象背景，语义要明确。
- HUD分别显示nodes=4、access=18、observation-only=22及拆分来源；从模型计算，不能硬编码这些数。
- 无高度、未定位、未贴合分项定义清楚，按entity/road/node/access分类；“没有高度”“没有3D primitive”“没有执行贴合”不是同一概念，不能用一个总数混写。避免道路在entity与road集合中重复出现时把分类记录总和声称独立对象总数。
- 无高度nodes/access仍明确观测记录；可折叠列表/元数据查看，不在3D空间发明高度。
- 核对并纠正本消费者的路径/模式标签：G27B/G28是独立真实3D消费者，不应沿用会误解为G27A canvas2D的“path-A”标签；只读、无工程/路线证明保留。
- 所有来源文本用安全文本插入，不以innerHTML拼接未转义entityId/limitations等输入。

### 3.2 相机、信息层级与可用性

实现整体斜视、俯视、聚焦选中实体、旋转、缩放、恢复默认；聚焦以实际几何边界取中心/距离，不移动模型。相机控制DOM区域不触发地图拾取，拖拽不误作点击，输入控件焦点不被相机快捷键劫持。

HUD可折叠/滚动，默认只显示关键事实；来源哈希、drift和limitations完整可达，不删除、不假装已解决。1280×800和较窄960×640视口不裁掉关键控制，面板不占满地图。按钮有文字或可访问名称，不用纯颜色表达状态。

项目已有UI规则适用于其运行载体：React壳使用已有shadcn/Lucide；当前独立自包含HTML不为一个按钮引入React打包/CDN，可用带语义的原生控件、文本或已许可内联图标，不用emoji装饰；维持离线包资源完整。

### 3.3 一次克制的视觉改善

在同一几何上改善地形、道路、实体的材质区分、面法线与光照，建立可读的坡面和体块明暗。背景/天空色、距离雾可仅作展示设置，不生成虚构地形、水体、植被或建筑。

- natural/final都保持正确来源，不通过缩放高度制造起伏；
- 实体不再只是难读的统一大色块，选择高亮与正常材质清楚区分；
- 不以强泛光、景深、雾遮挡穿插/缺口；不填洞、不贴地；
- 可提供“检查/展示”两个显示预设，但共用同一几何、来源和拾取，切换只改渲染参数/标签，不分叉两个世界；
- 不引入水面、粒子、昼夜或角色动画来替代本批目标；不抄第三方游戏代码/素材。整体布局优化与地图生成美学不是本任务实现范围。

## 4. 版本/证据策略

viewer/html改动后更新producer build标识，明确是新产物。模型schema不因只改CSS/光照而无理由升级；若改字段语义则需明确版本和兼容策略。

旧G27B包按旧构建指纹受限是可披露版本行为，不能悄悄用当前二进制要求旧包通过新生产者指纹。旧证据并未因为新exe改变而历史失效，只是不能证明G28新版通过；正确性回归按改动影响分析安排，不必因纯CSS重跑所有旧矩阵。

最终产品修改完成→测试→最终build→新包CLI/browser。若最终验收后再改产品，重做受影响证据并绑定新hash；不要拿旧截图配新代码。

## 5. 验收矩阵（一次交付）

### 单测/保护

具名覆盖选择数据与安全输出、计数分项（含空集合和缺高度）、相机聚焦/重置、场景与高度事实未改变、producer资产变化拒绝，以及既有3D正确性回归。最终CLI全套与core engineering_运行；不能用只匹配源码字符串代替交互。

### CLI：6项必需，另2项预留

1. 最终exe从显式来源导出G28新包；
2. 新包audit四段pass；
3. 同步普通声明后的替换viewer被producer规则拒绝；
4. 同步声明后的仅HTML内嵌来源/状态不同被拒绝；
5. 自洽来源身份篡改由sourceBinding拒绝；
6. 负例后的干净包audit四段pass。

余2项可用于重试、必要服务或二次确定性导出。不要硬开服务挤掉必需用例；支持file自包含时优先按实际环境运行。按具体失败阶段与诊断断言，不只is_err。静态样式若触碰几何/字段逻辑必须在单测补对应正负例。

### 浏览器：≤6次运行合并以下场景

E01 真实page.mouse实体与地形命中；面板可见文字、state、高亮身份一致；清除/背景点击/隐藏层一致更新；不以helper选择代替。

E02 final/natural来源正确；nodes/access/观测和无高度分项与模型逐一核对；drift/limitations可完整查看；窄视口不遮关键控制。

E03 整体/俯视/聚焦/拖拽/缩放/重置，以实际用户操作到达；重复三轮检查对象数不变、reset可复现。未验证路线不得提供“已安全漫游”声明。

E04 视觉评审：在同设备/同视口下实际打开冻结旧包仅作before参考，再打开最终新包；整体、俯视、近景至少三张after截图，并对可匹配的固定相机做before/after比较。若旧包无相应预设，记录实际camera参数或标记不成对，不能把不同构图冒充同机位改进。截图必须实际查看并逐条评价坡面、道路识别、体块比例、遮挡、文字可读性；记录不足，人工评价不伪装机器pass。

E05 真实WebGL2、shader/GL错误、加载耗时、控制响应和帧间隔（采样条件与设备/分辨率披露），旧/新同条件比较；三次重载稳定、坏包真实拒绝+当时截图+干净恢复。不能只用页面FPS自报或像素多样性证明好用；若有明显性能退化先定位，不以没有预设绝对FPS门槛掩盖卡顿。

第一次可做计入额度的早期冒烟；最终观察绑定最终包。浏览器读取异常记notObserved/失败，不填0/false判绿；驱动及产品失败原始日志留存，不只保存最后覆盖版。已有会话可合并上述操作，UI截图与隐藏state都要检验，不能重演“state有选择而面板空白仍通过”。

## 6. 交付与运行纪律

新目录包含完整代码/包/binary绑定、实际命令日志、逐进程record、单测与CLI原文、浏览器actual/截图/性能、变化对照表、report/progress/manifest/receipt/只读核验。可执行文件过大时绑定磁盘hash而不复制进归档；从最终构建日志和导出记录可追溯。

manifest采用scope+精确相对路径，不按basename排除；所有嵌套包manifest/receipt正常列入，只有本轮自绑定文件可按明确规则排除。先定稿报告/日志，再生成清单/receipt；旧G27B/R1不回写。保持原缺失失败日志与旧G24 stale为历史披露，不伪造恢复。

执行顺序：只读基线与预算→选择/HUD→相机和显示改善→具名测试→最终build→CLI/browser体验→一次交付。无需每段再申请确认，不另派子代理。

若真实容量中断保存未封存工作checkpoint和下一具体动作，不用旧绿测试冒充本轮进展。预算耗尽或具体外部阻断才停止等待；不把原授权误写hold。

**本任务已正式生效。DSH现在启动G28，不再停在R1交付点。完成后交delivered_for_inspection，体验是否验收由独立检查与用户看图/操作评价决定。**
