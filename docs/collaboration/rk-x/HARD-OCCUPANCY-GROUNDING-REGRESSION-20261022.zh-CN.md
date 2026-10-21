# 最终实体占用与接地硬约束回归

日期：2026-10-22

状态：**已修复并实烘焙通过**

## 1. 用户报告的失效

- 道路失序，局部道路被抬到相邻建筑标高；
- 建筑和道路压进彼此 footprint；
- 建筑与道路看起来悬空或插入虚空；
- 相邻建筑碰撞；
- 城市切面内容偏少，建筑类型过度集中在办公塔。

这些问题不是渲染误差。最终实体中确实存在建筑—建筑和建筑—道路 AABB/旋转矩形相交，也存在道路实体被后烘地质填充抬高数米的情况。

## 2. 根因

### 2.1 旋转包络错误

`spatial_half_extents` 只在 90° 时交换宽深，任意斜向仍返回未旋转 AABB。45° 建筑和道路因此被错误地当作轴对齐矩形处理。

### 2.2 聚落门限只看中心距离

候选地块用“中心距离小于最小边长的 0.9 倍”判断重叠。即使中心不重合，两个真实 footprint 仍可相交。最终建筑尺寸形成后也没有再次与已接受建筑及道路做精确碰撞检查。

### 2.3 道路拓宽发生在选址之后

聚落按原 8m 道路退距选址，规划史随后把道路拓宽到约 32m。建筑与市场仍在旧路宽的净空里，因而被拓宽道路穿过。

### 2.4 建筑台地填充和道路肩带互相侵入

同一坡面相邻建筑支撑包络重叠时，较高一栋的过渡填方会抬高较低建筑的地面。道路 transport shoulder 也没有在聚落选址中预留。

### 2.5 入口与地形使用了相反旋转约定

道路、地形 carve 和高度场按正向旋转；入口图按反向矩阵读取实体。正确落在斜向建筑边界的门被判定为 `anchor_not_on_building_boundary`。

### 2.6 市场物化重新使用了旧偏移

市场规划已经按最终路宽求出安全位置，但物化阶段重新用固定 `depth/2 + 12m` 计算中心，把市场移回拓宽道路上。

### 2.7 缺少最终实体审计

`scale-audit` 检查尺寸、契约、道路中心标高等，但没有把最终建筑/道路 footprint 相交、双向接地误差和完整 terrain support 作为统一失败门禁。

## 3. 修复

- 任意 yaw 使用真实旋转 AABB；
- 新增共享 OBB SAT、旋转 footprint 栅格采样；
- 聚落候选按真实地块矩形去重；
- 建筑选中后再次检查建筑—建筑和建筑—主道路精确相交；
- 聚落、市场和规划史共享最终道路宽度，并预留 pavement + shoulder；
- 建筑实体携带道路方向 yaw；
- 相邻且支撑包络重叠的建筑统一到同一台地标高；
- 生成建筑始终拥有 active platform，防止邻接填方穿入；
- 入口 local/world 变换与道路/地形旋转约定统一；
- 过长楼梯拆分为合法段，入口图支持分段楼梯；
- 市场物化使用规划阶段已经验证的中心，不再重算旧偏移；
- 新增 `final_scene_audit`，硬失败检查：
  - building—building OBB；
  - building—main-road OBB；
  - 完整 baked terrain support；
  - building base 双向接地误差；
  - terrain 穿入 building base；
  - road profile 合法性。

最终审计同时接入：

- 正式烘焙：失败时不发布 `scene.json`；
- `quality-report`：任一场景 final audit 失败，整体状态为 `fail`。

## 4. 内容修正

建筑数量仍不是固定目标。新增按时代区分的单体城市颗粒上限，让同一笔住户/岗位需求不能由少数超大塔一次吃尽。旧城、更新带、新区分别使用不同上限。

原型选择改为：

- 只有中心压力 > 0.78 才直接选择办公塔；
- 住宅与岗位同时存在时优先混合用途中层；
- 旧城、更新带、新区继续使用不同地块尺寸和覆盖率。

香港控制的需求提高为 1,400 户、4,200 岗位、30,000m² 商业，最终得到 53 栋规划建筑：

- office_tower：5；
- residential_block：17；
- storefront：24；
- urban_building：1；
- 另有 5 个分层生鲜市场。

## 5. 实烘焙证据

输出：

```text
bevy/work/rk-x/v3-hard-audit-hk-r19-20261022
```

结果：

- 2,901 实体；
- 53/53 建筑入口连通，0 断点；
- building—building collision：0；
- building—road collision：0；
- floating building：0；
- terrain-piercing building：0；
- unsupported footprint：0；
- invalid road profile：0；
- 5 个市场，覆盖率 0.997971；
- 200 段道路、14 段楼梯、483 棵树；
- `scale-audit pass`；
- `quality-report pass`；
- 最大 Chunk 边界高度差 0.957m；修复前为 6.695m。

运行态预览：

```text
http://127.0.0.1:8123/preview/index.html?view=street
http://127.0.0.1:8123/preview/index.html?view=strategic
```

战略图和街景均可见连续道路、混合高度建筑、市场、树木和地形；没有空屏。渲染细节仍属于后续视觉优化范围。

### 5.1 r26 预览回归与补充门禁

输出：

```text
bevy/work/rk-x/v3-hard-audit-hk-r26-20261022
```

r19 之后又发现两类不属于核心几何悬空的问题：

1. `street` 只加载焦点 Chunk，Chunk 边界会切掉地形与道路，视觉上把合法建筑
   表现成悬空；现在启动街景时始终加载邻接 Chunk。
