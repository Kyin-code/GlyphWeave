# B01-G27B continuation：来源重查、3D包合同与几何/拾取收口

日期：2026-09-23。

依据：
- 首轮独立检查：G:/difyllmwiki/GlyphWeave/docs/collaboration/b01-g27b-initial-inspection.zh-CN.md
- G27B交付：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g27b-context-preview-3d/
- 已生效开工批示：G:/difyllmwiki/GlyphWeave/docs/collaboration/dsh-b01-g27b-approval-task.zh-CN.md

## 0. 裁定与预算

G27B已经获批并已开始，不回到hold；但首轮为partial。当前源码/测试/CLI有实质进展，**不能继续宣称“只剩5次浏览器观察”**。本续作先修三个生产链缺口，再执行浏览器矩阵。

首轮累计：cargo5/8、CLI9/16、browser0/5、aux0/6。续作只使用原G27B剩余额度：

- cargo最多 **3**；
- CLI最多 **7**；
- browser/WebGL最多 **5**；
- aux最多 **6**；
- 不增加、不复用历史额度；归档仍≤64 MiB。

若余额不足，准确partial停点，保留失败测试和原始记录，不超支。

历史包/证据一律不改；G22/G23/G24核心算法、G23旧viewer及路径A行为继续冻结。产品修改以context_preview3d.rs和必要注册为主。为复用G25纯来源重建/模型映射，允许context_preview.rs及engineering_context.rs最小pub(super)可见性接线；如需要抽出局部只读helper，保持原命令语义并运行保护回归，不另写弱化实现。除此之外的改动先报告具体必要性，不借机重构。

## 1. P1：导出与审核必须独立重建来源

当前`load_inputs`把外部G25 audit report的`totalPass=true`和model hash当作放行凭据；攻击者可改v1 model、manifest/marker和伪造两字段报告后继续生成并审核3D包。

### 必须修复

- 3D导出和audit都接收调用者显式的`--world`与G24 `--context`包，加既有`--preview`。原报告可保留作可选留痕，不是放行条件；固定来源根和context包沿G25来源绑定追溯，不由报告自行选择隐式路径。
- 从该来源重新构建G24 context，并逐字段比对传入的G25 v1 model/context；重建失败或不一致，禁止生成/通过3D包。
- G25 audit report可以作为来源留痕和上下文，但`totalPass`不能是唯一信任根；报告内容必须满足完整四段合同，或者由同一受限重建函数重新产生。
- 保留来源hash、context audit hash和调用者输入路径；未知/缺失来源拒绝。
- 为伪报告构造具名反例：仅`{totalPass:true, modelSha256:<当前哈希>}`；以及改final高度+同步v1 hash+伪报告。两者都不得export成功。
- 必须新增正例：真实G25包、真实来源根和真实报告能通过。

禁止用“给伪报告补四个pass字段”代替来源重查；禁止在3D模块内复制一套弱化的G24 parser。

## 2. P1：3D包完整性必须覆盖全部文件、全部声明和页面内模型

当前audit对viewer3d.js只检查非空，漏掉脚本改坏；sources只核对部分字段，manifest字段类型/重复/固定引用覆盖不足；HTML内嵌模型只比较部分几何字段。

### 必须修复

建立一个导出/audit共用的独立格式规则：

- 六个必需文件恰好一次、全部为普通文件、无额外文件、路径受限；
- manifest/marker schema、modelFile/manifestFile、modelSha256/manifestSha256、requiredFiles类型/唯一性/固定集合全部校验；未知schema/version整包拒绝且其他阶段notEvaluated；
- viewer3d.js、preview3d.html、README.md也必须有bytes/sha256声明并逐项匹配，不能只判断脚本非空；
- render model闭集字段、类型、来源字段、coordinateSystem、heightFactUnit、window、surfaces、entities、roads、nodes、access、status、unknownKinds、schemaDrift、limitations全部纳入指定阶段；未知字段、缺字段、重复声明拒绝；
- HTML内嵌模型必须与render-model-3d.json全模型一致，不只比较surfaces/entities/roads/nodes/access五组；
- 页面资源hash和完成标记必须形成独立链，包内自洽但资源内容被改也要被拒；
- 完整性错误必须进入previewCompleteness；来源重建错误进入sourceBinding；不要依赖诊断字符串关键词聚合。

具名反例必须保留：损坏viewer脚本、伪造sources.sceneId/declaredSceneId、未知manifest版本/错误modelFile/重复requiredFiles、只改HTML内嵌模型status/schema/source、删必需文件/额外文件。

## 3. P1：实体、道路和access几何必须兑现批准合同

