# 2026-09-06：规划分区 v2、实体来源与独立验收

## 状态与适用范围

本次为实质性修复，不是最终完成交付。权威目录 `G:\difyllmwiki\GlyphWeave`；未修改 `F:\modenmapbox`。
生成器 revision 为 `worldgen-2026-09-06-r13-partition-provenance`，ParcelPlan / InfrastructurePlan 的规划 schema 为 v2。
本记录优先于主路线图后半部实验日志中与实际代码不符的完成描述。

## 发现的根因（不是参数微调）

1. **大型地块相互重叠。** 原来的第二列偏移虽然不越出 envelope，但小于一个 parcel 的宽度。旧测试只测了包含关系，没有调用真正的规划校验。新增 240×240 宏区测试首先得到 `parcels macro.parcel.0 and macro.parcel.1 overlap`，证明该缺陷。
2. **边缘碎片改变用地比例。** 512m 场景使用 160m 网格后，最后一排/列只有 32m；按 cell 数量分配时，自然区大多落入小碎片，目标比例与真实规划面积不同。
3. **实体被事后认领。** 同类型实体只要在 parcel 里就被计为实现，没有证据证明由该 parcel 生成。此前的邻近匹配更弱。
4. **质量门禁漏判。** `quality-report=warn`、零条实际 road、明显 underfilled 的场景仍可能 `world-quality-gate=pass`。以前答复中统一称它为 warn 不准确；应分别读取两个报告根节点。
5. **附属 JSON 和 scene 内的状态未独立核验。** 修改成功状态可能不影响现有检查。

## 本次实现

### 1. 面积分配

- 按场景宽深均分网格，将末端余数分摊到各格，不再单独创建极窄边缘碎片。
- 排序距离使用实际米制 cell 中心，而不是包含虚假边缘宽度的网格索引中心。
- urban/rural/nature 按累计平方米目标选择边界，舍入误差受单格面积约束；不再按格数冒充面积。
- 显式全零目标保留为 `unassigned`，不会隐式创建自然用地或地块。
- 仍是径向规划先验，不是 GIS 城市形态模型，也未证明坡度和水文适宜性。

### 2. 实际互斥的几何分区

- 记录原始 `zones`，校验不再从 envelope 的保留宽度反推父区边界。
- frontage/drainage strip 从最小 Z 边缘扣除；极小区域预留宽度截断到有效深度。
- 大型 urban envelope 先扣除 20m 公共空间带，再从剩余区域分成四个地块。
- 200m 以上宽深支持四分，均分后的约 238m 宏区仍支持四分；更小区域单地块。
- 使用偶数米宽深，实体整数中心不再使奇数宽地块的边缘向外偏半米。
- 检查唯一 ID、缺失父区、空矩形、不可表示坐标、父区包含、跨 Zone 重叠、Parcel/PublicSpace/Corridor 之间的所有占用重叠、预留面积守恒和公共空间预算。
- 扫描线维护活动矩形，避免对整张大图所有地块做完整平方级比较；仍须最终性能实测，不能仅据算法声称大地图性能验收。

### 3. 实体来源

- Parcel 和公共空间候选在生成入口获得 `generated.plan.<plan-id>` 稳定 ID，后续 rules placement 保留它。
- Parcel realization 只认领准确来源 ID，验证类型、唯一 ID、scale、有限且正数的宽深、任意旋转角度后的 footprint 包含关系。
- 无实体时输出 `missing_final_entity`，不猜测是坡度、道路还是水体导致；实际候选拒绝原因仍待贯穿 placement API。
- 空地块计划为 `no_developable_parcels`，部分实现为 `partially_realized`，只有非空且全部实现才为 `all_parcels_realized`。
- 这里的“地块实现”只证明来自该计划的实体满足上述合同，不等于建筑规划、道路可达性、公共服务或土地利用目标都已完成。

### 4. CLI 独立验证与门禁

- quality-report 从 manifest 重建 Zone，再重建 ParcelPlan 和 realization，对照 scene、parcel-plan.json、land-use-zones.json 和 infrastructure-plan.json。
- 重算预算，不信任持久化的 assigned/status；浮点预算允许 1e-12 相对数值误差以覆盖 JSON 一 ULP 解析差，状态、key 和结构仍完全一致。
- 对启用用地规划的 strict/release 场景，规划不达标、实体 underfilled、未实现 Parcel、城市无真实 road，均使 gate fail。
- 非失败但仍有警告时根状态为 warn，不再显示 pass。
- 新增端到端测试：同时伪造 scene 与 parcel-plan.json 的 accepted 状态，或篡改单独的 infrastructure-plan.json，CLI 必须拒绝。
- 访问篡改测试中先同步与访问无关的合法规划事实，再故意伪造 accessGraph，保持该测试针对独立访问重建，而非因陈旧 Parcel 提前退出。

## 目前实测结果（seed=20260906，temperate-plain，0.12，strict）

| 场景 | 实体 | 实际 road | 计划地块 | 准确来源且有效的地块实体 | 实体覆盖 m² | 用地规划比例 | 最终 gate |
|---|---:|---:|---:|---:|---:|---|---|
| 512×512 | 10 | 0 | 16 | 1 | 41326.4 | planned_within_tolerance | fail |
| 1000×1000 | 35 | 8 | 49 | 3 | 194599.0 | planned_within_tolerance | fail |

产物在 `bevy/work/partition-v2-512` 与 `bevy/work/partition-v2-1000`，不是早前 continuation-r33 等旧产物。
512 场景规划面积现在 urban=65536、rural=131072、nature=65536 m²；1000 场景规划面积见对应 budget JSON。
6×10km 本轮只验证 **规划层** 非空及互斥几何，未执行完整地形 bake，不构成最大地图最终验收。

## 测试证据

- 对 12×12 个宽深组合、3 种用地（432 组）测试 tiny / odd / partial / macro envelope、保留带和地块的合法分割。
- 宏区正向校验测试、缺父区、跨公共空间重叠、保留带移入 envelope、超大坐标尺寸等反向测试。
- 错误来源 ID、重复实体 ID、越界 scale、45° 旋转越界不能计为 accepted。
- 全零用地目标不产生可开发地块；6×10km 规划产生超过 3000 个合法宏地块且通过几何门禁。
- 完整日志：`bevy/work/partition-v2-final-tests.log`（更新后最终测试结果以文件为准）。

## 下一步必须工作（不能再以 green test 代替）

1. 道路规划先依据自然 raster 和可建设工程方案形成连通路网，再让地块沿实际 road frontage 切分。512 场景零 road 是尚未解决的明确故障。
2. building footprint 与 parcel 土地面积分离，建筑、庭院、公共空间、道路和农地不得用单个巨型实体代替；现有 entity coverage 指标不是城市用地完成的充分证据。
3. 将候选的结构化拒绝日志贯穿到来源 ID，记录真实 slope/water/collision/access rejection，不从最终位置猜测。
4. 自然地形的真实地貌质量、侵蚀/沉积与人类工程交互仍应按主路线图逐项完成。当前水文 audit 仅证明 sampled drainage 数据一致性，不证明水力安全。
5. Infrastructure realization 当前仍是兼容实体包含匹配，不是 corridor 来源及工程完成的严格证据，后续需与本轮 Parcel provenance 一样升级。
6. SettlementGraph 仍是事后派生图。许可证清单、HTML 实际截图验收、最大地图性能和最终工程验收均不能被本轮结果替代。

本轮未引入外部源码、依赖或资产，没有增加第三方许可证义务。
