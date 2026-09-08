# 可直接转交 DS v4.1f：B-01 独立临路地块与真实庭院

状态：首批可执行任务。只执行 B-01，不执行整个B；完成后停，交主 Agent 验收。

## 0. 目标（一句话）

让 Rust 生成的地块不再始终是建筑外接框各扩2米，而能根据显式前院/侧院/后院需求产生独立的parcel边界，并通过现有地表执行器落成真实庭院/通道与可独立复算的面积账本。

这是一项生成业务能力，不是再增加一层验收平台，也不是重新做frontage系统。

## 1. 开工基线与必读（只读这些，不重扫全部历史）

仓库：G:/difyllmwiki/GlyphWeave。F:/modenmapbox 不改。
SOURCE_BASE：42cb4973fe6fdf985d21ee2a1f4be3c0ce10382b；tag：milestone-a-accepted。

开工记录实际 WORK_BASE=`git rev-parse HEAD`。WORK_BASE可为SOURCE_BASE的仅文档后继；先确认SOURCE_BASE是其祖先，且中间只修改README/文档。代码基线若有其他未授权变化，报告，不checkout/reset掩盖。当前工作分支由主 Agent 已建立，不自行新建/切换或拉取覆盖dirty。

必读：
- G:/difyllmwiki/GlyphWeave/AGENTS.md
- G:/difyllmwiki/GlyphWeave/docs/collaboration/milestone-b-plan.zh-CN.md
- G:/difyllmwiki/GlyphWeave/docs/collaboration/milestone-b-acceptance.zh-CN.md
- G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/worldgen/frontage.rs
- G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/worldgen/road_network.rs
- G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/settlement/development.rs
- G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/settlement/landcover.rs
- G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/settlement/landuse_ledger.rs

只读现有小场景 examples/local-height-frontage.manifest.json 和现有frontage/development测试，复用其毫米支撑/独立审核方式。不联网重新搜城市，不下载资产/库，不读模型会话日志。

## 2. 功能合同

### 2.1 加法式输入，不重复发明Program

复用现有 `style.frontageProgram.requests[]`，建议增加可选字段：

```json
"parcelInsets": { "frontM": 4, "sideM": 4, "rearM": 12 }
```

- 这是B-01拟实现的字段，A版本尚不支持，不能在改代码前声称已可用。
- 均为非负偶数米；frontM/sideM上限32m，rearM上限64m；显式配置计算后地块两边都不超过128m（不回溯收紧legacy旧输入），不接受溢出/NaN/隐式截断。
- 缺省字段精确保留旧2m外扩语义；用serde default/skip_serializing_if保留旧输入解析与缺省输出形状，不能让全部旧场景突然扩大院落。
- 显式 frontM 不得超过该请求 setbackM，且实际parcel仍须完全避开道路所有权/禁止区域；不能为了腾地方改变既有setback含义、移动楼体或缩小尺寸。
- 门前方向来自既有朝向/入口合同，而不是固定世界北方。12×10建筑、front4/side4/rear12时，局部parcel为20×26m，面积520m²，footprint120m²，非建筑余地400m²；前后不对称必须体现为边界中心偏移。
- front=side=rear=2的显式输入与旧隐式边界等价；zero margin不导致退化footprint，接触边界不等于正面积重叠。仍遵守原入口、支撑与场地安全限制。

### 2.2 唯一权威几何链

输入请求 → Rust候选地块 → accepted RoadsideSite/来源metadata → DevelopmentPlan/FrontageParcel → LandCell Building/Yard分区 → SurfaceRealization → independent audit/land-use ledger。

可在RoadsideSite添加带来源的可选地块边界/参数，优先复用已有development::Rect或小型明确的同单位类型，不造第二套互不相认的坐标结构。具体最小字段方案先在工作结果中说明，然后按本合同实现，不需等待额外确认才能开始白名单工作。

