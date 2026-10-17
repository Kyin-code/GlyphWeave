# B01-G27B 首轮独立检查（2026-09-23）

## 1. 裁定

**G27B保持partial：版本化模型、两条CLI及测试有实质进展；“恢复只剩5次浏览器观察”不成立。** 存在来源信任链、3D包审核覆盖和可见几何/拾取一致性缺口。开工授权继续有效，不回到hold；先修限定缺口，再执行浏览器矩阵，不推翻路径A。

本次绑定：context_preview3d.rs 59634B / be4f2e1ff356eeb88b82e657237c0520fcdaf9130de44df1a7615b5a4b205c54；main.rs 135649B / d92e2fd318f787f1a8da9e1834268bf08412d5ef787182c8689297c72fc1d351。

## 2. 独立证据

- 本包递归清单85/85，0未列文件，receipt六项绑定成立。最终代码hash匹配。
- G27A/R1、G26/R1/R2、G25/R1、G24 continuation-4、G23/R1、G22两包，12组旧清单逐条重算匹配；不据此声称任何未纳入清单文件也被完整覆盖。
- 检查员运行cargo test -p glyphweave-cli --offline --bin glyphweave：57/57；另cargo build -p glyphweave-cli --offline通过。该57为整个CLI过滤结果，不代表57项都为3D新增断言。
- 当前源码二进制独立CLI共11次：正例export/audit、伪报告链export/audit、伪造上游高程export/audit、损坏脚本/来源/声明/内嵌状态审核、natural单字段篡改对照。原始结果见cli-review.json。
- 另1次纯JS几何函数探针，未创建浏览器、未运行WebGL；不声称着色器、GPU或真实鼠标已实测。
- worker cargo5/8、CLI9/16、browser0/5、aux0/6暂按自报；首轮cargo41构建没有单独cargo-41日志文件，仅command-log组合命令记录。缺日志不能由检查员构建倒补为worker原始记录。

## 3. P1：把报告当信任凭证，而不是从来源独立重查

位置：context_preview3d.rs load_inputs 91–147、audit_3d 475–504。

load_inputs只要求外部JSON的totalPass=true和modelSha256等于当前v1模型；不校验报告四段、schema或执行原来源重建。audit又调用同一个load_inputs并从这份可篡改v1生成期望模型。

独立结果：
- 03/04：仅两个字段的JSON `{totalPass:true, modelSha256:<当前v1哈希>}`，没有任何stage或来源证据，export成功、audit四段pass。
- 05/06：检查员复制v1，修改final.samples[0]增加1,000,000毫米，同步v1 manifest/marker，伪造上述两字段报告；未动权威世界或G24。export成功、audit仍四段pass。

结论：复制报告hash不提供真实性，攻击者可同时改数据和报告。不能用“审核曾经通过”代替当前调用对显式来源的复算；两个方向共享错误假设并不构成独立验证。上一层G25有来源复算能力，应复用，不再创造一条只信报告的旁路。

修复：显式world/context/preview来源输入，调用已有的受限读取和纯重建链，将v1逐字段与来源重建值比较，再生成3D；audit独立重读后执行同样判据。传入旧报告可作hash留痕，但不得决定放行。单纯给伪报告补schema和4条pass不解决信任问题。

## 4. P1：3D包完整性和全部字段仍未被审核

位置：audit_3d 395–519。

独立反例均 **exit0、totalPass=true、四段pass**：
- 07：viewer3d.js改成非空的 `throw new Error(...)`，其余文件原样；没有静态资产hash绑定，审计仍通过。
- 08：3D sources.sceneId/declaredSceneId改成invented-scene、sources.manifest改为伪对象；同步model/manifest/marker及内嵌模型。来源审核只比较world/scene/context与两个hash，漏掉其余sources。
- 09：manifest.schemaVersion改v999、modelFile改不存在路径、sceneId改错、requiredFiles追加重复viewer和数字；同步marker哈希后通过。filter_map+dedup消除了非法类型/重复的证据。
- 10：只改HTML内嵌模型的schemaVersion、sources.sceneId、status.entityCount=999，而render-model-3d.json不变；只比五组几何字段，故仍通过。

