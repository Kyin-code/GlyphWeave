# 生鲜市场网络辐射与缺口审计

日期：2026-10-22

状态：**已实现、已实烘焙、已提交**

采用版本：`b3b5502 feat(worldgen): publish market radiation evidence`

## 1. 为什么补这一层

原有市场计划已经能按真实路网计算最近市场、容量竞争和总体覆盖率，但只有
一个总覆盖比例，无法回答：

- 5、10、15、30 分钟内分别有多少需求可被服务；
- 某个需求点实际被哪些市场分配，权重和网络时间是多少；
- 市场负载如何形成，是否存在局部过载；
- 未满足需求是“没有市场在时距内”还是“市场容量耗尽”；
- 发布的总覆盖率是否与逐需求分配守恒。

因此，市场规划现在输出三份互相校验的权威文件，而不是一个汇总数字。

## 2. 生成模型

### 2.1 统一输入

- 需求点来自最终聚落计划建筑；
- 市场位于真实路线节点旁，保留道路宽度、肩带、建筑避让和入口方向；
- 需求到道路、市场到道路的时间均沿真实 route polyline 计算；
- 不使用直线半径冒充服务范围。

### 2.2 分配

每个需求点对可达市场进行有限容量分配：

```text
weight = capacity * quality * exp(-beta * travelMinutes)
```

分配按需求点和市场容量迭代消费，结果同时形成：

- `assignedDemand`；
- `deficit`；
- 市场负载；
- 每个需求点的市场分配证据。

### 2.3 时间带

固定输出 `5 / 10 / 15 / 30` 分钟四个时间带。每个时间带包含：

- 可达容量；
- 已分配需求；
- 覆盖率；
- 容量/需求比；
- 对其他需求点的竞争压力。

30 分钟是分析带；实际是否可服务仍受各市场自己的 `maxTravelMinutes` 限制，
不会因为输出 30 分钟带就放宽市场入口合同。

## 3. 发布文件

### `market-catchment-plan.json`

保持原有市场、需求点、最近市场、负载和总覆盖结果。

### `market-radiation.json`

包含：

- 全局时间带汇总；
- 每个需求点的分配市场、网络分钟、权重和分配量；
- 每个市场的容量、负载、利用率、分配需求和分时间带可达需求；
- 需求侧容量/需求密度。

### `market-deficit.json`

逐条列出未满足需求：

- 需求量和已服务量；
- 缺口；
- 最近市场；
- `no_market_within_travel_limit` 或
  `eligible_market_capacity_exhausted`。

`quality-report.json` 另行汇总时间带、市场利用率、分配需求和缺口。

## 4. 独立守恒审计

`market_catchment::audit_artifacts` 失败关闭以下情况：

- 三个文件 format/status/scene 不一致；
- 时间带缺失或顺序错误；
- 逐需求 allocation 总和不等于 served；
- served + deficit 不等于 demand；
- 市场 allocation 总和不等于 `assignedDemand`；
- 市场负载超过容量；
- 缺口记录数与辐射场不一致；
- 伪造总覆盖率。

质量报告重新读取并验证三份文件，不信任单次生成时的状态字符串。

## 5. r28 实烘焙证据

输出：

```text
bevy/work/rk-x/v3-hard-audit-hk-r28-20261022
```

结果：

- 2,901 实体；
- 5 个分层市场；
- 总需求 4,250.320；
- 已服务 4,241.696；
- 总覆盖 `0.997971`；
- 总缺口 8.624；

时间带覆盖：

```text
5 min  : 0.582
10 min : 0.821
15 min : 0.998
30 min : 0.998
```

唯一缺口：

```text
route-auto-0.lot.32.0.1
deficit 8.624
reason eligible_market_capacity_exhausted
nearest market.3
```

该世界覆盖目标为 0.85，因此 0.997971 为 `pass`；缺口仍被保留而不是被抹平。

## 6. 验证

- 市场专项：7/7 unit tests pass；
- `baked_audit`：26/26 pass；
- r28 `scale-audit pass`；
- r28 `quality-report pass`；
- Computer Use 战略图、街景、第一人称/探索三视图 pass，HUD 为
  `1m geometry / 1m data`；`visual-feedback.json` 已写入 `pass`，加入
  视觉反馈后 quality-report 仍为 `pass`、warnings 为空；
- 两次烘焙的 128 个可比生成文件完全一致；唯一缺失项是第二次尚未执行
  `scale-audit.json` / `quality-report.json` 两个派生审计文件。

## 7. 边界

- 本层不是完整客流仿真；
- 市场容量关系仍是规划/审计模型，不是逐摊位经营模拟；
- 季节、节假日和游客需求接口可在后续场景 agent 中使用，但本轮不扩大
  ABC 里程碑范围；
- 渲染仍只读取已烘焙市场实体，不在 HTML 中重算市场逻辑。
