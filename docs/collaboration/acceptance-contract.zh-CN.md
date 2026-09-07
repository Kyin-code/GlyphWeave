# 协作统一验收合同：A当前 / C最终

日期：2026-09-08。根目录均为 `G:\difyllmwiki\GlyphWeave`。

## 1. A的验收边界

A验证“稳定底座”，不是把全地图质量fail改成pass。下表全部要求都需证据；未证实为未完成。A中允许保留的内容缺口仅限冻结基线逐条批准的B/C任务，保留原gate非零退出。

| ID | A必须达成 | 当前状态 / 证据方法 |
|---|---|---|
| A-01 | 当前源码、输入、运行二进制与测试对应 | 部分；需源码快照/dirty集hash、构建命令、exe hash，不使用旧结果代新源码 |
| A-02 | 固定实体/入口/访问图/路面一致，正负毫米不被自动改写 | T1优先；旧r49完整产物与当前新源码存在差异 |
| A-03 | 同输入确定性、存量ID与约束 | 未完成；roads/sites保留不等于所有可选对象保留，新增无关需求不得重排其他身份，移动/移除显式记录 |
| A-04 | 6×10km真实需求完整生成并有独立gate | r49已证明一次；最终候选需重验。必须含144新增frontage及旧设施需求，不以缩图替代 |
| A-05 | 性能优化等价与受控资源 | r49农业已对照；生态等新优化要新证据；生成和gate各600s/8192MiB，不提高原预算 |
| A-06 | 权威数据无超大单字符串读取风险 | 部分；任何改法不得删权威内容；原始/派生逐字段校验、截断/缺块/缓存竞态拒绝 |
| A-07 | 全240块HTML加载/卸载/重访与来源一致 | 未完成；首尾截图不是全域。记录三轮复访和资源曲线，驻留块受配置上限，不持续单调增长；数值预算开测前冻结 |
| A-08 | 保持自然源、保护区、旧最低需求，显式报告差异 | r49局部证据；新最终候选重验自然hash、保护样本0、来源图、面积ledger与全部新增失败 |
| A-09 | 发布基本可复现/说明与许可清单 | 部分；已有MIT和NOTICE，只盘点与标明未核实；外部新代码须事前核验 |
| A-10 | 文档和正负回归齐全 | r49新修改只core验证不能视为整套；完整workspace、Node/Vitest/lint/typecheck/tree/diff依风险集中运行 |

稳定ID、大文件路径和全域漫游没有证据前不得宣布A完成。已知内容例外不是工程例外：碰撞、数据伪造、图/实体错配、1mm支撑篡改、保护区改变、需求静默下降一律拒绝。

## 2. 提交回执

每个子任务一份 `docs/collaboration/<task>-result.zh-CN.md` 和机器可读receipt（放独立work目录）：

```json
{
  "taskId": "A-T1",
  "status": "ready_for_review",
  "sourceSnapshot": {"branch": "refactor/authoritative-baked-audit", "files": [{"path": "absolute-path", "sha256": "actual-hash"}]},
  "changedFiles": [],
  "manifest": {"path": "absolute-path", "sha256": "actual-hash"},
  "runtime": {"path": "absolute-path", "sha256": "actual-hash"},
  "commands": [{"command": "exact-command", "exitCode": 0, "log": "absolute-path"}],
  "checks": [{"requirement": "A-02", "status": "pass", "evidence": ["absolute-path"], "scope": "what was actually checked"}],
  "worldQualityStatus": "not_run",
  "knownFailures": [],
  "unverified": [],
  "runningHandles": [],
  "thirdPartyChanges": [],
  "usage": {"elapsedSeconds": null, "tokens": null, "cost": null}
}
```

示意占位值必须替换为真实值；未运行项为not_run，不能捏造hash或退出码。无runtime/manifest任务填null并解释scope；声称bake通过时这两项必须完整。hash索引不是独立语义证明。

## 3. 测试与资源顺序

- Worker：限定模块红—绿测试、小fixture、一次小图复现；不跑全库所有测试来拖延结果。
- 总控：审查增量→复现关键正负测试→批准候选→串行 `cargo test --manifest-path bevy/Cargo.toml --workspace`（大型集成必要时限制test threads，保证测量不竞争）→前端既有命令→release构建→独立runtime副本→同输入小图与最大图→原CLI完整gate→独立几何/来源/HTML检查。
- 单纯文档/只读脚本不用重生成世界；任何生成/工程语义变更必须对对应场景重验，不以旧hash泛化。
- 最终同输入确定性需独立重跑或可靠重放证据，比较实际语义/有效载荷；时间戳、输出绝对路径等非语义字段事前列出，不事后挑字段。
- 性能比较严禁CPU/IO重任务并发，记录机器/后台负载、warm/cold差异，不用一次测量声称通用倍速。线性参考保留但只在必要候选上跑一次。

## 4. 必须出现的负向样本

原入口堵塞但另一门可通；固定负毫米实体遇到+1mm错误高度；相同seed非零origin；缺高程块/截断文件；同步伪造scene和派生plan；存量ID重排/移除；跨块重复/漏实体；异步请求返回过时数据；生成人数/设施/植物缺额；超时中断目录；旧runtime和新源码混用。验证失败原因不能只有“崩溃”，必须知道哪条合同拒绝。

## 5. 接受/退回

- `accepted-subtask`：仅此子任务和证据范围通过，不表示A通过。
- `needs-changes`：明确哪条要求未满足，退回责任人修正，不由总控偷偷代写。
- `blocked`：具体缺失输入/环境；未接入DeepSeek只是该worker待接入，不阻塞Terra和总控可做的工作。
- `milestone-A-accepted`：A-01～A-10逐条证据齐全，总控记录质量gate保留例外和转B/C任务；禁止显示为whole-world-quality-pass。
- `milestone-C-accepted`：C任务书全部范围验证通过，无A临时例外；最终HTML和文档可交付。尚缺街区/自然/水文/许可任何一项都不是C完成。

验收报告必须列出缺陷和未证明部分，不用完成度百分比、测试数量或漂亮截图替代。
