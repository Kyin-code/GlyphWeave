# G30-R3独立裁定：面积缺陷闭合；详情入口红项可据原始记录裁定；详情图例仍需修复

日期：2026-09-25（北京时间）。**G30-D2限定闭合；G30整体仍needs_changes，不进入G31。** 不批准仅修变量名再刷154/154的确认性重跑。

## 1. 核验与认可

证据：`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g30-r3-20260925/`。

检查员本轮只读核验、源码审查与查看真实截图，无cargo/产品CLI/浏览器/VM执行，无worker文件或Git写入。

- 当前R3 manifest176条、R2 81、R1当前56、G30首轮118逐项bytes/hash匹配，精确路径闭集正常。R1历史55→56变化仍是已有异常，不重新声称从最初封存以来零改写。
- R3最终c04包六文件、c06重复导出、当前源码与exe符合receipt绑定；c05 audit四段pass。model922d404c…不变，viewer d90923a3…，HTML1f8a9673…，Rust a335e48c…，exe c9f8e8aa…。
- a03的29张原始截图全部匹配磁盘与browser-record，check-id唯一，154必需项无缺项。
- 实际分类：**134项离线通过（含VM39）+20项浏览器结果（19pass/1fail）**。153/154是整次调用，不是154条浏览器测试；“offline100”在日志中不是实际离线总数。
- cargo-04日志含96单测与集成测试结果，最终包审核/资产字节与当前源码相符。构建绑定不是仅凭mtime验收；本次未重跑构建。

## 2. G30-D2：确实修好了

四个1280/960×final/natural首屏，读图栏均45px、地图全宽；重算面积1280为647/800=80.875%，960为487/640=76.09375%，均高于80%/75%。与记录四舍五入差异正常，五处地图采样点均命中画布。

检查员查看960首屏图：地图优先布局恢复，简短坐标/非真北、类别图例、米制范围、详情按钮都常驻。**认可面积修复，不要求重新证明同一个D2。** 仅有限状态观察，不能升格任意尺寸/任意详情展开状态结论。

原始记录还支持实体/地形真实点击、面板真实滚轮/地图缩放、同页已有选择→坏包拒绝→恢复等显著进展；5.6ms/6.9ms是本机固定操作render中位/p90，含readPixels，不称FPS。

## 3. 唯一红项：可限定采纳交互证据，但不能顺带认定全部详情可读

当前driver中将 `afterClickState.pageLoads` 逆向改回该断言处的 `afterClick.pageLoads`，重建出的全文SHA精确等于a03执行版 `db89bb86…`。检查员仅在自己的目录保存逆向重建版本，不执行、不倒写worker文件。

`sections.details`实际记录：
- 真实page.mouse.click进入inspect，details.open/aria-expanded/readingDetailsOpen均true；
- pageLoads=1、cameraUnchanged=true、surfaceUnchanged=true；
- detailsVerdict.ok=true、problems=[]、rampItems=5；
- 五个父级块可见；focus与真实Enter再打开均成功。

因此**限定接受“详情入口点击/键盘可用、不重载且相机/面保持”**。a03仍为153/154、exit1，修订driver仍未执行；不改原结果、不合成绿色运行。不需要为变量名再开一轮确认性浏览器。

## 4. 新发现G30-D3：摘要的不换行规则泄漏到了详情图例

截图 `G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g30-r3-compact-reading/runs/g30r3-a03/browser/g30-1280-details-open.png` 中，详情侧栏“图例”色块行继续向右伸出可见范围，末尾类别/色块在截图右边被截断。正文长解释已换行，色块行却没有。

源码 `G:/difyllmwiki/GlyphWeave/bevy/crates/cli/src/context_preview3d.rs:1909-1910` 的全局 `.legend-row { flex-wrap:nowrap }` / `.legend-item { white-space:nowrap }` 同时影响摘要和详情。摘要适合紧凑单行，375px内宽的详情侧栏不适合。

“详情块5/5”检查了reading-details、direction、legend、range、evidence-details这些**父容器**；原记录未逐项记录五个图例项的矩形/裁剪，也未证明被截到右侧的内容经实际横向滚动可达。不能从父盒可见推导所有子内容完整可见，也不能把detailsVerdict读取到文本/颜色当作用户看到了内容。

结论限于证据：当前详情图例在已拍视图中有横向裁切，完整子项可见性未证。不能断言永远不可达，但这不符合本任务的易读详情目标。下一任务明确让详情图例自然换行，保留摘要不换行；不改模型/几何/颜色，不新做阅读布局设计。

## 5. 预算：所谓“保守CLI12/12”有算术矛盾

预算和command-log列出：
- c01 export、c02 audit、c03 determinism、c04 export、c05 audit、c06 determinism：六个成功进程，有对应产物；
- 另两次audit参数拒绝，并明确说countedAsCli=true。

按其披露，本轮至少6+2=8次CLI。轮前6次，**整线至少14/12，超支至少2次**；不是本轮6次/整线12次。参数拒绝不因“未做工作”退回额度。拒绝进程的独立stdout/exit日志未在本归档找到，当前依据是worker预算/命令日志的两条披露；不伪称这些缺失日志已完整保留。

cargo按10/10、browser4/4、aux声明15/15维持；command-log还提多次node --check/oxlint且未逐次列台账，是否按执行验证计aux须说明，不能据此宣称全类别账已审定无遗漏。不得重置历史或追溯合法化超支。

## 6. 下一步：仅修详情裁切，窄范围新包验收

发布 `G:/difyllmwiki/GlyphWeave/docs/collaboration/dsh-b01-g30-r4-detail-legend-task.zh-CN.md`。

提供新独立R4工作额度cargo2/CLI3/browser1/aux2；此前至少CLI+2超支保留为异常，不吸收进“新增上限”。不重跑完整20场景，不为变量名制造R4；R4只因真实详情裁切而需要新产品包与针对性观察。

评价：这一轮有明确用户可见进展，不能说偷懒。但摘要与详情样式共用导致裁切、父级可见性代替子级、计数反复混用依然是可避免的交付质量问题。只读检查不会为了尽快收口把这些写成绿。

A冻结/B总体partial/C不推进；G30-D2闭合、详情入口限定通过、G30-D3待修，G31未授权。
