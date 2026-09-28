# B01-G25：保存场景上下文的只读预览消费者

日期：2026-09-22。

本任务建立在 G24 scoped delivered_for_inspection 之上：
- G24 检查报告：G:/difyllmwiki/GlyphWeave/docs/collaboration/b01-g24-continuation4-inspection.zh-CN.md
- G24 最终上下文与审核包：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g24-saved-scene-context/continuation-4/
- G23/G23R1 只读工程预览既有消费者保持冻结。

## 0. 目标

把已审核的保存场景局部上下文交给一个新的、明确只读的预览消费者，使检查员能看到：

- natural 与 final/engineered 两套地形表面；
- 保存场景中被窗口相交的实体及其身份、旋转、原始 bounds；
- 来源道路及其节点/边关系；
- saved access path 的观测线，并明确标注 `sourceObservationOnly` / `notReverified`。

本任务只证明“保存事实可以被真实消费者显示并保持来源绑定”，不把上下文升级为工程可行性、路线通过性或施工安全证明。

## 1. 严格边界

允许：

- 新增独立的 `engineering-context-preview` 导出/审核适配；
- 复用现有本地 Three/vendor 和 G24 已审核的 context 包；
- 新增只读 render model、来源 hash 绑定、四段审核和最小 HTML 入口；
- 在新目录进行真实浏览器加载、取景和选择观察。

禁止：

- 不生成 EngineeringInput；
- 不调用楼梯、工程连接 build、route、自然生成、bake、realize；
- 不把 `connected=true`、`walkable_route` 或道路实体标签转成 verified route；
- 不接默认 SceneIndex、ST18、默认世界或世界发布；
- 不修改 G22/G23/G23R1 冻结源码、G23 viewer 合同、旧证据和固定来源；
- 不因预览需要补字段、补高度、插值、删除未知实体或改变坐标。

## 2. 输入和来源合同

输入必须是：

1. G24 `engineering-context.json`；
2. 对应 `COMPLETE.json` 与 `manifest.json`；
3. 调用者显式提供的原始 world root；
4. 可选 `--window`，若提供必须与 context window 完全一致。

导出前必须重新执行 G24 audit 或等价独立重建，不能只信 context 包内的 `checks`。来源重建失败、未知 schema、sourceBinding/terrain/entity/context completeness 任一失败，都不得生成完成预览包。

预览模型必须保存：

- context schema/version；
- world/scene/context/manifest 的 bytes 与 SHA-256；
- window、世界坐标系和高度单位；
- natural/final 表面各自的采样来源；
- entityId、kind、assetId、位置、旋转、scale、尺寸、原始 bounds；
- roadId、from/to 节点、plan relation；
- access path 原始折线、状态以及 `sourceObservationOnly`/`notReverified`。

禁止只保存三角形数量或 `pathPoints` 摘要后宣称来源完整。

## 3. 显示语义

- natural 与 final 必须可分别开关，禁止合并成一张“真实地面”。
- 工程操作/道路/实体保持世界坐标；不得把 final 表面抬平或把路线投影到另一表面。
- 未建模 kind、水、保护、碰撞角色继续显示为“source fact / not converted”，不能渲染为安全空地。
- access path 必须显示来源观测标签；界面不得出现 `verified route`、`buildable`、`safe to build` 等结论。
- 选择实体时必须返回其源身份和标高/位置事实，不允许生成新的工程身份。
- 空数据、未知 kind、缺 source observation、schema drift 均需在页面状态中显式显示，不得静默成功。

## 4. 审核与篡改

预览包必须有独立 audit，至少分为：

1. `sourceBinding`：预览模型绑定的 context、world、scene、manifest 字节与显式来源重建一致；
2. `terrainParity`：natural/final 顶点与高度采样逐值一致；
3. `entityAndRoadParity`：实体、道路、节点、access path 逐字段一致；
4. `previewCompleteness`：必需模型、viewer、描述、完成标记和 vendor 资产独立完整。

必须拒绝：

- 改 natural 或 final 顶点并同步外层 hash；
- 删除实体、道路节点或 access path 并同步声明；
- 把 `sourceObservationOnly` 改成 verified；
- 删除 unknown/limitations/schema drift 说明；
- 错换 sceneId/window/context hash；
- 删除必需 preview 文件和其声明；
- duplicate/unknown field、未知 schema/version、报告路径覆盖输入。

每项篡改必须由对应阶段拒绝，不能只靠总 payload hash。

## 5. 核心测试

至少具名覆盖：

- natural/final 双表面独立显示与逐样本绑定；
- 负/非零坐标、窗口边界实体、旋转实体；
- road edge/node relation 与窗外 referenceOnly 节点；
- access path 保持原始折线和 observation-only 状态；
- schema drift/unknown kind/空 access 集合显式状态；
- context、manifest、render model 逐字段自洽篡改；
- 版本/字段/路径/必需资产缺失、重复和额外文件；
- 普通测试不写冻结 G24/G23 目录；失败导出无完成标记且不覆盖旧文件。

## 6. CLI 与浏览器最小矩阵

建议最低 12 次 CLI：

1. G24 固定窗口 context→preview export；
2. preview audit 四段通过；
3. 第二次同源导出确定性比较；
4. natural 顶点自洽篡改；
5. final 顶点自洽篡改；
6. 删除实体；
7. accessObservation 改 verified；
8. scene/window/context hash 错；
9. 删除必需 render model/声明；
10. unknown version/field；
11. 报告路径覆盖包或来源；
12. 预存输出目录/故障清理。

浏览器最多 4 次：

- 直跑固定窗口：natural/final 切换和实体选择；
- 道路/节点/access observation 显示；
- 重载后对象数、路线线条和来源标签不重复；
- 一个坏包拒绝并保持空视口。

不需要重跑 G23 已验收的三个人工工程楼梯场景浏览器矩阵；本包新增观察只针对 G24 保存场景上下文。

## 7. 预算建议

本包新授权建议：

- cargo：8 次；
- CLI：16 次；
- 浏览器/WebGL：4 次；
- 辅助验证驱动：6 次；
- 归档≤64 MiB；
- 自然/ST18/默认世界/工程 build/route/bake/realize/发布：0。

G24 历史累计 cargo25、CLI58、辅助6 不重置、不倒写。旧 G24 动态 symlink 未实测仍只作为观察边界，不得为了预览任务改安全规则；若另要补动态 symlink，应另行授权和单独记录。

预算不足时 partial 停止，保留实际运行记录，不删失败测试、不复制旧日志冒充新运行、不修改 G24 continuation-4。

## 8. 交付顺序

新目录：

`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g25-context-preview-consumer/`

顺序必须是：源码/测试 → 最终构建 → CLI 实测 → 浏览器观察 → 报告/progress → 递归 manifest → receipt → 只读 hash+bytes 核验。

manifest 生成后不再追加修改任何已绑定文件。来源类别必须分开列：本轮实测、G24 worker 观察、G23/G23R1 既有浏览器观察。

正分支只能声明“已审核保存场景上下文的只读预览消费者交检查”，不能声明真实城市、ST18、默认世界或可达性验收。

**A 冻结、B partial、C 不推进。**
