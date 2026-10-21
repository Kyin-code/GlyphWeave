# 48 城案例先验进入正式生成链

日期：2026-10-21

状态：**已接入；手写字段优先**

## 1. 问题

`urban_case_priors` 已包含 48 个核心切面的自然地理、三阶段历史、城市四层结构、
社区类型、特殊因子和土地压力，但自动组装函数此前只在测试中被调用。正式
Manifest 仍需要人工抄写 `naturalGeography`、`urbanSlice` 和
`communityProfile`，容易产生案例数据与生成输入不一致。

## 2. 接入方式

正式 `bake_world` 在 morphology 编译前执行：

```text
style
→ referenceData.caseIds[0]
→ 48-case prior
→ 缺失字段自动组合
→ morphology compile
→ effective style
→ natural geography / route / settlement / epoch / market
```

自动补齐字段：

- `naturalGeography`：setting、区域起伏、气候带、地貌过程、水文提示、
  灾害和确定性 seed；
- `urbanSlice`：settlement scale、feature cut / whole settlement、焦点地物、
  场外连续性、特殊因子；
- `communityProfile`：案例社区类型，并标注 `inferred=true`；
- `urbanCasePrior`：案例 id、区域、核心切面、焦点、完整三段历史、
  context case ids 和证据等级。
- 当 `settlementEvolution` 存在但没有手写 `districtDemand` 时，还会根据
  三段历史中的水港/治水、铁路/工业、轨道新区、更新保护和旅游保护机制，
  推导 old-core / renewal / new-growth / periphery 预算份额。输出标记
  `inferred=true`，模型名为 `case-history-keyword-heuristic-v1`。
- 推导预算的 `model`、`inferred` 和 `inferenceReason` 是
  `settlementEvolution` 的正式可消费字段；推断预算若缺少模型或理由会被
  严格 schema 失败关闭，避免审计信息只停留在 `urbanCasePrior` 镜像中。

已手写的值不会被覆盖，因此香港等控制场景可继续保留语义 v2 的主体关系、
锚点和场外连接。

## 3. 失败语义

- `caseIds` 为空或不是字符串数组：失败；
- 引用不存在的案例：失败并具名；
- 未提供 `referenceData.caseIds`：完全不改 style，旧 Manifest 保持原字节语义。

这使 48 城引用成为真实来源字段，而不是只用于展示的注释。

## 4. 验证

```text
worldgen::urban_case_priors::tests                         7/7 pass
worldgen::settlement_evolution::tests::district            2/2 pass
```

完整 Core 回归：

```text
884 passed; 0 failed; 34 ignored
```

其中新增：

- 重庆案例自动补齐山地带状河谷的自然地理、feature cut 和社区类型；
- 重庆历史自动推导四个时代需求份额且总和为 1；
- 推导结果可直接通过 `settlementEvolution` 严格校验并供预算分配消费；
- 手写 `urbanSlice` 保持优先；
- 手写 `districtDemand` 保持字节优先，同时保留案例推导结果供审计；
- 未知 case id 失败关闭。

## 5. 后续

当前自动组合选择 `caseIds[0]` 作为主先验，并把其余 id 保留在
`urbanCasePrior.contextCaseIds`。后续若要混合多案例参数，应建立明确的
权重、冲突处理和来源审计，而不是静默平均多个城市。