对照11：修改natural.heightsMm且完整同步内嵌与hash，明确仅terrainParity失败；说明并非所有检查都失效，而是覆盖不完整。

静态补充：viewer read_confined Err被if let Ok忽略；README只要求名字存在，未读取校验；manifest/marker版本/引用没有完整合同；heightDisplayConversion字段和部分来源字段没有语义归属。未知3D版本虽总拒绝，仍进行了部分旧schema比较，不能声称其余阶段未评估。

修复应一次定义格式必需集合、字段类型/唯一性/未知字段、模型/HTML/静态资源字节绑定及全部字段→阶段映射。导出与audit共用规则，不只补这四个坐标化反例。静态资源hash证明文件完整性，不把任意自洽JS宣称为行为安全认证；当前产品发行的viewer应明确版本与构建绑定。

## 5. P1：实际渲染没有落实已声明的世界几何

### 5.1 未知高度被补成固定常数

源码render：access path Y=0.6，道路线Y=0.7，节点Y=0.8。这些值不存在于保存事实，违反“不补高度、不贴合”的正式批示。模型写heightStatus=no saved height并不能豁免renderer使用造出的高度。

道路实际画节点间线段，不是保存长度/旋转的矩形带。节点height未知应明确未定位于3D，不能标referenceOnly后放到一个虚构世界高度。

### 5.2 实体只画AABB线框，旋转仅存元数据

build_model以savedBounds直接组成bounds3d；renderer的rectangleWire不读取rotation/scale/本地尺寸，只画GL_LINES；拾取也是该世界AABB。它不等于有向实体体块。90°且源bounds恰好匹配的固定正例遮蔽45°旋转等情况；本次不宣称已跑旋转浏览器反例。

原批示“savedBounds＋rotation”确有歧义：savedBounds是世界AABB时不可再整体旋转一次，也不能替代源有向尺寸。该歧义由检查员在续作合同明确修正，而非让DSH猜测后继续等待。

### 5.3 地形拾取并非画出的三角面

代码使用固定0.5m步进+nearest sample(round)决定命中，不与渲染索引三角求交。

检查员从实际viewer抽出surfaceHeightAt，输入2×2顶点毫米[0,1000,0,1000]，画出的网格平面为y=x。z=0.5时x=0.25/0.49返回y=0而不是0.25/0.49；x=0.51/0.75返回1而不是0.51/0.75。见geometry-review.json。此为JS函数差分，不是WebGL鼠标观察。

“禁止插值”指不生成新的权威高度样本，不禁止既定三角形内部的栅格化或求交。应以实际绘制三角形作ray intersection，命中位置是派生几何，不伪装保存毫米采样值；保留面索引/源样本索引。

## 6. 不再重复的工作

保留57项绿色与版本化实现、原失败日志和CLI原始运行。路径A/G22/G23/G24/G25已接受范围不回退，不重跑无关世界，不另建入口外壳。只在G27B修复来源复算、全部包合同、几何/拾取三块，再做真实WebGL2观察。

不得再次以“只剩浏览器”描述当前状态；但也不因问题回到等待产品需求批准。用户2026-09-23开工授权仍有效。

## 7. 下一执行包

见 G:/difyllmwiki/GlyphWeave/docs/collaboration/dsh-b01-g27b-source-geometry-browser-closure-task.zh-CN.md。明确合同澄清、最小模块白名单、剩余额度内的执行顺序及预算不足停点，不分拆成逐个确认轮。

检查员成本：cargo2、生产CLI11、CLI矩阵驱动1、纯JS几何探针1、hash读取另列；browser0、世界/工程build0。所有反例仅修改检查员新目录副本，worker源码和固定世界未动。

证据根：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g27b-20260923/。

**G27B partial；A冻结、B partial、C不推进。**
