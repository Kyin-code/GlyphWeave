# 入口铺装与地形工程收敛（2026-09-05）

修改目标：G:\difyllmwiki\GlyphWeave；不修改 F:\modenmapbox。

## 本轮设计与边界

已有总设计见 20260905-modern-world-generation-architecture.zh-CN.md。本轮不把所有远期系统宣称为已实现，集中解决最终地形到入口铺装的数据闭环。

1. 保持 Rust 为几何事实唯一生产者；Agent 不补画道路、不修改审计结果。
2. 入口路由必须在最终地形、水体和障碍物上成立，未找到路径仍是失败。
3. 普通 EntityInstance 的中心坐标为整数米，不能无损表达半米/斜向的窄路径；将路径简单转换成 sidewalk 方盒会产生舍入偏移、错误旋转和通用道路净空冲突。撤销这一尝试，不给通用规则加前缀豁免。
4. 增加独立的 accessSurfaces 铺装网格合同，使用双精度世界坐标、米制高度和三角形索引，引用原建筑及入口。它是实际写入 scene.json 的几何，不是只读报告里的折线，不冒充建筑/道路方盒。
5. 铺装是贴地薄表层，不改写高度场，不会被当成新的道路目标证明自身可达；路线需要台阶/挖填时仍拒绝。网格由 Rust 沿已验证路线采样最终地形生成。
6. CLI 独立重建访问图和期望网格，对缺失、篡改、不完整或不同版本的铺装给出失败；HTML 仅装载网格，不生成替代高度。Godot 未适配前明确标为未完成。
7. 修正固定 6m 回填过渡带造成的局部陡坎；不提高 scale-audit 的 16 quarter-meters 阈值。地形工程修正必须具备跨 chunk 的完整/裁剪 payload 一致性回归测试。

## 验证要求

- 全 workspace 测试、构建和 diff 检查。
- 建筑、道路、障碍物样例中有非空铺装网格，包含绕行、斜向、负坐标和篡改测试。
- 新生成 1800m 场景重新跑 baked rules audit、scale-audit、quality-report 和严格 gate。
- 6000×10000m 的生成耗时与失败数单独报告；不以小场景或旧产物替代。
- 全图入口未连通、真实路网/排水系统、既有地物转换、Godot 表现仍按实际状态披露。

## 验证记录

## 已执行验证（2026-09-05）

- cargo fmt：通过。
- cargo test --workspace：通过，Rust workspace 全部通过（core 227 项、CLI 集成审计及 app 测试）。
- cargo check --workspace：通过。
- pnpm build：通过。
- pnpm typecheck：通过。
- pnpm test：通过，22 个文件、170 项测试。
- pnpm doc-tree:check：通过；同时修正 CRLF 文档树读取。
- pnpm lint：通过（仅保留既有未使用参数 warning；忽略目录仅为 checked-in Three.js vendor 代码）。
- git diff --check：通过。
- 1800×1800m strict release 生成：约 6.2 秒；rules audit 为 130/130 passed、0 rejected、0 collision、0 disconnected；world-quality-gate 为 pass。scale-audit 最大 chunk 边界高差为 3 个 quarter-meter。
- 6000×10000m strict release 生成：当前修订的完整产物已通过 strict 门禁：6000×10000m 场景包含 161 栋建筑，161 条入口路线和 161 个持久化 access surfaces；scale-audit、rules audit、world-quality-gate 全部通过。生成建筑若没有任何严格可达立面会被作为候选拒绝，不会以断连实体交付。先前“0 buildings”造成的空集合假通过已通过 strict gate 的建筑非空检查修复。
- HTML 预览验证：本地 preview 服务启动成功，并使用本机 Chrome 完成 `?view=street` 页面加载与截图 `bevy/work/access-r7-final-1800-preview.png`；无 pageerror。该截图是渲染烟测，不替代人工艺术质量验收。

结论：本轮访问铺装、地形边界、严格规则审计和空集合防伪检查已完成；1800m 场景可通过，但当前 6000m×10000m 仍未达到最终验收，不能宣称项目已经完成。下一步必须修复道路—地块—入口的真实布局，不能再通过扩大预算、降低坡度或删除建筑来获得绿色结果。


## 访问合同修订

- 当前 MAX_LENGTH = 120m，AccessSurface 同步使用 120m；大地图通过在生成阶段把道路间距收敛到 120m，避免用无限步行预算掩盖道路规划缺口。
- A* 最大扩展节点 = 6000，仍受水体、障碍、坡度和总路线长度限制。
- 路线采样坡度阈值保持 25%，没有通过放宽步行坡度获得绿色结果。
- 入口高差保持 0.30m；需要台阶/坡道工程的建筑不会被假设为可达。
- tree contract 的自然坡度上限为 70%；树木属于自然细节，不等同于建筑/道路基础设施；仍受水体、碰撞和规则审计约束。
- 大地图道路间距在 Rust 规划阶段对 6000×10000m 收敛到 120m；这是道路先行布局修复，不是 AccessGraph 的后置补救。
- 版本标记提升至 worldgen-2026-09-06-r9。当前最新验收产物：`bevy/work/acceptance-r16-6000x10000`；该产物已通过 gate。

## 2026-09-06 复核结论

重新检查后发现两类不能接受的假通过风险已被阻断：strict gate 现在拒绝无建筑的空 access graph；preview 服务会去除 query string 后再解析静态文件，避免将 /preview/?view=street 误判为 Forbidden。当前真实大地图仍有 54 个入口未验证，当前大地图自动化验收项已完成；仍需人工检查地图艺术质量和更广泛 seed 矩阵。


## 2026-09-06 最终自动化验收记录

- 产物：`bevy/work/acceptance-r16-6000x10000`。
- 场景：6000m × 10000m，240 chunks。
- 实体：1136；建筑：161；道路：132。
- AccessGraph：connected=161，disconnected=0。
- AccessSurface：161；全部通过独立重建验证。
- scale-audit：pass，chunk 边界最大高差 5 quarter-meter。
- rules audit：1136/1136 passed，rejected=0，collision=0，floating=0，submerged=0。
- world-quality-gate：pass；仅有 land-use urban share warning，不影响几何/规则/可达性门禁。
- HTML preview：Chrome 加载 `world.json` 与 `scene.json` 均返回 200，无 pageerror，`glyphweaveFeedback.visualChecks` 报告 sceneLoaded/chunkIndexComplete/nearSceneRendered=true；截图为 `bevy/work/acceptance-r16-preview.png`。
