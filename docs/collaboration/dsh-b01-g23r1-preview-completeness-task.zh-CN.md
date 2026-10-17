# B01-G23R1：预览必需资产集合审核——最小收尾执行任务

日期：2026-09-21。用户已明确要求推进下一步，本文件为执行授权。无需另发确认稿或设计审批稿；只在本包内完成下面一个缺陷的实施和验证。G23原交付继续冻结、保持partial，不倒写历史结论。

## 1. 唯一目标

修复`audit_export`的previewCompleteness：**不能仅验证待审包自己声明的文件，还必须独立要求当前格式规定的必需文件与引用关系。**

已接受的三个场景真实3D消费、最终源码新鲜导出一致性、真实鼠标raycast命中、工程与渲染数值映射均保留，不重做。无需重新跑浏览器矩阵、重新取景或重建楼梯。

固定反例：从合法包删除preview/index.html，并同步删除files[]对应条目；viewerEntry仍为该路径。当前audit四段pass、入口HTTP404；修复后必须previewCompleteness=fail。

## 2. 基线、范围与证据目录

起步只读核对：

- G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/engineering_preview.rs：80655B，SHA256 a64a967a89f58579de79510019e48a33a3ddebc2dff39c2da27e5682a6f13376。
- G:/difyllmwiki/GlyphWeave/bevy/crates/cli/src/engineering_preview.rs：12993B，SHA256 52af51ae39532138cf042142b3b162776b1b73c9e0ec28c68b7306e725609621。
- 检查报告：G:/difyllmwiki/GlyphWeave/docs/collaboration/b01-g23-inspection.zh-CN.md。
- 原反例：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g23-20260921/missing-entry-selfconsistent/；可只读引用或原样复制到本包，不能覆盖。
- 三个已交付正例位于G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g23-engineering-preview-consumer/的preview-straight-v2、preview-switchback、preview-terrace-v2目录。

新目录：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g23r1-preview-completeness/。必须首次创建且所有新产物create_new，旧G23目录及G22两包不写入。发现基线漂移先记录来源，不回滚、不覆盖他人工作。

允许修改core engineering_preview.rs中的格式资产清单/审核/必要测试；可局部提取同职责helper。仅在确需错误传播或定向CLI测试时修改cli engineering_preview.rs，不能顺手改发布事务或服务器。允许新增本包局部测试资源/运行记录器/文档。

冻结：G22三个源码基线、access/auto_roads、楼梯输入和安全阈值、HTML/JS/CSS/viewer/vendor及默认SceneIndex/预览流、现有渲染映射。若需改viewer才能解决，先说明并partial停止，不自批扩范围。

## 3. 格式要求：可信必需集合，不来自待审files数组

### 3.1 一个统一的格式资产规则

对当前受支持schema/version定义期望路径及角色。可从现有PREVIEW_ASSETS与SOURCE_*常量建立统一只读规则，导出和审核共同使用，避免两份列表漂移。当前格式包括：

- source/input.json → source-input；
- source/bundle.json → source-bundle；
- render-model.json → render-model；
- preview/index.html → viewer；
- preview/engineering-preview.js、preview/preview-model.js → viewer-module；
- PREVIEW_ASSETS列出的本地Three/OrbitControls及LICENSE → vendor。

engineering-preview.json作为描述/完成标记单独要求实际存在；避免把它放入自引用文件hash循环。当前清单应有10个payload文件，描述另计，但实现不要只靠数量等于10判通过。

必需集合是受支持预览格式的规则，不是description.files的剩余内容，也不是从待审HTML动态推断的“依赖”。

### 3.2 对每个必需文件验证

1. files[]中恰有一个规范相对路径条目；
2. role与当前格式规则一致；
3. observed/disk中确实存在该文件；
4. 声明bytes/hash与实际字节一致；
5. 不因同时删除文件和声明而免检。

明确拒绝重复路径、重复/冲突身份、错误角色和非法路径形式；无须扩大为新通用资源管理系统。沿用当前安全路径/符号链接限制，不降低已有拒绝。

viewerEntry必须是支持的入口、在必需集合内、与唯一files条目和实际文件相符。renderModel.path/role/bytes/hash等与唯一render-model清单项和实际文件保持一致；source文件固定引用也不能被files遗漏。支持schema和version要相符，未知格式明确拒绝而非按任意清单“自认证”。

额外文件规则沿当前格式：不允许用额外文件替代必需项；若现有schema不允许扩展，明确拒绝额外条目，不悄悄发明扩展合同。

### 3.3 不引入错误的版本限制

本修复要求的是**路径/角色/引用的结构完整性和已声明内容的hash一致性**，不是要求所有合法旧预览的静态字节都与当前二进制PREVIEW_ASSETS逐字节相同。

保持三个已交付合法包可通过。未来资产集合变化需要可识别格式规则；本包无需为此实现迁移系统或默认提高版本。不要为修缺入口变成“凡旧viewer一律拒绝”。

## 4. 四段审核继续独立

