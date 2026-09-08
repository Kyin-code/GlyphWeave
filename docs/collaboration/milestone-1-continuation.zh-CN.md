# 里程碑1持续协作计划（A稳定底座）

日期2026-09-08。用户最新授权：不再考虑v4f，原D2/D3队列转给GPT-5.6 Luna；GPT-5.6 Terra负责Rust核心；主Agent只协调、审查、独立验收。里程碑1解释为既有里程碑A，不修改目标标准。唯一仓库G:/difyllmwiki/GlyphWeave；F:/modenmapbox禁止修改。

HEAD 6020fbe2713e8c79c1b2934b244402dc93e1df14；A-D1 accepted_tooling、A-T1 accepted_candidate，均不是A整体通过。既有dirty保存。所有worker禁止git写操作、提交/暂存/推送/改分支历史/清理其他文件。主Agent按验收合同持续派发直到A-01至A-10有证据；不得自行扩到B/C业务，也不让known content failure抵消工程失败。

## 当前并行任务与写入锁

|任务|执行者|写集|运行许可|
|---|---|---|---|
|A-T2 生态查询优化|Terra|bevy/crates/core/src/settlement/ecology.rs、landcover.rs；如需独立辅助仅新增同目录ecology_spatial.rs并通过已有文件path模块引用，不改mod.rs；docs/collaboration/a-t2-result.zh-CN.md；bevy/work/collaboration/terra-t2/|独占窄Rust test槽，不许全workspace/release/大图|
|A-D2 全域HTML/资源验收工具和必要观测修复|Luna|scripts/collaboration/check-preview-world.mjs及.test.mjs；adapters/html/app.js、core/render-stream.js、render/ecology.js（仅生命周期/观测/重试相关）；docs/collaboration/a-d2-result.zh-CN.md；bevy/work/collaboration/luna-d2/|Node轻测试；浏览器完整巡检等待总控RUN槽；不启动新server|
|总控|主Agent|协调/验收文档与review目录|源码检查；重集成统一串行|

不改对方文件、原A-D1脚本、Rust gate/manifest、资产风格/许可证、新依赖。review目录只读，不能执行会向review写入的脚本。需要扩大写集先报告。上表相对路径均以唯一仓库为根。

## A-T2必须交付

先读docs/milestone-a-agent-task.zh-CN.md、统一acceptance-contract和实际ecology/landcover实现。仅优化候选空间查询，禁止改变密度/层配比/种子/ID/排序/尝试次数/footprint/碰撞/水体谓词/拒绝原因，原生态缺1株不能在本任务偷偷消除。

保留真正线性参考路径（cfg(test)或明确诊断模式，不让发布默认走慢路径），indexed与reference输出逐字段/序列化hash一致。覆盖负origin、旋转障碍、长道路或超大AABB、桶边界/切边、不相交邻近、植物不同宽度、失败/rejection顺序。宁可保守候选后用原完整谓词复核，不能以网格占用替代几何。

同时审查realize与verify_contract查询，避免优化生成却留下独立gate全扫描。不得为了快而让verify直接信生成结果或共享候选缓存充当真值。至少小fixture计数证明候选检查下降且结果不变；记录耗时但不以单次微基准称最大图加速。参考与索引真实6×10km最终对照由总控授RUN后执行；此阶段不跑大图。

允许精准cargo test -p glyphweave-core对应ecology/landcover模块，test-threads=1。不运行cargo fmt全库、不启动其他worker。输出实现/测试hash、精准命令及结果、边界限制、未证实项。

## A-D2必须交付

先读scripts/check-maximum-preview.mjs、scripts/check-render-stream.mjs、adapters/html/app.js与core/render-stream.js、已有Node测试。旧maximum脚本只有首尾，不得把它当全240块证据。

实现可执行巡检工具：同一页面、按实际chunk描述（含最后部分块与非零/负origin）访问全部chunk，并至少三轮回到同一组关键区。校验当前worldRevision/sceneId/chunk、生态实例IDs摘要/数量、patch数量与紧凑受信源对应；禁止fetch全scene.json、禁止把render-chunk自身当独立来源。现存parity文件没有足够当前身份时明确incomplete，不能用旧r47冒充当前r49。总控后续补独立紧凑parity。

补必要观测（有版本、只读stats；可执行导航仍用已有__focusAt），至少：generation/token或请求轮次、activeChunk/loaded chunks、pending/cache entries、resident render bytes、renderer.info.memory geometries/textures、drawcalls、资源释放/创建计数。先输出你实际能取得的字段，不编造JS heap/GPU字节。只改生命周期/丢弃旧请求/清理缓存/重试、不能变视觉资产或忽略实体。

冻结预算：稳定渲染时active高细节块=1、render payload缓存<=1块、residentBytes<=该块descriptor.bytes且实际相等；有界pending上限2（超过不得悄悄提高，先复现）。三轮回同一focus的warm基线后，geometry计数增量<=8，texture<=4且不可每轮持续增加；如有设计常驻资源，必须事前分类扣除而非事后删样本。JS heap使用CDP/实际API（可用时显式GC后采样），同focus回访相对warm基线增量<=64MiB、后两轮增长<=8MiB；不能测时unverified，不伪造0。单步导航deadline120s，整个全域巡检预算先按实际块数×120s作为故障上界，不当性能达标值；图生成/CLI gate仍600s/8192MiB不变。

负向：一次render chunk网络失败后恢复、故意迟到响应不得覆盖新focus、截断/错误hash/错revision内容拒绝、跨块重复/漏植物识别。工具需Node单元测试覆盖预算/覆盖率/三轮复访/错误状态/证据绑定，完整control正向不能靠全拒绝蒙混。out新文件wx，所有证据自己的work目录。

此阶段允许Node测试和只读本地HTTP探测，不浏览器全240遍历、不生成世界、不启动server、不运行Rust。需要更改HTML其他文件先报具体泄漏/接口复现。最终浏览器槽由总控串行发放，届时可用已安装Playwright/msedge，不装新依赖。

## 后续队列（持续推进，未同时派发）

1. 主Agent审查T2/D2小证据后，安排Luna A-D3：大JSON入口及权威/投影流式parity工具、基本许可清单。许可证必须查实际依赖及官方来源；未知明确未核实，不引入新外部代码。
2. Terra A-T3：稳定ID/存量约束先冻结语义（保存编辑与从零生成区别），补无关需求新增不重排身份、显式移除/移动记录；不使用计数冒充保留。
3. 冻结最终候选源码、Cargo/依赖/manifest/hash，串行全workspace、Node/Vitest/lint/typecheck，再release独立runtime；600s/8192MiB的6×10km生成和完整gate，精确授权已知内容失败而非工程失败。
4. 当前产物独立geometry/protection/natural/ledger、流式projection parity、同输入确定性、全240块浏览器巡检；不足回派对应worker修复，其他不相交任务继续。
5. 主Agent逐条A-01至A-10签收，发布复现命令/限制/许可证材料；全证据齐全才milestone-A-accepted，不宣称C或whole-world-quality-pass。

主Agent不因临时模型容量或单项困难停止全部工作，不替换未授权型号；不循环重跑无变化大图。任何worker执行完成必须交文件hash/命令/退出码/日志/未做项/无后台/Git未写声明。费用无法获取则unknown。
