# G-1b 裁定记录与下一局部诊断（本轮不执行）

* 状态：**`geometry_constructed / engineering_unverified`**。本轮 **cargo / 自然 / carve 重放 / final terrain / realize / bake 全 0**；未改旧卡、旧回执、新回执与当前日志。

## 1. 裁定

1. **G-1a 维持空集阻断**（parcel z 148..166 / 194..212 与平带 z 173..187 无交集），不重开。
2. **G-1b 三项几何检查接受**：非空 ✓、入口到平带连续 ✓、**避开**对既有两个冲突点 (268,171)/(268,188) 的完全覆盖 ✓（`cx=270, cz=168/192, half_w=1.0, half_d=5.0, blend=2, priority=2, target=2390`，x=268 落在矩形外）。
3. 本 worker 此前"允许变量下不存在可行构造""只能格点 mask 或 C-1"的**族级结论撤回**——那只是对单一矩形族的推断，不足以否定其他构造；不再设计 G-1c，不再扩展族级论证。
4. 正确读法：**几何已构造，工程未验证**。点值相等不代表完整支撑或 access 通过；点值不等只证明该输入失败，**不得外推为所有连接方案失败**。

## 2. 旧回执一处失配的说明（新增文件说明，不修改、不重跑）

上一轮的 G-1 回执（`receipt.json`，3578B / sha256 `94bd6c7d…e013`）绑定了同目录的 `git-status.log`、`git-diff-check.log`、`doc-tree-check.log`。G-1b 轮我在**同一目录**重新执行 `git status` 并重写了 `git-status.log`，因此该回执的 **`git-status.log` 一项**现在失配（内容差异仅在于 porcelain 列表新增了本轮文件路径；`git-diff-check.log` 与 `doc-tree-check.log` 内容相同仍匹配）。

处置：**只在本文件与同目录 `receipt-ruling.json` 说明**；不恢复、不覆盖旧日志，不改旧卡与两份回执，不为补证重跑。这是因为新增文档改变了 `git status` 输出，属记录变化，不是证据被篡改。

## 3. 下一项实验（固定范围，本次未获批自然额度）

* 唯一问题：**固定 G-1b 是否改变两个"必经"保护格** —— 北 **`(270,171)`**、南 **`(270,188)`**；以旧的 x=268 两点 `(268,171)`、`(268,188)` 作**对照**。
* 基线：北/南候选分别作用于**同一份不含候选建筑的道路基线**；**只替换 connection**，pad 与 apron 不动。
* 输出同口径：每点 natural、before/after、决定性 carve 的**身份**（owner/component）、priority 与 target。
* **不得**用旧规则生成的最终栅格去验证新规则；反事实地形必须由显式 carve 列表生成（cfG(test) helper：`(scene, natural, zones, 显式 carves)` → 生产 encoder + `apply_terrain_carves`）。
* 该局部诊断**不包含** measure、access 寻路、final terrain、realize、bake。

## 4. 分阶段

1. 实现 + 合成验收（测试侧，尚未批准）；
2. 自然放行（**本轮不追加额度，未批准**）；
3. 结果只回答"这两个必经格是否被改变"，任何单点结果不得外推。