### 3.1 缺高度不得偷偷造世界Y

当前viewer把无高度道路/节点/access固定画在Y=0.7/0.8/0.6。批准合同是“按保存世界坐标、不贴合；无高度显式显示无高度/未贴合”。

修复：

- 不把常数Y当保存事实；无高度要么以不占用世界三维位置的明确overlay/render-only层显示，要么不绘制该3D段但在HUD/表格显示未贴合；
- 模型中保留`heightStatus`/`snapping:none`，viewer不要把render-only高度写回world事实；
- roads必须按保存长度、宽度、旋转表达为矩形带/明确的保存几何，不能只连两个节点直线；缺节点坐标时保留referenceOnly，不凭空放节点。

### 3.2 实体旋转语义不能只存不用

批准合同需明确区分：`savedBounds`是保存的世界AABB还是局部有向几何。续作采用以下无歧义规则：

- 当前G25实体具有worldXYZ、widthM/depthM/heightM、rotation及grounding时，按源变换与pivot约定生成有向体块三角几何，savedBounds作为原始世界包围事实保留，不重复旋转AABB，也不重复施加已烘焙scale；
- 若其他输入确实只有savedBounds，则可明确标记为sourceAabbProxy而不是精确有向体块；没有竖向事实时仅未定位overlay或明确不支持，不补Y=0。实体拾取须与实际渲染体块一致。
- audit逐字段核对表示和来源；新增45°/非方形fixture单测，证明不会把AABB重复旋转或把rotation丢掉。

### 3.3 地形拾取必须与实际三角几何一致

当前`surfaceHeightAt`用nearest sample+0.5m步进，和已绘制三角不一致。

修复：

- 以实际绘制的网格三角形做ray-triangle求交，命中点是派生显示几何；
- 不生成新的权威高度样本，不改保存毫米事实；
- 记录surface、triangle/cell索引和命中世界坐标；
- entity体块优先，随后三角表面；无命中返回空；
- 新增非平面2×2/至少一个跨面斜率fixture单测，不能只测平面或最近样本。

## 4. 浏览器矩阵：修复后才运行，不先消耗5次

修复并通过核心/CLI测试后，使用剩余最多5次真实WebGL2浏览器运行：

1. clean package：WebGL2 context成功、shader/program成功、模型加载、natural/final切换、像素多样性、HUD事实；
2. reload：至少三次重载，计数/模型hash/层状态稳定无重复；
3. entity click：真实`page.mouse.click`，返回批准字段；
4. terrain click：真实鼠标命中实际三角表面，返回surface/triangle/cell/height派生坐标；
5. broken package：坏包不显示旧模型、错误状态明确、视口为空。

若选择file://自包含页面可不启动CLI服务；如使用静态服务，明确计入实际启动预算。浏览器launch最多5次，不是要求凑够5次；可在同一有效运行内完成多个独立验收场景。每次都保存实际console/page errors、WebGL renderer/version、model hash、截图和真实pickMethod。任何page.evaluate失败只能标未观测，不得赋期望默认值再记pass。不能用列表选择替代鼠标命中，不能把2D G27A观察当3D证据。

## 5. 调用与交付顺序

建议剩余调用分配：

- cargo1：最终CLI具名单测（来源伪报告、完整性、几何/拾取）；
- cargo2：core engineering_保护回归（不跑会写历史目录的auto_roads全套）；
- cargo3：最终CLI构建；
- 以上每条为独立调用，不把test+build合并计成一次；若失败需要复测占用额度，应先重排，不能跳过必需验证后宣称完成。
- CLI最低按7次安排：clean export+audit（2）、伪报告/改源负例export拒绝（1）、坏脚本/声明负例audit（1）、natural与实体各一次指定parity拒绝（2）、第二次同源export确定性（1）；其余字段变体用参数化具名测试覆盖。如启动CLI服务，需从7次额度中统筹，不能额外隐形消耗；
- 之后才启动5次浏览器；浏览器前置入口进程计入CLI余额，不能另称非CLI。

每次运行立即记录argv/cwd/exit/stdout/stderr/report hash和耗时。旧首轮失败日志保留，新目录使用：

`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g27b-context-preview-3d/continuation-1/`

最终顺序：源码/单测 → 构建 → CLI矩阵 → 5次WebGL2 → report/progress → manifest → receipt → 只读核验。manifest生成后不再追加绑定文件。

正分支只能声明“G27B真实3D/WebGL上下文消费者交检查”，不能声明工程安全、真实城市、ST18、默认世界或全城可达性。

**A冻结、Bpartial、C不推进。**
