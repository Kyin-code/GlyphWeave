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

已手写的值不会被覆盖，因此香港等控制场景可继续保留语义 v2 的主体关系、
锚点和场外连接。

## 3. 失败语义

- `caseIds` 为空或不是字符串数组：失败；
- 引用不存在的案例：失败并具名；
- 未提供 `referenceData.caseIds`：完全不改 style，旧 Manifest 保持原字节语义。

这使 48 城引用成为真实来源字段，而不是只用于展示的注释。

## 4. 验证

```text
worldgen::urban_case_priors::tests                         6/6 pass
```

完整 Core 回归：

```text
884 passed; 0 failed; 34 ignored
```

其中新增：

- 重庆案例自动补齐山地带状河谷的自然地理、feature cut 和社区类型；
- 手写 `urbanSlice` 保持优先；
- 未知 case id 失败关闭。

## 5. 后续

当前自动组合选择 `caseIds[0]` 作为主先验，并把其余 id 保留在
`urbanCasePrior.contextCaseIds`。后续若要混合多案例参数，应建立明确的
权重、冲突处理和来源审计，而不是静默平均多个城市。