缺入口或缺vendor但input/bundle/render-model完整时，预期：

    engineeringVerification = pass
    sourceBinding = pass
    renderParity = pass
    previewCompleteness = fail
    overall = fail

错误明确点名缺失路径、是否缺声明/磁盘/角色/引用，不能仅把总status改fail而四段全pass。缺source本身可同时影响sourceBinding/工程审核，按实际影响报告，不硬凑只有一个失败段。

审核从真实导出目录读取，报告写本包目录外于被审预览根，不能让audit报告自身成为未声明额外文件。

## 5. 最小回归与实际CLI

### 核心定向测试（新增具名、现有保留）

- 三正例的完整集合/角色/引用保持通过；
- 删除入口但保留声明 → fail；
- **删除入口+删除声明 → completeness fail**（固定核心反例）；
- **删除three.core.js+删除声明 → completeness fail**；
- 对每个必需项做参数化“删文件+删声明”测试，缺source/model按真实多段结果判断；
- 重复条目，即使两个bytes/hash相同也拒绝；
- role错误、viewerEntry指错、renderModel引用与条目不符、未知schema/version分别拒绝；
- 现有source损坏、工程包篡改、渲染投影自洽篡改回归保持，不能把完整性缺失误归因工程几何。

测试使用内存导出或唯一临时目录，不写冻结证据。实际调用只用定向engineering_preview/必要CLI测试；G22 32项可作为明确过滤的保护回归，不跑全量会触碰auto_roads冻结路径的测试。

### 最少9次CLI验收

1–3. 当前最终CLI分别audit三个原合法预览目录，报告写本包新路径；均pass。
4. 原“入口与声明同步删除”负例audit：上述四段结果成立。
5. “vendor核心模块与声明同步删除”负例audit：completeness失败并点名文件。
6–7. 从已验证输入及保存bundle全新导出一个代表包，再audit通过，确认共用资产规则没有破坏导出。
8. 重复文件条目负例audit拒绝。
9. 错误角色或引用负例audit拒绝。

最多12次CLI，余3次用于必要失败重试/补充；不要把一条build与一条audit算成一次。不重建工程包，使用原G23已允许重建并验证过的保存bundle作为导出输入，保留来源hash。

HTTP对照可在一个独立驱动中用本地127.0.0.1只读服务做GET：合法入口200、原缺入口副本404；不启动浏览器/WebGL，不复跑点击/截图。若已有检查员404证据直接引用，也可以不再启动服务；必须明确来源。HTTP不能代替audit负例。

## 6. 运行记录与预算（本次新授权明确列出）

历史G23：cargo8/12；CLI24/24；前端17/12超支5。历史G20超支1亦保留。本包不抹除任何超支。

- **G23未用cargo4一次性转入G23R1，新增cargo2；本包总上限6次。** 转入后旧包不再使用该4次。
- **新增CLI12次，上限12。** 原CLI无余额。
- **新增辅助验证驱动4次，上限4**，用于记录器校验、负例制作/汇总及可选HTTP对照；纯hash/读取不算驱动。一个矩阵脚本调用多条CLI时脚本和每条CLI分别按对应类别记账，不重复计成浏览器进程。
- 浏览器/新截图/WebGL验收 **0次**：本包不需要，既有和检查员观察继续引用。
- 工程包build/新几何/地形重合成/自然/ST18/SceneIndex/默认世界/bake/realize/世界发布 **0**。预览导出只读显示转换沿G23允许范围。
- CLI单次≤30s、累计≤300s；归档≤32MiB，三原正例只读引用，派生负例按需复制，不重复存调试二进制或vendor多份无意义副本。

先把成功/预期拒绝的记录器运行可靠，再开始剩余矩阵。每个run立刻create_new写argv/cwd/实际exitCode/stdout/stderr/hash/report路径/耗时，禁止硬编码预期为观测或复用上一条记录。首两个记录器检查可与正式CLI行共用，无需额外跑一轮。

预算预测不足时partial停止，不能又以“验收要求如此”为理由超支。旧ledger15/3与最终17/5的阶段差异在本包总账用一条明确引用解释：**最终历史数以17/12、超5为准**；不改旧ledger，不要求单独制作数字勘误包。

## 7. 交付与停点

先局部实现+具名单测，再最终构建CLI，最后跑限定矩阵。源码变更后所有受影响审核结果绑定最终版本；不在完成标记之后手工刷新旧导出或修改声明。

报告分开列：本轮实测、旧worker观察、检查员已补证；不将检查员新鲜导出/鼠标命中写成本轮重跑。保护G22及原G23整包hash。

新产物定稿后清单→回执→只读核验。G23核心缺口可在本包交检查员收口，但是否验收由检查员决定，不自批里程碑。

正分支：原同步删除反例及vendor同类反例在正确完整性阶段拒绝，合法三包/一新导出通过，四段分工未变，预算与冻结核验成立，提交delivered_for_inspection。

负分支：任一必要门、来源绑定或预算未满足则partial，列唯一剩余阻断并停止，不自动开下一包、不要求用户逐条确认中间步骤。

A冻结、B partial、C不推进。无需确认稿，完成后一并汇报。
