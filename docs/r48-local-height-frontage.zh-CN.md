# r48：局部道路标高驱动的沿路开发

日期：2026-09-07。状态：本轮代码及小图验证完成；最大图生成超时失败，完整目标未完成。

## 动机与边界

r47 自动道路使城市分区获得道路，但没有增加城市建筑。r48 在最终规划路网之后追加显式建设需求，保留旧住宅与设施选址，把新建筑的整地标高绑定到道路纵断面整数毫米 station。道路、地块用途、建筑平台和真实步行入口必须同时成立才算实现。

不更换自然源算法，不引入外部库/资产，不把这一步称为侵蚀或完整城市形态模拟。默认旧 manifest 不开启新程序；不减小尺寸、不减少需求来换取通过。最终必须以真实 bake 和独立重建验收为准。

## 新输入

`style.frontageProgram` 版本 1，包含 requests。每项明确 id、sceneId、purpose、kind、count、widthM/depthM/heightM、stationSpacingM、setbackM 和 maxCandidateAttempts。输入表达类型、精确规模及预算，不接受逐栋坐标或标高。当前支持既有资产体系中的 residential_home / residential_block / storefront；并非已经包含所有文化建筑类型。

- 每项最多 1024 栋，全程序最多 2048 栋、64 项请求。
- 间距 8–128m 的偶数；道路边缘至建筑立面的 setback 为 8–32m 偶数。
- 搜索预算 1–65536 次；每一项保留请求量、接受 ID、拒绝原因及缺额。
- 必须启用 road-first、毫米高程并引用合法场景。
- 按请求顺序，从已有 profile-bearing 道路逐 station、双侧搜索。当前不在旧等高道路上额外开发，不声称全局最优或街区密度优化。

## 合同与重构

新增 `G:\difyllmwiki\GlyphWeave\bevy\crates\core\src\worldgen\frontage.rs`，将需求校验、搜索、局部工程预检和实现审计与道路生长模块分开。

`RoadsideSite` 新增可选 supportLevelMm；旧 supportLevelM 保留兼容。新的 entity 以欧几里得整除产生 worldY，余数写入 bottomOffsetM，负标高不会丢失毫米部分。站点实体构建统一为同一个方法，避免生成器与审计各写一套偏移逻辑。

整个追加操作对错误事务化：先在临时计划上执行，只有完成合法请求处理后才写回。有效请求的预算不足不是异常或成功，保留 unfilled，并在完整 gate 中失败。

候选地块及全支撑采样边界必须完全属于请求用途，而不是只检查中心点；保护区不重标。禁止与已有道路支撑、建筑地块和入口走廊冲突。局部标高取道路 station，不凭空提升为全局平台。检查原始地形无挖方、最大填土与保护区填土衰减。

预检复用现有工程包络与步行检查器，以只读自然毫米栅格构造局部最终高程预测，同时拒绝改变已有硬支撑的候选。预检使用量化自然输入，不能代替最终 bake 从原始源计算的全域独立检查。

新建筑通过原 development/landcover/area-ledger 通路记账；真实 footprint 才算建筑，不能把整个分区或通路记为建筑面积。CLI 增加 frontageAudit；源道路/站点计划仍由原输入独立重放，生成器填写的 report 不作为事实来源。

## 必须核验

1. 小图和 6×10km 原背景同输入对照出现真实新增城市建筑、平台、入口，不只是道路或报告字段。
2. 旧道路/站点与最低住宅/设施需求保留；自然源不变；保护区高度改变样本为零。
3. 正负毫米标高、非零 origin、预算/用途拒绝、重复生成确定性与事务回滚测试。
4. 对实际生成文件篡改平台 1mm、站点/实体或双份报告时，独立审计必须失败。
5. 已实现量与未实现需求分别报告；整图门禁仍保留所有自然/城乡/生态问题。
6. HTML 展示的是最终权威输出，而非独立模拟或隐藏未实现区域的样例。

## 尚未完成

