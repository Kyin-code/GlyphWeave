# B01-G27A：保存场景上下文的显式项目交接入口

日期：2026-09-22。

依据：
- G26R2独立检查：G:/difyllmwiki/GlyphWeave/docs/collaboration/b01-g26r2-inspection.zh-CN.md
- G26入口任务：G:/difyllmwiki/GlyphWeave/docs/collaboration/dsh-b01-g26-context-preview-entry-task.zh-CN.md
- G25路径A合同：G:/difyllmwiki/GlyphWeave/docs/collaboration/b01-g25r1-inspection.zh-CN.md

## 0. 目标

继续路径A，把现有`saved-context-preview --preview DIR`交给项目操作层的一个**显式选择入口**。用户明确选择preview包路径后，项目可以打开G26只读2D入口；不自动扫描工作区、不自动选择最近包、不改变默认世界。

这是操作交接任务，不是G26修复，也不是路径B真实3D迁移。

## 1. 合同

入口必须：

- 要求用户显式选择或输入preview包路径；
- 在打开前验证G26 preview schema、COMPLETE/manifest/model哈希链和必需文件；
- 显示sceneId、window、来源hash、schema drift、natural/final状态、实体/道路/access计数；
- 明确显示“路径A：只读2D上下文投影”，并显示无WebGL/3D、无工程可行性、无路线安全证明；
- 加载失败时清空当前显示，不保留上一次成功包；
- 只读访问，不写preview包、G24 context包或固定来源；
- 不扫描工作区或隐式发现目录；
- 不接默认SceneIndex、ST18、自然生成、工程build、route、bake、realize或发布。

如果现有项目UI无法安全选择本地目录，允许提供一个显式CLI/文件路径交接器，不得为了“看起来集成”加入隐式扫描。

## 2. 必须测试

- 合法包显式选择后打开；
- 缺marker、manifest/model哈希错误、未知版本和额外文件拒绝；
- 先加载坏包再加载好包无旧模型残留；
- 两次选择同一包不重复服务、不重复对象；
- status/window/hash/计数与包一致；
- 非法路径、包内报告覆盖和工作区扫描均拒绝/不存在；
- 普通测试不写G26/G25/G24冻结目录。

## 3. 最小验收和预算

建议：CLI/入口进程8次、浏览器3次、辅助4次、cargo最多4次；归档≤32 MiB。浏览器只验证项目入口到G26路径A页面的交接、坏包隔离和来源状态，不重跑G23三场景、不做WebGL/GPU。

源码范围限项目入口与必要注册；不得修改G23 viewer、G26核心校验或G24核心读取合同，除非出现明确阻断并单独披露。

## 4. 交付

新目录：`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g27a-context-preview-handoff/`

顺序：源码/测试→入口矩阵→浏览器→报告/progress→递归manifest→receipt→只读核验。正分支只能声明“保存场景上下文的显式只读项目交接入口交检查”。

**A冻结、B partial、C不推进。**
