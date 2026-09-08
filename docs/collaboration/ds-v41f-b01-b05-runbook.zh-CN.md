# DS v4.1f：B-01～B-05 完整开发任务

按B-01→B-02→B-03→B-04→B-05顺序实现并自测，不需要逐阶段等待人工批准。本文件替代旧文档“仅执行B-01、后续等待派发”的限制。功能与验收要求不降低。

## 1. 基线与职责

- 仓库：G:/difyllmwiki/GlyphWeave；不修改F:/modenmapbox。
- A源码基线：42cb4973fe6fdf985d21ee2a1f4be3c0ce10382b；tag=milestone-a-accepted。
- 记录实际HEAD和改动文件；允许基线之后仅README/文档的提交。保留已有他人修改，不覆盖冲突。
- 你负责业务实现、测试及结果文档；主Agent负责最终检查和签收。
- 不执行Git暂存/提交/推送/重置/清理/分支切换，不启动其他agent，不修改A冻结世界、runtime、证据、标签和原manifest。

## 2. 要完成的五项工作

| 任务 | 必须实现 | 详细任务书（以下均位于G:/difyllmwiki/GlyphWeave/docs/collaboration/） |
| --- | --- | --- |
| B-01 | 独立前院/侧院/后院地块，真正生成庭院/通道并计入面积账本 | ds-v41f-b01-task.zh-CN.md |
| B-02 | 需求驱动的分层道路供给、真实地块与农田可达性 | ds-v41f-b02-task.zh-CN.md |
| B-03 | 城市/农村/自然保留地的用途分区、实际地表和缺额闭合 | ds-v41f-b03-task.zh-CN.md |
| B-04 | 形态profiles、公共服务空间关系及现实案例来源 | ds-v41f-b04-task.zh-CN.md |
| B-05 | 生态尾差、组合回归、最大图和浏览器集成验证 | ds-v41f-b05-task.zh-CN.md |

共同验收合同：G:/difyllmwiki/GlyphWeave/docs/collaboration/milestone-b-acceptance.zh-CN.md。
背景与现状：G:/difyllmwiki/GlyphWeave/docs/collaboration/milestone-b-plan.zh-CN.md。

每阶段实现和针对性自测通过后直接继续下一阶段。共享文件按阶段串行修改，记录必要的接口/版本/重构说明，避免后一阶段覆盖前一阶段实现。

若只有明确的内容供给缺额、工程接口与安全正确，可以继续能够解决该缺额的后续阶段；若编译、几何、来源、保护或固定入口存在错误，先修复，不能用错误runtime推进集成。问题无法在范围内解决时保存最小复现及未完成项，继续不依赖它的工作；不靠降低需求或放宽检查绕过。

## 3. 全程保持的约束

- Rust决定坐标、工程、生成和审核；Agent表达需求/profile；HTML只读取权威产物。
- 原自然源、保护区、固定实体/入口/毫米支撑不被静默改动；新功能缺省关闭，旧输入语义保持。
- 不减少原144 frontage、设施/housingMinimum、植物数量、用户用途比例或精确楼体尺寸来取得成功。
- 地块/建筑/院落/道路/农田面积独立且不重复计数；planned、reserved、realized、unfilled分开。
- 不把全部未开发城市/农田铺成草地后宣称用途实现；不靠更名或删失败字符串完成B。
- A的六类内容失败逐条对照原指标；未关闭的继续保留，不自动继承为B通过的例外。
- B-02可在新B manifest选择明确许可的道路政策预算；A原manifest只读，时间/内存及工程安全判据不放宽。
- 不引入未经核实许可的代码、依赖、模型、纹理或GIS数据；B-04允许只读来源研究。两项历史许可未解析不能被忽略。
- 算法放职责明确的小模块，worldgen.rs只做必要接线；只修改各子任务白名单及明确的机械适配位置，不全仓格式化。

## 4. 验证方式与运行边界

先纯函数/小fixture，再局部端到端，最后B-05集中集成；不要每个补丁重跑最大图或整套浏览器。

- B-01：最多3次512m生成及对应gate；每个生成/gate120s、4096MiB private。
- B-02/B-03：各最多3次≤1km生成及对应gate；每个180s、4096MiB。
- B-04：最多2次≤1km生成及对应gate；其他profile/seed组合以Rust内存fixture验证。
- B-05：最大图6000×10000m生成最多2次、独立gate最多2次，各600s/8192MiB；第二次生成用于确定性或一次明确修正，两者共享配额。
- 240块浏览器完整巡检最多1次，40分钟上限；此前最多2次≤2块的真实控制检查。保留原驻留/资源/来源标准。
- 全workspace、release构建各最多2次；定向测试优先。编译/测试/生成/gate串行，性能采样期间不并发重任务。
- 新B release输出到G:/difyllmwiki/GlyphWeave/bevy/target-r-b-<runId>/，不覆盖A exe；记录当前源码、输入、runtime、输出和采样的实际hash。
- 同一根因最多两次实质修复；仍失败就保存复现与证据，不盲目循环重跑或增加预算。

输出均写自己的新work路径，原失败记录不覆盖。进程仅停止自己创建的PID，Start-Process使用隐藏窗口；历史8099 preview和用户页面不动。无法运行的验证标not_run/incomplete，不伪造通过。

## 5. 交付

每阶段交付 G:/difyllmwiki/GlyphWeave/docs/collaboration/b01-result.zh-CN.md ～ b05-result.zh-CN.md，说明：功能实现、修改文件、接口/重构/兼容、正负例结果、实际几何/面积/数量对比、仍缺什么。

最终提供：
- G:/difyllmwiki/GlyphWeave/docs/collaboration/b-run-handoff.zh-CN.md：B01～B05完成情况、关键产物、未解决问题、建议下一步。
- G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b-run/final-receipt.json：实际HEAD、源码/输入/runtime/evidence绑定、命令/退出码/日志、运行次数、失败/未执行项、后台进程。

codeReady、contentStatus、worldQualityStatus、engineeringStatus、browserStatus、researchStatus分别报告。状态仅ready_for_review或partial，不自行签收B，不进入C。全部工作结束后保留源码与证据，等待主Agent核对。
