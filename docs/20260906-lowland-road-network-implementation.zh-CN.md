# 自然地形驱动的低地路网与临路住宅：r16 实施及验收

- 日期：2026-09-06
- 权威工程：`G:\difyllmwiki\GlyphWeave`；未修改 `F:\modenmapbox`。
- 核心版本：`worldgen-2026-09-06-r16-roadside-frontage`。
- 状态：**阶段性实现，整体 roadmap 未完成，不能发布为完整现代社会地图生成器。**
- 本文接续 `20260906-planning-partition-v2-implementation.zh-CN.md`。旧实验产物没有删除；下列 r16 目录才是本次结果。

> 后续实现状态见 `20260906-road-first-development-ownership.zh-CN.md`：临路住宅已经接入新地块/建筑计划；本文 r16 的旧地块未实现数据保留为历史，不代表 r18 当前状态。

## 1. 解决的实际问题

此前 512 米用例虽然有规划地块，却没有实际道路。道路候选先按理想位置生成，再进行贴地及工程调整，导致设计高度失效。更隐蔽的问题是普通 `green_space` 被当成建筑平台：小块绿地也会产生数百米的填土过渡带，抬高远处道路。只过滤建筑与路面的平面碰撞，无法发现这种工程影响。

本轮改成：

```text
Rust 生成并保存自然 metre raster
  → 在完整自然 raster 上找可施工的连通低地路网
  → 为路网选择共同支撑标高
  → 沿实际道路筛选临路住宅，确定朝向和入口
  → 预留路床、住宅地基及其施工影响
  → 其他候选进入原有规则系统
  → 工程高度场与实体落地
  → 从自然/最终 raster 和最终实体独立复算验收
```

Agent 没有接管道路、地形、碰撞或审计计算。自然层没有被最终房屋反向覆写。

## 2. 实现位置及职责

| 路径（相对权威工程） | 本轮职责 |
|---|---|
| `bevy/crates/core/src/worldgen/road_network.rs` | 新模块：道路节点、边、临路选址、自然采样、连通分量选择、工程影响预检查、独立审计 |
| `bevy/crates/core/src/worldgen.rs` | 在自然 raster 生成后调用规划；预留空间；接入设计标高；普通绿地不再生成建筑平台；输出审计 |
| `bevy/crates/cli/src/main.rs` | 读取实际文件独立复算；strict/release 必要计划缺失或验收失败不能通过 gate |
| `bevy/crates/cli/tests/baked_audit.rs` | 实际 bake→CLI 一致性与篡改测试；明确区分通路校验提前退出与贴地规则失败 |
| `adapters/html/app.js` | 静态相机设置 target 后，在注册 change 回调前执行一次 update，修复天空空画面 |
| `scripts/check-preview-camera.mjs` | 独立浏览器烟雾检查：脚本异常、首帧、相机朝向断言及截图 |

不是重写整个 worldgen；旧地块实现与新路网之间仍存在必须解决的接口缺口。

## 3. 道路算法合同与边界

模型名称：`connected-level-lowland-fill-frontage-v2`。

1. 64 米候选网格，8 米道路宽度，场景边缘留出安全余量。
2. 对每条候选道路完整采样路面、路肩及端点范围，不用九个探针代替完整 raster。
3. 排除水体相交及超过最多 6 米填土能力的候选。
4. 枚举可行整数支撑标高；只保留在同一标高可实现的最大连通分量，同规模时优先更接近场景中心的分量。
5. 保存自然高度场完整 hash、候选数量、因水体/起伏拒绝数量及未选择数量。
6. 路网边具有稳定来源 ID，生成后的道路变换必须与计划一致。
7. 当前是**填方为主、共同标高的低地路网近似**，不是山区公路线形设计、桥梁工程、真实城市交通优化或水力模拟。
8. 自动启用限于 strict/release、存在 landUseProfile、非 naturalOnly、非 wilderness/pastoral 且无结构化水体的场景；其他场景没有被宣称已迁移。

保留原有 8% 道路限制。未通过增加道路容许坡度或删除必要道路换取绿灯。

### 3.1 临路住宅