- 不允许Agent/HTML事后制造parcel坐标。
- site/building的宽深高、yaw、稳定ID、固定门、road point、毫米支撑不因parcel面积扩大而改变。
- 选址/预留必须用完整地块检查与其他地块的占地冲突，不能等房屋都摆好后才发现院子重叠。
- 既有无显式地块的site也必须以旧2m边界参加冲突检查，不只检查楼的bbox。
- 新地块应完全位于目标urban/rural用途允许范围；允许跨同用途zone，精确记录每个zone的source_zones面积。不得占用nature/保护地/越界/不允许水域；不能重标zone以让候选通过。
- building覆盖与yard必须无重叠无空洞地覆盖parcel；source_zones之和等于parcel面积；场景土地仍守恒。
- 独立重建必须从原请求/已验证道路site/几何推导边界，而不是相信发布的area_m2、parcel_boundary或ledger状态。更改源派生结构但与冻结输入不符仍拒绝。

### 2.3 真正落地，不只扩大计划框

现有landcover::realize应消费新的Yard分区，产出与最终地形、入口通行面一致的ManagedGrass/AccessPavement等已有SurfacePatch。不新增材质，不改renderer。

只允许将实际通过独立核验的几何计入realized。仅增加FrontageParcel.boundary、把plannedArea写大、修改状态为pass，不算完成。不得把整个parcel都计为building，不重复计算pavement和grass；未实现地面要明确拒绝/缺额。

扩大地块不是扩大建筑平台的许可：不修改自然源，不整片填平院子，不突破原fill/坡度/保护区限制。不能落地的院落要诚实失败，门/楼仍不能自动挪位兜底。

## 3. 写入白名单

允许生产改动：
1. G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/worldgen/frontage.rs：解析/验证可选需求、完整parcel预留和选址、来源与测试。
2. G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/worldgen/road_network.rs：最小site metadata与来源验证，不改道路增长算法、道路预算和既有工程语义。
3. G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/settlement/development.rs：消费权威parcel、精确分区、独立重算和测试。
4. G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/settlement/landcover.rs：仅新Yard边界接入所必需的修正/测试，优先保持原执行器；不重写农业/生态。
5. G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/settlement/landuse_ledger.rs：仅parcel面积对应与防重复计量的必要测试/修正，禁止放宽判据。
6. 可新增 G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/worldgen/parcel_geometry.rs 放纯几何，避免往大文件堆算法。
7. G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/worldgen.rs：仅新增模块声明、明确generator revision更新及必要接线；不改变整体阶段顺序。

仅机械适配例外：若RoadsideSite新增Option字段，允许在 G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/worldgen/services.rs 和 access.rs 的现有构造/测试补缺省None；不得修改设施选址、访问算法、正负毫米判据。

允许新增：
- G:/difyllmwiki/GlyphWeave/bevy/crates/cli/tests/parcel_program.rs：端到端正负例。
- G:/difyllmwiki/GlyphWeave/examples/b01-parcel-program.manifest.json：新的512m示例，不覆盖A manifest。
- G:/difyllmwiki/GlyphWeave/docs/collaboration/b01-result.zh-CN.md：设计/重构/兼容和结果。
- G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01/：自己的全新测试输入/输出/回执。

其他全部只读：原A世界/runtime/采样/观察/签收/归档、所有现有checker、HTML、自然地形算法、auto_roads、Cargo/package/lock、资产与许可证。不得改CLI gate以放过新parcel失败。若真正需要白名单外的生产接口修改，报告最小diff建议并停止该部分，不顺手扩大。

## 4. 测试与成功标准（开工前冻结）

### 核心正例

- 旧请求无parcelInsets与legacy旧语义一致；显式2/2/2产生同边界。
- 非对称前4/侧4/后12的尺寸/中心/面积精确，四yaw分别正确。
- 同用途跨zone，source_zones分摊精确；负origin/非零origin、zone边界、互相贴边均覆盖。
- 同seed同输入重复确定；增加远处独立请求不重排原accepted实体ID/固定入口。
- 构造可行的512m平地控制布局，冻结16栋12×10×8住宅与8栋12×10×6店铺，setback8、stationSpacing24、parcelInsets4/4/12；预算maxCandidateAttempts每请求2048。24个新增请求均合法实现，逐个有parcel、building、固定入口/道路连接、yard surface与来源链。
- 可行控制布局必须在原允许地形/区域合同内构造，不修改自然核心。若现有CLI的低地输入不能表达该控制条件，先报告缺失接口，不反复换seed直到碰巧全绿；纯函数正例仍继续完成。
- 新地块需求的庭院分母只含这些accepted parcels的非建筑余地，面积从几何复算；把场景其他缺额排除时必须明确scope，不能冒称整个512m都完成。

