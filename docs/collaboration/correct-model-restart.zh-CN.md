# 正确模型核验后恢复开发

日期2026-09-08。用户在模型偏差叫停后同意使用正确Luna/Terra继续。旧Maxwell/Euler保持关闭，不调用resume_agent。先新建指定model的worker，仅握手不开发，主Agent独立读turn_context核实后再下发工作。

## 新实例与已核实首轮

- Terra / Carson / 01a07fc3-69d6-7a31-8137-fbca84a3def5：2026-09-08 14:45:08 +08:00，model=gpt-5.6-terra。
- Luna / Hooke / 01a07fc3-a0e8-7863-b6b9-a7fa09b01f00：2026-09-08 14:45:22 +08:00，model=gpt-5.6-luna。

原始本地会话路径和精简核验在bevy/work/collaboration/review/correct-model-restart.json。不用昵称或prompt自报替代实际字段。这里只核实客户端选用模型，不保证custom服务上游没有再次映射。

## 每轮防护

不关闭后resume指定模型worker；需要新任务时优先新建明确model再核验。在保持打开的实例send_input后主Agent再次读取新增turn_context；worker每个新执行轮首个工具只读自己的最新turn_context，model与预期不符或记录缺失即停止、报告，不读写业务文件、不跑测试。worker不能仅凭自称模型继续。
若任何核对不符，立即关闭对应agent、不改配置强行回退、不启用替代模型。以后不把12:23后旧Astra工作重新署名给本轮正确模型。

## 当前接续写集与任务

唯一G:/difyllmwiki/GlyphWeave，F:/modenmapbox禁止修改。HEAD仍6020fbe2713e8c79c1b2934b244402dc93e1df14。中断中的修改是未验收输入，先读实际文件与旧review，不清理或回滚。严禁所有Git写操作、暂存提交推送、整库格式化、新依赖/额外模型/preview服务或最大图。

### Terra：A-T3-R1-correct-terra

接续docs/collaboration/a-t3-core-task.zh-CN.md与a-t3-result。仅修改bevy/crates/core/src/worldgen.rs中候选ID及相关小测试、bevy/crates/core/src/worldgen/entity_identity.rs。ecology.rs已追加的ignored真实等价入口只读，不运行。新结果docs/collaboration/a-t3-r1-terra-result.zh-CN.md与证据bevy/work/collaboration/correct-terra-t3-r1/，不覆盖旧回执。

主Agent已裁定transport边界：使用保留方向语义的transport专用planner，如intent-road-ns、landuse-road-ns、landuse-sidewalk-ns，使stable ID仍被旧constructor/normalize识别方向。key来自filter前原始bank/网格/slot，不用成功serial或core_index。固定具名单例/explicit planned IDs保留；不全局修改其他stable_id格式、不最终事后改名。无需修改access/agriculture排序代码；新scheme的一次排序迁移须明确，证明同scheme重排/新增空间无关对象不重编号、NS/EW几何和引用正确。保留旧opaque-ID错误yaw反例作为设计依据，补真实stable transport正向。

你仅有独占窄Rust test槽（test-threads=1），不workspace/release/大图。核对未授权移动/修改/移除/重复ID均失败的inventory API，不以库函数通过宣称CLI preserving workflow已接好。遇需CLI文件则报告，后续单独授锁。

### Luna：A-D2-R1-correct-luna

接续docs/collaboration/a-d2-review-01.zh-CN.md和a-d2-parity-contract。仅修改scripts/collaboration/check-preview-world.mjs及.test.mjs、adapters/html/app.js和core/render-stream.js；必要ecology生命周期修改先报告。新结果docs/collaboration/a-d2-r1-luna-result.zh-CN.md与证据bevy/work/collaboration/correct-luna-d2-r1/。不覆盖旧回执，不写review/不执行会向review写文件的脚本。

定点补齐nonMesh仅允许独立source access_pavement摘要且actual accessSurfaces mesh对应，故障回执真实注入/失败诊断/迟到交付与新token绑定，elapsed非负，out词法/realpath拒绝world/input树内写入（先验证再创建，保留wx）。非生态entityCoverage/heightCoverage未完成必须使完整A-D2为incomplete，可单列componentStatus pass。不能用全部incomplete掩盖组件反例；完整合成组件正向与逐项负向保留。

允许Node单元与静态检查，未授真实浏览器/新server/cargo/大图槽。D3a Rust源parity任务书已备，但先交这轮修订再派发，不一次抢写CLI或worldgen。

## 总控与目标

主Agent继续只拆分、资源控制、代码审查、独立验收。恢复原里程碑1=A目标但不标完成：A-D1/A-T1/T2局部证据保留，A03/A06/A07/最终大图等未完成。两worker最终必须交新hash/范围/命令/退出码/模型首轮自检证据/未验证项。模型错配不会被测试通过抵消。

## 用户最新指示：停止重复模型核验（2026-09-08）

用户已自行确认模型正确，明确不再检查模型。此指示覆盖之前每轮读取turn_context的流程：主Agent和worker不再为核对模型读取会话日志或运行相关命令，继续实际开发与验收。保留已确认的Terra/Luna分工，不恢复旧错误实例、不擅自换模型；下一任务继续显式指定用户要求的模型，不重复握手核验。此前事故记录与已取得证据保留。