完整文化形态库、密度/容积率驱动街区、外侧边坡/排水/涵洞、真正工程后水文、合法 T 路口拆分与绕障、旧可选对象稳定 ID 和全库开源许可清单仍在总路线图中。新建筑 ID 从请求/道路身份/station/侧别构成，避免自身使用顺序流水号，但并未解决旧生成器全体对象的存量约束。

实测结果、故障修复及最终证据将在本阶段运行完成后追加；本节清单不是通过声明。

## 本轮实际排障与修复记录

### 1. 门口与道路之间的人工凸起

首次真实小图没有实现建筑，7 个住宅候选与 12 个商铺候选缺少可走通路。局部源/最终预测诊断发现，门口与道路之间一个采样处由旧工程过渡叠加抬高约 1.5–5m；两侧硬支撑本身正确。这不是自然岩石，不能放宽步行坡度或删掉自然高程解决。

为新站点/实体添加可选 `frontageRoadPoint`，明确连接道路的中心线点。Rust 创建从入口到该点的 4m 宽硬支撑连接面；优先级高于软过渡、低于道路。自然层不改，不能挖低自然地形，保护/填土限制依旧。连接支撑检查全宽，最终步行路面仍由原访问图与权威最终地形生成，而不是渲染器凭空增加。

新站点的建筑支撑扩展到与原 parcel 合同相同的完整 2m 边缘。不能只做 1m 平台却检查/出售 2m 支撑。后续候选同时保护已有新站点的完整 2m 边缘，避免共享采样边界被邻近工程抬高。新增回归测试证明旧凸起被连接工程接管，但高于设计面的自然样本不被挖低。

### 2. 尺寸合同前置校验

首个输入曾给住宅 24×18m，而样例既有资产合同要求最小深度 24m；scale-audit 正确拒绝。已将示例需求明确改为 24×24m，并在 manifest 校验中读取实际资产合同 min/max，提前拒绝不匹配，不自动缩放对象、不放宽原资产范围。住宅数量保持 6（小图）/120（最大图），商铺数量保持 2/24。

搜索算法还有独立的有限尺寸范围，不是区域建筑法规；通过这一范围并不能绕过 manifest 的资产范围。

### 3. JSON 比较的 f32 升宽假失败

实际 `scene.json` 和 `development-plan.json` 内容相同，但 CLI 把 typed f32 转成 Value 后与原始 JSON 比较，出现不同表示。现在以同一 typed serializer 产生 canonical JSON 后做严格 Value 比较，不增加浮点容差。实际篡改计划和高程仍需拒绝。

### 4. 商铺与长道路的相对位置规则

旧商铺 `relations.require road distance=8` 实际比较两个实体的中心，导致距长路段边缘 8m 的商铺也被拒绝。

- 新增 require 专用 `distance_metric`，默认仍是 `center`，旧描述符保持中心语义。
- 商铺明确选择 `footprint`，按物理 footprint 的轴对齐边界距离计算；配置值仍为 8m。
- 规则索引中的碰撞 clearance 单独保存，并在这一距离计算中扣除，不能把碰撞扩张当作更宽的可达范围。
- 本阶段道路/地块都是基数方向；该距离并不声称为任意旋转多边形的精确距离。

同时将 Footprint 的 yaw 传入入口约束：front/left/back/right 随实体转动，north/west/south/east 保持世界方向。`must_face` 检查向前方向与目标范围，而不是只瞄准长道路中心。距离上限由 require 提供，入口清空半径仍由 clear_radius 提供，不再把二者混在一起。非基数方向的局部入口尚无精确 OBB 合同，当前拒绝而不是误判通过。

回归覆盖道路中段可接近、8m 合法/8.01m 拒绝、碰撞 clearance 不获得距离信用、90° 正向入口与背向拒绝、旧中心距离默认值保持。

### 5. HTML 不得丢失毫米标高

新站点渲染禁止再次吸附地面或忽略 bottomOffsetM。缓存网格只应用一次权威标高偏移和 yaw，保留 sourceEntityId；新增 `view=frontage` 并修复视图白名单，使自动聚焦能够实际进入沿路开发视图。道路 profile 的精确世界轴几何继续走原路径。

## 已完成的小图验证