### 核心负例

地块互相重叠但房屋不碰；地块越界；前后院颠倒；跨入nature/保护zone；超出setback；水体/固体阻断院落或门到路；缺Source/不相容场景；整数溢出/非法尺寸；重复parcel/source；篡改parcel边界或source_zones；把surface删掉仍写realized；只篡改面积；同步伪造派生parcel/ledger却与原输入重建不一致。

每项要求给明确失败/缺额，不降count/楼宽深高、setback或用途目标，不改判据成宽松容差。纯逻辑负例不反复生成世界。

### 局部端到端

使用新512m示例完成Rust产物→独立audit→yard/ledger核对；保留24个新增目标与已有最低需求，不删除原site换成功。new生成重复一次，比较权威布局/来源/面积稳定；实现revision等身份变化单独声明。

验收不是截图。HTML无需改动，能够消费原SurfacePatch即可。最多一次可选的人工局部查看，不启动240块巡检。

## 5. 执行预算与命令

先实现纯几何/小fixture的红—绿，再集成。建议给新增测试统一 `parcel_program` 名称前缀：

- cargo test --manifest-path bevy/Cargo.toml -p glyphweave-core parcel_program -- --test-threads=1
- cargo test --manifest-path bevy/Cargo.toml -p glyphweave-core worldgen::frontage -- --test-threads=1
- cargo test --manifest-path bevy/Cargo.toml -p glyphweave-core settlement::development -- --test-threads=1
- cargo test --manifest-path bevy/Cargo.toml -p glyphweave-cli --test parcel_program -- --test-threads=1

复用当前依赖和缓存；定向编译/测试单命令300s，首次确有编译需要可用一次900s上限，记录编译与测试耗时，不能无限等待。限一个Rust重任务，不与bake并发。禁止完整workspace和release重建；CLI集成测试所需debug二进制构建允许，不能复用A exe声称验证了B代码。

本任务最多3次512m生成（旧配置对照1次、新配置2次）及对应最多3次独立gate，每次生成/gate120s、4096MiB private；控制在自己的新输出目录。场景输出identity变化先解释再复用，不照搬R2失败后自动再跑大图的做法。进程测量工具复用现有设施，不新建证据平台。

Node源码没改无需全套重跑；tree/diff检查必做：corepack pnpm doc-tree:check、git diff --check。不执行全仓格式化；只格式化自己实际改动，不能造成无关几千行重排。

禁止最大图、1km组合大测、linear基准、浏览器全域测试、外部模型、联网安装依赖/资产、Git写操作。运行不足/失败如实返回，不提高预算或删目标。最多两轮同因修复，第三次前给最小复现和决策。

## 6. 交付与停止

交付 G:/difyllmwiki/GlyphWeave/docs/collaboration/b01-result.zh-CN.md 和 ds-b01/receipt-b01.json：
- 实际HEAD/源码增量hash、修改文件、兼容/版本策略；
- 输入请求→site→parcel→surface→ledger字段与函数来源图；
- 每条成功标准对应测试/产物、实际命令/退出码/日志；
- 旧2m边界与新20×26m控制地块的几何/实现面积对照；
- 24目标的requested/planned/realizedWithAccess/yardRealized/unfilled，不混计划与实际；
- 自然源/保护/已存在实体的保持证据；
- 全场仍有何质量失败及本次局部scope；预算、失败尝试、未验证项、后台句柄。

状态只写 ready_for_review，不写B完成，不重新改写A结论。未做项标not_run/unknown，不造hash或费用。无需复制A全证据链。完成后停止，等主 Agent复验后再派B-02；**不得 git add/commit/push/reset/clean/checkout/switch/stash/merge/rebase，不得启动其他agent。**
