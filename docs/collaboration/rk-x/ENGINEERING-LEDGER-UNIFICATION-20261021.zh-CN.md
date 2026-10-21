# 工程账本、入口 landing 与最终道路统一

日期：2026-10-21

## 1. 修复内容

### 1.1 道路 shoulder 方向

graded road 的 2m shoulder 原被误加到纵轴。现在运输带公式为：

```text
max(|along| - flatHalfAlong,
    |across| - (flatHalfAcross + shoulderM))
```

因此相同中心线、相同端平台，只横向扩展支援带，不再把道路两端错误延长。

### 1.2 门前 landing 进入同一 pad 账本

一个建筑实体可能同时拥有：

- 主体 support pad；
- 2m 门前 landing；
- 其它带 `boundedCutMm` 的同实体弱削方。

账本现在按实体 ID 对全部 cut carve 取格点最深值。车辆/道路 profile 仍单独记账，
不会与建筑 pad 混成同一 owner。

### 1.3 非 frontage 建筑进入账本

settlement evolution、service、residential recovery 等实体即使不是
`RoadsideSite`，只要携带 `boundedCutMm`，也进入同一 bounded-cut ledger 和
owner map。不再把合法建筑削方报成 `bounded_cut_outside_pad`。

### 1.4 最终道路实体成为 cut owner

道路拓宽和 profile 附着可能替换计划边上的最终几何。cut-owner 图现在优先读取最终
road entity，计划实体只补缺失 ID。这样账本、carve 和审计不会再引用不同版本的
同一条道路。

### 1.5 shoulder 边界数值稳定

support window 使用 1e-6m 的边界容差：格点恰好位于 2m shoulder 外边界时不会因
浮点减法落入错误 owner。该容差只处理“严格内点”判定，不放宽任何几何合同。

## 2. 回归

已通过：

```text
worldgen::bounded_cut::tests                 10/10
worldgen::road_support::tests                 2/2
a_graded_road_cut_clears...                  pass
a_recomputed_graph_reaches...                pass
```

R31 1km bounded-cut 控制重烘成功：

```text
bevy/work/r31/1km-b8-cut-r32l-current
```

```text
generate-world   pass
scale-audit      pass
quality-report   warn
```

`quality-report` 的 `warn` 属于该旧 probe 没有地标/完整开发计划，不是 bounded-cut
失败；ledger 自身已通过烘焙期验证。

## 3. 剩余问题

全量 Core 最近一次完整实跑为 `879 passed / 1 failed / 34 ignored`。
之前 6 个失败中本轮修复了 5 个。剩余：

```text
diagnosis_of_a_published_world_reproduces_its_frontage_report
```

当前差异只在一个候选的拒绝原因：

```text
diagnosis: entrance_fill_limit = 1
published: existing_support_changed = +1
```

accepted/attempted、其他拒绝类别和总量一致。该问题不能通过合并原因名掩盖；下一轮
需要定位 `append` 与只读 `diagnose` 在 support-change 预检顺序上的分歧。

## 4. 边界

本模块统一了工程所有权，不代表：

- 所有宏观开山/填海都已进入同账本；
- 挡墙、涵洞、排水、台地已共享同一几何版本；
- 旧城拆合和连续街墙已经完成。