最终小图目录：`G:\difyllmwiki\GlyphWeave\bevy\work\r48-frontage-small-final`。

- 请求 6 住宅 + 2 商铺；实际实现 **3 住宅 + 2 商铺**，保留 3 住宅缺额。
- 新增建筑 footprint **1,968m²**（3×24×24 + 2×12×10）。总城市建筑面积为 2,112m²，其中还包含原有建筑的城市分区交集，不能全部算作本轮增量。
- 道路 34/34、站点 27/27、原设施 4/4；frontage 5 个已计划对象全部有真实实体与指定道路入口。
- 独立 Node 检查 3,033 个完整支撑样本、285 个入口坡度采样，两份自然/最终高程文件；通过。
- 质量门禁仍 fail，因为请求的 8 栋未全部满足，且原用地实现缺口仍在。不得把只请求 5 栋替代原 8 栋来制造通过。
- 端到端测试包含原件接受、双份同步伪造报告拒绝、单采样 +1mm 拒绝。早期失败日志保存于 r48-frontage-integration-v*，最终通过日志为 `G:\difyllmwiki\GlyphWeave\bevy\work\r48-frontage-integration-v6.log`。

独立检查脚本：`G:\difyllmwiki\GlyphWeave\scripts\check-frontage.mjs`；渲染检查脚本：`G:\difyllmwiki\GlyphWeave\scripts\check-preview-frontage.mjs`。脚本自身不能替代 CLI 从 manifest 重建规划，也不能证明全域路由最优或最终视觉质量。

最大图仍须完成测量和验收后才能下结论；过程中的失败目录不能混作最终证据。

## 源码兼容性与发布范围

本轮修改了 Rust 的 `RoadsideSite`、`GroundingSpec`、`Footprint`、`PlacedKind` 结构体，以及 require 关系的解析类型。旧 JSON 可通过默认值读取，未启用新程序时不会追加新建筑；但下游 Rust 代码直接使用结构体字面量时需要补新字段（或使用适用的 Default），不能将其描述为完全无源码兼容影响。

`frontageRoadPoint` 的新增支持不改变 `.gemap` 的冻结坐标/字节协议；它属于世界语义、工程及实体合同。`distance_metric` 默认 center，商铺描述符显式变更为 footprint；入口朝向修复可能改变旧错误判定，应通过全 workspace 回归而不是只测新模块。

主要改动位置（完整根路径 `G:\difyllmwiki\GlyphWeave`）：

- Rust 规划与精度：`bevy/crates/core/src/worldgen/frontage.rs`、`road_network.rs`、`services.rs`、`access.rs`、`engineered_placement.rs` 及 `bevy/crates/core/src/worldgen.rs`。
- 规则语义：`bevy/crates/core/src/rules/schema.rs`、`constraint.rs`、`validator.rs`、`placement.rs`、`audit.rs`；商铺规则 `assets/objects/storefront.object.toml`。
- CLI 与回归：`bevy/crates/cli/src/main.rs`、`bevy/crates/cli/tests/baked_audit.rs`；settlement 测试 fixture 新字段适配，不减少断言。
- HTML：`adapters/html/app.js` 的沿路视图、权威标高与 source ID。
- 正向独立验收：`scripts/check-frontage.mjs` 与 `scripts/check-preview-frontage.mjs`。
- 用户输入：`examples/local-height-frontage.manifest.json`、`examples/maximum-local-height-frontage.manifest.json`。

没有新增依赖或复制外部算法/素材；本阶段许可证声明只覆盖新增代码来源，不替代全项目发布前的许可证与 NOTICE 清单审查。

新建筑身份仍依赖上游道路 ID 的稳定性：如果自动路网自身被重选或重新编号，跨次生成的身份保持仍需后续存量/迁移合同，不能据本轮的 station ID 宣称全项目增量编辑已解决。

## 最终最大图试验：明确失败，不是完整交付

输入：`G:\difyllmwiki\GlyphWeave\examples\maximum-local-height-frontage.manifest.json`。保留 r47 的 6000×10000m、seed=7、自然背景、比例、自动道路预算及旧设施/住宅要求，新增 120 个 24×24m 住宅与 24 个 12×10m 商铺的需求。