- 道路两侧尝试 24×24 米住宅，住宅中心离道路中心线 24 米。
- 28×28 米自然采样范围必须满足同标高、填土上限、水体与其他道路/住宅间距约束。
- 保存来源 road ID、标高、自然最低/最高高度、住宅朝向。
- 前门位于朝向道路的一侧。基于基数方向使用整数坐标生成入口，避免大世界坐标先转 f32 再舍入。
- 在 i64 中检查边界后才转回 i32；极端合法场景 origin 有回归测试。
- 这仍然是 RoadNetworkPlan 的临路住宅，不是已经完成的通用 BuildingPlan，也未完成旧 ParcelPlan 的实现关联。

### 3.2 工程相互影响

普通绿地是地表覆盖，不能默认具有建筑填土平台。道路和选定住宅先预留；其他建筑和停车场的工程过渡范围若会抬高它们，拒绝该候选。

预检查不是最终保证：后续规则移动、其他工程处理或 authored 内容仍可能改变地形。因此最终审计必须保留。发现失败时写入失败报告，不伪造成功，也不移除冲突事实。

## 4. 独立审计

`road-network-plan.json` 必须与 scene 内计划一致；CLI 从完整自然 raster 重建计划，并检查实际最终数据。保存的 `road-network-audit.json` 中即使写着 pass，也不能替代重算。

### 道路

- 唯一来源 ID，kind/asset/尺寸/角度/scale/标高与计划一致。
- 完整路床采样检查实际填方、切方及路面设计高度误差。
- 容差 0.26 米与四分之一米高度量化相配合，不意味着工程水力精度。
- 空可行路网不会成为 pass。

### 临路住宅

- 计划住宅缺失、重复、移动、改尺寸/旋转/标高或入口均失败。
- 核验住宅地基全采样范围及实际填方，不只检查中心点。
- 报告 `plannedSites`、`realizedSites`、`maxFinalSiteFillM`、`maxSiteDesignHeightErrorM`。
- 通路可步行性仍由独立 access graph/实际 pavement 校验，不由住宅计数推断。

### 测试修复说明

原来的 sunken fixture 把第一个实体改成深埋房屋。道路生成变化后，这个实体可能是参与人行通路的道路，导致 CLI 在通路一致性校验处退出。测试随后读取上一次 `cli-audit.json`，错误地认为贴地检查没有发现问题。

修复保留原始篡改必须失败的断言；随后仅重建派生 pavement 来隔离贴地规则测试，不更改坏实体或地形，删除旧报告并确认新报告确实生成，最后仍要求 `floating_items > 0`。生产审计顺序与规则没有放宽。

## 5. 实际结果（固定 seed=20260906）

输入为 temperate-plain、strict/rules，urbanCoreRatio=0.12、suburbanRatio=0.144；完整其他参数见输入 manifest，比例由既有 profile 归一化处理。

| 指标 | 512×512 米 | 1000×1000 米 |
|---|---:|---:|
| 最终实体 | 16 | 48 |
| 道路计划/实现 | 5/5 | 10/10 |
| 临路住宅计划/实现 | 1/1 | 9/9 |
| 必需且连通的建筑通路 | 1 | 9 |
| 道路最大填土 | 5.75 米 | 5.50 米 |
| 住宅采样范围最大填土 | 5.25 米 | 5.50 米 |
| 道路/住宅设计高度误差 | 0/0 米 | 0/0 米 |
| 道路及临路住宅专项审计 | pass | pass |
| 旧 ParcelPlan 未实现数量 | 16 | 49 |
| 实体面积指标 assignedM2 | 33367.16 | 157739.72 |
| 完整 world-quality-gate | **fail** | **fail** |

1000 米场景中旧朝向导致部分入口绕房约 47 米；临路朝向接入后，9 条通路均约 7.3 米，连接到各自分配的道路。这里是固定种子的实测，不是所有种子的保证。

完整 gate 的两类失败保持真实：用地实现不足、规划地块未实现。没有把新住宅强行认领到不相干旧地块，也没有把空白地表当成达成面积。

### 5.1 可复查产物

权威工程下：

- `bevy/work/road-network-r16-512/`
- `bevy/work/road-network-r16-1000/`
- 输入：`bevy/work/road-network-r15-512.manifest.json`、`bevy/work/road-network-r15-1000.manifest.json`（输入沿用 r15 文件名，生成核心为 r16）。
- 每个目录包含真实高度场、自然高度场、scene、road plan/audit、rules audit、quality report、world-quality-gate。
- `bevy/work/road-network-r16-tests.log`：Core 260、App 44、CLI 6、跨 crate 实际 bake 集成 6，全通过；共 316 个测试。
- `pnpm doc-tree:check` 通过。
- `pnpm typecheck` 通过；`pnpm test` 为 22 个文件、170 个测试全部通过，日志在 `bevy/work/road-network-r16-web-tests.log`。
- lint 退出码 0，但存在未用变量等警告；不可写成零警告。