2. 路线演化把每 8m 一个、长 32m 的分段实体逐个绘制，造成多层深色路带与
   锯齿状叠面；现在按 `routeEvolutionPlan` 的同一 polyline 一次生成路面、
   人行道和中心线，并停止重复绘制其分段实体。
3. 90°/270° 走廊的精确横向边界曾因 `sin/cos` 浮点残差被误判到 carve
   外侧，导致道路 shoulder 边缘保留自然高地、道路剖面与实际地形不一致；
   现在四个正交方向使用精确轴对齐坐标变换，并新增边界归属回归。

同时，`final_scene_audit` 新增建筑 footprint 水体覆盖检查，并记录
`submergedBuildings`。`quality-report` 独立重算同一检查。

r26 结果：

- 2,901 实体、4 Chunk、53 栋建筑、5 市场；
- building—building collision：0；
- building—main-road collision：0；
- floating / terrain-piercing / submerged / unsupported：0 / 0 / 0 / 0；
- invalid road profile：0；
- 53/53 建筑入口连通，0 断点；
- 最大 Chunk 边界高差 0.957m；
- `scale-audit pass`；
- `quality-report pass`；
- 运行态视觉反馈 `pass`。

预览：

```text
http://127.0.0.1:8130/preview/index.html?view=street
http://127.0.0.1:8130/preview/index.html?view=strategic
http://127.0.0.1:8130/preview/index.html?view=explore
```

流程教训：HTML 适配器通过 `include_str!` 编入 CLI。修改 `adapters/html/`
后必须先重新构建 `glyphweave-cli` 再执行 `generate-world`；否则会复制旧的
嵌入脚本，让源码内容与预览文件哈希不一致，形成假验收。

独立复核还发现 `quality-report` 用原始 style 重编译有效程序，而正式生成
使用 `urban_case_priors::compose_style` 后的 style。带 `urbanCase` 或社区
模板的世界会因此被自身生成物误判。现在生成与审计共享 composed style。

### 5.2 r27 最终占用与街景剖面闭合

输出：

```text
bevy/work/rk-x/v3-hard-audit-hk-r27-20261022
```

r26 之后的实浏览器复核和全量测试又定位到三个硬缺陷：

1. `oriented_local_coordinates` 与 `spatial_half_extents` 只排除了“非整数
   角度”，导致 45°、135° 等整数度斜向实体重新落入轴对齐分支。斜向道路、
   建筑支撑和候选地块因此仍有 AABB 误判。现在只有 0/90/180/270° 走精确
   正交分支，其他角度全部使用真实旋转。
2. 规则引擎的通用碰撞曾写成“任一对象是道路就跳过”；正确语义是只允许双方
   都是共享交通面时重叠。道路实体现在在规则放置前预先占据索引，建筑、
   植被和市场候选必须避让真实道路 footprint。
3. 多阶段 `engineering-placement.json` 的质量审计曾只按 `sceneId` 选择第一条
   记录，可能审到没有原始 `inputItems` 的阶段。`quality-report` 现在选择
   同场景且包含 `inputItems` 的权威 reconciliation 记录。

旧冻结水岸/切城夹具中有一个住宅和一个店铺曾依赖“建筑压道路仍放行”的
错误规则；精确几何重放现在会拒绝它们，并把替代候选计入当前确定性基线。
这不是放宽断言，而是把“房屋压道路”从旧通过的夹具中移除。

r27 结果：

- 2,901 实体、4 Chunk、53 栋建筑、200 段道路、14 段楼梯；
- 483 棵树、310 灌木、1,427 草丛、412 岩石、1 条河流、1 个公园；
- 5 个分层市场，服务 4,241.70 / 4,250.32 需求，覆盖率 `1.00`；
- building—building collision：0；
- building—main-road collision：0；
- floating / terrain-piercing / submerged / unsupported：0 / 0 / 0 / 0；
- invalid road profile：0；
- 53/53 建筑入口连通，0 断点；
- 最大 Chunk 边界高度差 0.957m；
- `scale-audit pass`；
- `quality-report pass`。
- 最终 `cargo test --workspace --no-fail-fast`：31 个测试目标，
  `1,140 passed / 0 failed / 37 ignored`。

r27 运行态预览：

```text
http://127.0.0.1:8131/preview/index.html?view=strategic
http://127.0.0.1:8131/preview/index.html?view=street
http://127.0.0.1:8131/preview/index.html?view=explore
```

Computer Use 实测：

- 战略图完整显示山体、道路、城市建筑、树林和海岸水体；
- 街景为沿路线节点的高位剖面，可见连续车行道、人行道/路肩、成排建筑和
  街道绿化，不再只加载焦点 Chunk 造成虚空切边；
- 探索视图无空屏，前景草坡、岩石、树木和中景城市天际线连续；
- HUD 为 `1m geometry / 1m data`。
- 预览 API 已写入 `visual-feedback.json`，`verdict=pass`；quality-report
  在包含视觉反馈后仍为 `pass`，warnings 为空。

对应代码/回归提交：

```text
c965dc6 fix(worldgen): enforce road-first exact occupancy
```

## 6. 失败关闭语义

不得把以下情况重新降级为警告：

- 建筑或主道路 footprint 相交；
- 建筑 base 高于支撑面或地形穿入 base；
- footprint 缺完整 baked terrain；
- 建筑 footprint 与水体相交；
- 道路 profile 无法验证；
- 已启用的市场覆盖计划未达到配置目标。

`rules audit` 的旧 AABB 报告仍可作为保守诊断，但最终发布判断以 `final-entity-audit.json` 和 `quality-report` 的精确 OBB/接地结果为准。
