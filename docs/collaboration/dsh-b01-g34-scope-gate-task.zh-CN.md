# DSH B01-G34：新目标范围与验收闸门

日期：2026-09-28。依据 `b01-g33-final-verdict.zh-CN.md`。

**本任务不是 G34 产品实现授权。**只确定下一目标和验收合同，不启动产品进程。

## 1. 冻结基线

唯一推荐基线：

`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g33-r2-minimap-clip-closure/cli-runs/c06-export/`

生产 ID：`g33-overview-minimap-3`。

G33 首轮、R1、R2、C1、C1-R1 和 G32/G31 证据全部只读。

## 2. 闸门输出

必须先生成：

- `g34-scope.zh-CN.md`；
- `g34-baseline-binding.json`；
- `g34-budget-proposal.json`；
- `g34-stop-rules.zh-CN.md`；
- 如涉及观感：`g34-visual-intent.zh-CN.md`。

## 3. 必须明确

1. 一个具体用户可见目标；
2. 与 G33 的冻结边界；
3. 允许修改的文件白名单；
4. 不涉及 ST18、默认世界、自然生成、route/bake/realize、工程安全或发布链的声明；
5. 离线纯函数、反例、边界例；
6. 必须真实浏览器验证的场景；
7. 预算：cargo、CLI、Chrome、driver、归档；
8. 失败、未观测、驱动缺陷和预算超支的停止口径；
9. 基线包、源码、exe、model hash；
10. 最终独立裁定方式。

## 4. 禁止事项

- 不修改 G33 产品或旧证据；
- 不启动 cargo、CLI、浏览器；
- 不把 G33 的截图、运行记录或 manifest 当作 G34 证据；
- 不在目标未明确前直接实现新功能。

## 5. 完成条件

范围闸门文件齐全且只读核验通过后，才可以另发 G34 实现任务书。此任务完成不代表 G34 已实现或已验收。