本轮没有运行 6×10km 完整 bake 或峰值内存基准，不能把小图结果当成最大地图性能验收。

## 6. HTML 验证：发现问题而非只读取 ready 标志

第一次 HTTP=200、JS 无异常、`nearSceneRendered=true`，实际截图却只有天空。原因是 OrbitControls 构造时朝向初始 target，随后设置新 target，但静态模式没有动画帧调用 update。

修复为设置完参数后执行一次 update；必须在 change listener 之前，不能放回 renderNearFrame 造成递归。运行时测试断言 camera 朝向与 target 方向点积 >0.9999，本轮 street/skyline 实测均为 1。

预览已在本地端口 8830 启动，运行的是 work 目录的可执行文件副本，不锁定 Cargo 输出。入口：`http://127.0.0.1:8830/preview/?view=skyline`。

截图与旁边 JSON 报告：

- `bevy/work/road-network-r16-1000/preview-skyline-fixed.png`
- `bevy/work/road-network-r16-1000/preview-street-fixed.png`

人工核验看到 terrain、农地、住宅及部分路面，不再只有天空。**这不等于视觉验收通过**：当前材质过亮、植被与路面关系仍有问题，street 默认只加载一个 chunk，默认路灯焦点也不一定展示住宅路网；skyline 更适合查看这次完整小场景。尚不能声称每条路在渲染上都已正确可见。adapter 文件同步到了两份当前预览，未修改 Rust bake 几何或审计数据。

## 7. 下一步必须继续，而不是宣布完成

### P0：统一规划与实际用地

1. 让 Settlement/District/Parcel 基于已实现道路 frontage 切分，而非另一套独立轴线保留带。道路影响到地块拆分，地块需求再反馈候选道路，但必须设置有限迭代和确定性终止条件。
2. 临路住宅升级到真正的 BuildingPlan：parcelId、roadId、footprint、yard、entrance、施工范围、拒绝原因。保留旧来源核验，不能用“最近地块”伪造认领。
3. 用地分母和实体 footprint 面积分开；农田、庭院、公共绿地、道路等使用互斥 landcover/parcel 几何求面积。禁止巨型 building 或 green_space box 充数。
4. 拒绝日志从规划候选到最终 placement 保留稳定 ID 和真实阶段原因。最终缺失只能写缺失，不能猜测 slope/water/collision。
5. InfrastructurePlan 与新 RoadNetworkPlan 统一来源及 realization，避免两份可互相矛盾的计划。

### P1：自然地貌和工程边界

1. 保留自然/工程双层 raster 和独立 hash；拆分自然地形 provider 与工程算子。
2. 开始地貌核心候选的固定版本对照与流域/山脊/谷地基准。当前自然层仍是既有实现，本轮没有接入 Mapgen4 或 symbios-ground，没有实现真实水力侵蚀/沉积。
3. 路线允许受约束纵坡和多片区标高，再引入切方、挡土、排水与桥涵。共同标高只适合当前低地最小验证场景。
4. 保存完整施工影响和土方预算；6 米填方上限不是成本可接受性或排水安全证明。

### P2：最终验收

- 城市/乡村/荒野与中美欧日港形态规则接到同一土地/路网合同，不止比例和装饰实体。
- 多 seed、跨 chunk、含水和崎岖地貌回归；最大 6×10km release bake 耗时、峰值内存及产物大小实测。
- 修复 HTML 可视实体/道路/地表之间的差异后，截图与 Rust 几何互相核对。
- 通过 strict 完整 gate 后再谈交付；本次未到该阶段。

## 8. 来源与开源许可证边界

本轮新增路网/临路选址/审计是项目自有代码与工程近似；没有复制 Mapgen4、symbios-ground 或其他外部项目代码，没有增加 Cargo/npm 依赖或新资产。现有 Playwright 和已安装 Edge 仅用于本地验证。

本轮不作新的外部许可证兼容结论，也不把既有第三方资产改标为项目许可证。未来任何外部实现必须先固定仓库/commit、核验对应 LICENSE/NOTICE 和依赖树，再决定集成；继续维护 `THIRD_PARTY_NOTICES.md`。自然核心或视觉资产的全量发布许可证审核仍属发布前必做项。
