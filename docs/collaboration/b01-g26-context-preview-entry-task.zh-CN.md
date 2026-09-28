# B01-G26：保存场景上下文预览的可选项目入口集成

日期：2026-09-22。

依据：
- G25 检查：G:/difyllmwiki/GlyphWeave/docs/collaboration/b01-g25-inspection.zh-CN.md
- G25R1 检查：G:/difyllmwiki/GlyphWeave/docs/collaboration/b01-g25r1-inspection.zh-CN.md
- G25R1 交付：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g25r1-preview-browser-contract/

## 0. 目标

把已经接受为路径 A 的“只读 2D 上下文投影”从独立导出包推进到项目中的**显式、可选入口**，使用户可以选择一个已审核 G25 context-preview 包并打开它；不改变默认世界、不接 ST18、不把保存事实升级为工程安全结论。

本任务不是 G25/G25R1 修复包，也不是路径 B 的真实 3D 迁移。

## 1. 允许的范围

允许：

- 增加一个明确命名的项目入口或静态 artifact launcher，例如 `saved-context-preview`；
- 从用户显式指定的 preview package 读取 `COMPLETE.json`、manifest 和 render model；
- 在项目 shell 中显示来源 sceneId、window、source hashes、schema drift、natural/final 状态、实体/道路/access 数量；
- 打开 G25 自包含 canvas 2D viewer，或在同一合同下嵌入其只读页面；
- 对入口输入做只读完整性检查和清晰错误提示。

禁止：

- 不修改 G23 Three/WebGL viewer；
- 不引入 WebGL/3D 语义；
- 不将 G25 2D 模型接入默认 SceneIndex、默认世界或自然生成；
- 不生成 EngineeringInput，不重新求路，不建楼梯，不做工程 build；
- 不把 `walkable_route`、`connected` 或道路关系写成 verified route/buildable/safe；
- 不自动扫描工作区寻找“最近包”，必须由用户显式选定路径或包 ID；
- 不修改 G24/G25/G25R1 旧目录、固定来源和冻结证据。

## 2. 入口合同

入口必须：

1. 拒绝缺少/错误的 `COMPLETE.json`、manifest、render model、preview schema/version；
2. 校验 manifest/marker/context/render-model 的 hash 链和必需文件集合；
3. 显示来源路径和 sceneId，禁止只显示“loaded”；
4. 对 schema drift、unknown/unmodelled kind、空 access、sourceObservation-only 状态显式显示；
5. 加载失败时不显示旧模型、不保留上次对象、不产生误导性绿色状态；
6. 只读加载，不写回 preview package 或 source root；
7. 报告路径和日志路径不得位于被加载包内覆盖输入。

若入口使用浏览器/file URL，必须说明相对资源解析和本地加载合同；不以“能打开 index.html”替代完整性审核。

## 3. 最小测试

具名测试至少覆盖：

- 合法 G25 package 可以被用户显式选定并显示；
- 缺 marker、manifest、render model 或 hash 不一致时拒绝；
- unknown schema/version 和额外文件拒绝；
- natural/final 切换仍指向正确字段；
- entity/road/access 数量和 source observation 标签保持；
- 加载坏包后再加载好包不会残留旧对象；
- 包内路径穿越/覆盖、重复加载、空 access 和 schema drift 状态；
- 普通测试不写 G25/G24 冻结目录。

## 4. 最小 CLI/浏览器矩阵

建议 CLI 或项目入口测试 8 次：

1. 合法包显式加载；
2. 缺 marker；
3. manifest hash 错；
4. render model hash 错；
5. 未知版本/额外文件；
6. 坏包后加载合法包；
7. natural/final 状态和数量显示；
8. 包内覆盖路径拒绝。

建议浏览器 3 次：

1. 合法包从项目入口打开，显示来源状态和 2D 画布；
2. 坏包拒绝且视口为空，再加载合法包无残留；
3. natural/final 切换、实体选择和 access observation 标签。

不重跑 G23 三场景浏览器矩阵，不做 WebGL/GPU 断言；真实 3D 若重新提出，另立路径 B 任务。

## 5. 预算建议

建议新授权：

- cargo：4；
- CLI/入口进程：8；
- 浏览器：3；
- 辅助驱动：4；
- 归档≤32 MiB；
- ST18/默认世界/自然生成/工程 build/route/bake/realize/发布：0。

历史 G24/G25/G25R1 额度不重置、不倒写。若只做前端/项目入口且无需 Rust 构建，应明确不虚报 cargo 使用量。

## 6. 交付与停点

新目录：

`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g26-context-preview-entry/`

交付顺序：入口源码/测试 → 最终构建 → CLI/入口矩阵 → 浏览器观察 → report/progress → 递归 manifest → receipt → 只读核验。

明确区分：

- 本轮实测；
- G25/G25R1 既有观察；
- G23/G23R1 既有 3D 观察。

正分支只能声明“保存场景上下文的可选只读项目入口交检查”，不能声明真实 3D、工程可行性、路线安全、默认世界或城市可达性验收。

**A 冻结、B partial、C 不推进。**
