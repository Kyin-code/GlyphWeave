# B01-T2 R9 检查裁定 + R10 任务（给 V41F）：用审计自身的失败信息定位 20 块院落

日期：2026-09-30。检查员/项目经理：Claude。执行：V41F。
上位：`dsh-b01-t2c-r9-inspection-and-task-20260930.zh-CN.md`（`3f63e5b2…1ce5`）。R7 起的额度、闸门、硬边界全部继续有效（1km 0/2 已用，本轮可用）。

**请严格按任务书完成任务，不得因为其他任务无关问题擅自中断或停止部分步骤；上下文压缩、会话切换、摘要丢失或可修复的驱动问题不是跳过必测步骤的理由。需要中断时先写 checkpoint，恢复后继续。**

---

## 第一部分：R9 裁定

| 项 | 裁定 |
|---|---|
| 窗口由共享函数推出（`worldgen.rs::support_pad_half_extents`，规划器不再手写 12/14） | 接受，源码确认（`worldgen.rs:4256`，`road_network.rs:535`） |
| A′ 扫描窗口 ±14 → 4419 m² / 21；±12 → 4211 m² / 20 | 数据接受 |
| “检查员假设被推翻” | **表述过度，更正为：“只扩大 A′ 扫描窗口”这一修法被否定，失败格位置与根因仍未知。** 垫层 carve 对精确站点本就覆盖 ±14（`support_pad_half_extents(.., precise=true)`），与审计窗口一致。±14 实验只说明：把声明抬高后站点重新进入义务，但院落仍不实现——真正的失败原因没有被观测到 |
| 探针放错测试、编译失败后删除并复验 | 处理正确 |
| runs.jsonl 6 ↔ 6 日志；补写 r8/write-set.json | 接受 |
| 中断原因是上下文耗尽而非停止条件，并如实区分 | 接受；本轮用下面的“上下文纪律”解决 |

---

## 第二部分：上下文纪律（本轮起强制）

你已连续两轮因上下文耗尽中断。原因很可能是读取了大文件或大段日志。从本轮开始：

1. **禁止整读大文件**：`auto_roads.rs`（约 1.15 MB）、`road_network.rs`（约 156 KB）、`worldgen.rs`、`frontage_supply.rs` 只能用 `grep -n` 定位后按行号范围读取，每次不超过 120 行。
2. **cargo 输出一律重定向到日志文件**，只读取 `test result`、`FAILED`、`panicked` 与自己打印的标记行（`grep`），不得把完整测试输出读进上下文。
3. JSON 证据只用脚本抽取需要的字段，不整份打印。
4. **每完成一个编号项就更新 `r10/checkpoint.json`**（已完成项、源码 hash、下一条命令）。新会话从 checkpoint 继续，不重读已完成项的材料。
5. 允许跨多个会话完成本任务书；每个会话结束前写 checkpoint。

---

## 第三部分：R10 任务（按顺序）

证据目录 `.../r10/`，`r10/runs.jsonl` 一条记录一份日志。

### R10-1 导出审计自身的失败信息（第一条命令）

不写新的逐格探针。审计 `worldgen.rs::verify_impl` 对每个失败的路边站点已经会产生：
`roadside site <id> changed after planning: design error <e>m, fill <f>m, cut <c>m, support sample Some((x, z, 声明mm, Some(实际mm)))`，院落/地表实现也有各自的拒绝原因。

1. 在 W1 冻结路径上（即那 7 个 frontage_supply ignored 测试所用的基线构建处，先 `grep -n` 找到构建该基线并运行审计/地表实现的函数），**把审计 failures 列表与院落/地表实现的拒绝原因完整写到 `r10/w1-audit-failures.json`**（测试内 `serde_json` 写文件即可，写到一次性目录后复制；不要打印到 stdout）。
2. 用脚本汇总：20 个站点各自的失败类型（design error / fill 超限 / cut / 支撑采样不等 / 院落其他原因）、失败格相对站点中心的偏移（是否在 ±12 以外的外圈）、实际值与声明值之差。写 `r10/w1-failure-summary.json`。
3. 同一份导出在 ±14 A′ 的实验配置下再做一次（临时修改，做完还原并复算 hash），对照两种配置下失败类型的变化。

### R10-2 按失败类型修复

根据 R10-1 的汇总定位根因（文件:行），在写集内修复。检查员预先列出几个可能方向供核对，**以数据为准**：
- 若失败是 `fill` 超限：审计用的是 `expected.max_roadbed_fill_m`，放置门槛用的是 `max_fill`，二者是否同一数值、同一窗口；
- 若失败是支撑采样差 1 mm 量级：声明与 carve 的毫米换算/取整不一致（实体 `world_y` 用 `support_level_mm.div_euclid(1000)` 取整到米，检查 `ground_height_m()` 是否由毫米值精确给出）；
- 若失败格在外圈且实际值高于声明：是哪一条道路的哪种 carve（路面/band/blend），A′ 声明应纳入的是哪一部分；
- 若审计全部通过但院落仍未实现：问题在地表/院落实现环节（`settlement/landcover.rs`、`development.rs`），按其拒绝原因处理。

目标：W1 院落未实现降到约 51 m² 量级。被合法拒绝的站点逐个列出数值。不得放宽审计精度或任何数值合同。

### R10-3 129-130 纵向失败（第三次延续，本轮必须完成）

用同样的方法：R5 真实 1km 世界（只读）的 `scale-audit.json` 中已有这条失败的完整文字，先用脚本抽出违反的站号与数值，再用离线全链重放导出当前源码下该路的纵向校验结果。写 `r10/road-129-130-diagnosis.json`，定位并修复（授权同 R7-2）。

### R10-4 验证补齐（R9-3 原样）

A′ 正反例与新增场景、三处生产突变、四读者测试、拒绝记录 serde 往返、确定性两次一致、1km 冻结输入全链离线重放（以 R5 真实世界为基准）。

### R10-5 收口

全库 0 failed、15 模块、冻结 ignored 套件（前后 hash 一致）、黄金快照、CLI 重建、全集绑定写入 `r10/`、`r10/write-set.json`。

### R10-6 1km（自行放行，规则同 R7-5）

闸门满足即运行，按预注册 v3 判定，成果直接交项目负责人。

## 停止条件（同 R9，只有这些可以提前交 partial）

1. 必须改 `road_alignment.rs`、`road_profile.rs` 或放宽任何数值合同；
2. 需要挪动或缩小站点；
3. cargo 达到 150 次；
4. 需要改 zoning、需求、尺寸、比例。

上下文耗尽不是停止条件：写 checkpoint，由新会话继续。

## 预算与边界

cargo 软计划 150（本轮重新计数），达 110 汇报。额度与硬边界同 R7。

## 交付

`r10/report.json`（逐条回应 R10-1～R10-6）、`r10/checkpoint.json`、`r10/write-set.json`、`r10/runs.jsonl`、`r10/w1-audit-failures.json`、`r10/w1-failure-summary.json`、`r10/road-129-130-diagnosis.json`、离线重放对比、全集绑定；若运行了 1km：`r10/1km-self-gate.json`、`r10/1km-run/`。
