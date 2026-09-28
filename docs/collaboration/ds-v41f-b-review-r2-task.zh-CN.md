# 转交 V4.1：B Review R2——完成512m真实小闭环

## 1. 当前结论

读取：
- G:/difyllmwiki/GlyphWeave/docs/collaboration/b-first-inspection-20260916.zh-CN.md
- G:/difyllmwiki/GlyphWeave/docs/collaboration/b-review-r1-result.zh-CN.md
- G:/difyllmwiki/GlyphWeave/docs/collaboration/milestone-b-acceptance.zh-CN.md
- G:/difyllmwiki/GlyphWeave/docs/collaboration/ds-v41f-b01-b05-runbook.zh-CN.md

R1的F1/F2/F3修复保留，但真实512m控制仍未通过：第二次在station45报告`required 7120mm exceeds 7040mm cap`，前置道路/地块/建筑闭环未完成。本轮不做最大图、release、240块浏览器或C。

仓库仅 G:/difyllmwiki/GlyphWeave；不改 F:/modenmapbox。A基线/标签/冻结世界、runtime、A证据不改。禁止Git add/commit/push/reset/clean/checkout/switch/stash/merge/rebase和启动其他agent。状态只交`ready_for_review`或`partial`，不签收B。

## 2. 目标

在原自然高程、保护区、水体、fill/坡度、实体尺寸和需求不变的情况下，让`examples/b-r1-control.manifest.json`的512×512控制场景真实完成：

- frontage requested/planned/realizedWithAccess = 24/24/24，unfilled=0；住宅16/16、店铺8/8；
- 24个parcel/building/yard来源链，20×26示例地块面积和四朝向合同保持；实际yard surface缺额为0或逐项给出可解释工程失败；
- 至少collector+local两级真实道路entity/profile/拓扑链，8/12/16m宽度只使用已授权用途，farm-access不得冒充collector/local；
- 自然源hash、保护高度、固定入口、旧已接受实体/道路ID保持，碰撞、悬空、淹没、越界、堵门和断路为0；
- 失败仍需保留requested/planned/realized/unfilled与拒绝原因，不能把报告字段直接改成成功。

## 3. 诊断先行

在改算法前新增或补充只读诊断（可在`bevy/work/collaboration/ds-b-r2/diagnostic-table-r2.json`）：每个自动road候选和station45记录start/end、父road/tier/width、自然高度、需要fill、允许fill、protected/water/solid/parcel冲突、候选拒绝原因。必须区分：

1. corridor本身超fill；
2. 共享端点/节点支撑半径；
3. parcel/既有site占用；
4. zone/purpose不符；
5. 真实目标位置不可达。

不能把“没有候选”直接写成数学不可行；至少保留被拒候选样例。

## 4. 修复边界

允许修改前一轮B相关的`worldgen/auto_roads.rs`、`road_network.rs`、`road_alignment.rs`、`road_profile.rs`、`frontage.rs`、`parcel_geometry.rs`、`development.rs`及必要测试/版本接线。允许新增窄模块或CLI测试。

禁止：提高`max_fill`、`max_rise`、坡度/保护/水体容差；修改自然源或整体推平；缩小建筑/地块、减少16+8需求、改用途比例；删除既有需求/道路/site来腾空间；扩大reachDistance或让不允许purpose的tier fallback；换seed只保留绿色结果；修改A冻结文件或验收checker。

如果station45确实在当前自然/工程合同下不可行：必须交付完整容量证明和明确的不可行结果，不能把不可行重新标作通过。只有在证明候选搜索/拓扑策略缺陷后，才允许调整候选顺序、端点选择、局部合法连接或在道路政策内选择不同合法路径；每次改变都要通过工程审计。

## 5. 测试和运行

先跑纯函数和定向core/CLI测试；覆盖宽路共享端点、station45重现、候选方向、tier用途、非零origin、四朝向和旧v1/8m回归。

真实控制最多2次新的512m生成及对应gate，串行，每次仍120s/4096MiB；不覆盖原R1失败目录。一次可用来确认修复，第二次可用于一次明确原因的复验，不得第三次自动尝试。若生成失败，gate不得伪造运行；保留进程日志。

成功后独立检查：quality-report、frontage、maximum-world（仅512m）、render-stream/landcover ledger和规则审计。不要跑最大6×10km、release或240块浏览器。

## 6. 交付

新增`docs/collaboration/b-review-r2-result.zh-CN.md`和`bevy/work/collaboration/ds-b-r2/`回执，列：

- 实际HEAD、全部改动文件hash、输入/运行时hash；
- R1三修复的保留和测试；
- station45诊断原始表及修复前后差异；
- 24/24/24及两级实网的真实产物路径与独立审计；
- 所有失败尝试、生成/gate次数、PID、时间/内存、未做项；
- 如果仍partial，明确说明哪条需求与当前地形冲突，不得把小场景不通过包装成B完成。

完成后停止等待检查员，不提交Git、不推进B-03/B-04后续或C。禁止删除/覆盖R1证据。