**600.0878072 秒触及原 600 秒实验预算，测量程序已终止这个特定进程。**采样峰值私有内存为 616,390,656 B，峰值工作集为 589,115,392 B；这是被中断进程的观测值，不是完整生成峰值或产品 SLA。

关键阶段证据：

| 阶段 | 本轮 |
|---|---:|
| 道路及设施/沿路开发规划 | 69.486s 开始，77.627s 结束 |
| 实体生成与预检 | 77.675s 开始，81.900s 结束 |
| 最终高程与语义 | 81.900s 开始，231.553s 结束 |
| 地表实现 | 240.307s 开始，522.072s 结束，耗时 **281.765s** |
| 地表独立重建 | 522.072s 开始，在 600.088s 被预算中止，完整耗时未知 |

r47 地表实现为 104.514s，本轮这一阶段显著变慢；不能把瓶颈一概归因于自动选路或自然高度场。下一步应继续细分地块/院落裁剪、农用通道与生态实现/验证的成本，使用精确空间查询、缓存及同输入等价对照优化，不能取消独立重建或减少实体/田块/生态数量。

目录 `G:\difyllmwiki\GlyphWeave\bevy\work\r48-frontage-maximum-final` 的名字虽然含 final，但**实际是不完整失败目录**：仅保存中间 240 个自然和 240 个最终高程块等文件；没有 world.json，没有 generation-report.json，没有完整发布场景，不能作为最大图 HTML 成果。没有对它执行完整 world-quality-gate，也不报告不存在的最大图新增建筑数量或面积。

证据：

- `G:\difyllmwiki\GlyphWeave\bevy\work\r48-maximum-generation-measurement\process.json`
- `G:\difyllmwiki\GlyphWeave\bevy\work\r48-maximum-generation-measurement\stderr.log`
- `G:\difyllmwiki\GlyphWeave\bevy\work\r48-maximum-generation-measurement\disk.json`
- `G:\difyllmwiki\GlyphWeave\bevy\work\r48-maximum-incomplete.json`

保留失败原件，不覆盖/删除，不把重新提高时间预算后的运行包装为 600 秒通过。最大图性能、完整人文环境及自然地貌的总目标仍未完成。

## 回归与可查看的 HTML

Rust 完整 workspace **440 passed**：core 363、app 44、CLI 7、integration 26；1 个既有微基准 ignored。Node **43 passed**、Vitest **170 passed**；lint、typecheck、doc-tree 检查通过（保留既有 lint 警告）。

已在浏览器核验当前小图 5 个新增建筑：实际网格 footprint/高度、毫米标高、门朝向、源 ID 与 Rust 数据一致，没有请求整份 scene.json，没有页面错误。发现商铺曾被较早的旧模板分支截获，已调整为新规划建筑优先进入精确 envelope 渲染路径，而不是只添加缺失的显示标记。新视图及此分支以最终浏览器检查验证。

截图：`G:\difyllmwiki\GlyphWeave\bevy\work\r48-frontage-small-preview.png`。人工查看确认能看到沿路住宅、商铺和门前连接面；建筑仍是简化资产，植被与地表视觉尚粗糙。这是 512×512m 小图的功能验证，不是最大图验收或整个现代社会地图完成。

HTML 仅同步最新 adapter；不改权威场景、高程和用地内容。测量使用的旧 runtime 副本保留作二进制身份证据；后续另行构建的 verified runtime 包含最终视图白名单与商铺分支修复，不混淆两个二进制的 SHA256 或将新构建耗时计入原测量。

最终测试日志：`G:\difyllmwiki\GlyphWeave\bevy\work\r48-workspace-tests.log`；前端状态：`G:\difyllmwiki\GlyphWeave\bevy\work\r48-js-checks.json`；几何/入口独立证据：`G:\difyllmwiki\GlyphWeave\bevy\work\r48-small-independent-frontage.json` 与 `G:\difyllmwiki\GlyphWeave\bevy\work\r48-frontage-small-preview.png.json`。
