# G34-R2 独立检查：重复渲染漏遮挡、SVG并集与状态诚实性仍阻断

日期：2026-09-28。裁定：needs_changes / accurate partial，不接受G34整体通过，G35不启动。
本轮独立检查没有修改产品或执行者证据；运行只读核验、实际源码函数VM反例、一个独立Chrome诊断会话（单独计账，不冒充执行者a12）。没有cargo、产品CLI、Git写操作。

## 1. 确认成立、应保留的进展

- 独立枚举R2闭集1002项，missing/drift/unlisted全0；原verifier本次exit0、ok=true、problems=[]；核验前后全目录无漂移。
- c17/c18六文件逐字节一致；最终包viewer与源码同为9729e0740a65d94c2684fd68e22cae5f129d00fdb0e2f6c7497f4165f02a2c1e，170680 B；model仍922d404cdbd413e567d5ab89094d69ef75f785ab73e469c5676a79dc025f23ff。
- 正确旧路径两份清单16/16、39/39均匹配。
- a12的partialKnowledge确实进入了该状态，unknownData=16；不是R1的条件蕴含假绿。短状态也已回到小地图框内（本次查看既有partial截图）。
- 6.3ms/8.7ms是交付报告中的性能结果，不否认测量发生；但下面重复渲染漏测证明尚不能称“正确性等价的优化达标”。本次未重测其20相机性能或完整生产链。

## 2. 阻断A：去重标记跨帧碰撞，遮挡变通视

G:/difyllmwiki/GlyphWeave/bevy/crates/cli/src/g28_viewer3d.js：index的dedupeStamp跨调用保留；minimapVisibilityGrid每次却令stampCounter=0；occlusionClassifyDetailed遇stamp[key]===marker便跳过三角。
独立VM抽取实际minimapVisibilityGrid、occlusionClassifyDetailed、occlusionSegmentHitMetres，固定全部输入和受控依赖：O=(0,0,0)，P=(10,0,0)，x=5墙，一格eligible，其余outside-domain。完整grid连续调用：
1. 首次occluded，hidden1/visible0；
2. 第二次unobstructed，hidden0/visible1；
3. 只清空dedupeStamp，第三次又occluded，hidden1。
这是实际产品函数在受控解析场景中的确定性错误，不是浏览器自然场景复现；不把它夸写为已证真实数据默认相机受影响。它已经足以否定“相同输入结果稳定”和候选去重安全性。
性能优化必须先修该问题并做cold/warm/repeated/interleaved逐格对照，再重新测量。

## 3. 阻断B：同一clipPath的polygon+rect并不是交集

产品模板把polygon和rect放成同一个clipPath的兄弟；报告称其自动得到cellRect∩footprint∩window。独立Chrome原生栅格结果否定了这个解释。
取交付HTML的实际clipPath，给polygon设三角域(0,0),(136,0),(0,136)，绘制原反例色块(61.25,61.25,8.75,8.75)。中心(65.625,65.625)在域内，点(69,69)在域外：
- 原结构：域内像素(65,65)红[255,0,0,255]，域外(69,69)也红；
- 仅在独立对照SVG中去掉rect：域内红，域外白[255,255,255,255]。
该原生渲染证明并列形状扩大允许绘制区域，未实现所需交集。诊断只改独立内存fixture，不改包。
另外启动真实c18页面后读到rect为x=y=4.375、width=height=131.25。源码1325附近仍执行半格内缩，只把ID改成minimap-domain-clip-window；所以“内缩矩形已删”不成立。
a12只证明格角在声明polygon外，未测该角是否真的被clip抹掉。polygon>4点也不是非矩形裁剪正确性的通用判据（三角形是3点，斜四边形是4点）。

## 4. 阻断C：numerical-degenerate仍被当成完整知识

代码unknownCount只取unknownData，judgedCount反而加上numericalDegenerate，knowledgeComplete只看unknownCount===0。
独立执行实际短线段原语，返回undefined/numerical-degenerate；将该类别作为受控分类结果输入实际grid，256格全部数值退化：实际status=unobstructed、visible=0、hidden=0、knowledgeComplete=true。
这不是“域外不是未知”的正确推广，而是把无法完成判定当成了通视。数值退化可以保持独立计数，但必须影响完整性和汇总状态；没有一个可判定样本不能称通视。此反例是聚合层受控fixture，不宣称256格同时退化是合法真实相机自然可达。

## 5. 阻断D：DOM实际未知数仍错误

a12原记录partialCase：unknownCount=unknownData=16、outsideDomain=96、outsideView=24、undefined=136。
实际可见文案却是“部分未知：0 格被挡，136/256 格未知”，截图一致。即内部新增量改了，但用户看到的仍是undefined总数，继续把域外/视锥外叫未知。
验收所谓“精确计数+可见文案”没有核对显示数字。需要状态、数据属性、正文、title/aria同源且数值逐项独立断言，不是只包含“部分未知”四字。

## 6. 性能预筛的审查边界

当前保守上界预筛使用离散采样及下一对角格；源码另有计算整格穿越区间的helper，但不能仅凭注释认为已经证明不漏交点。本轮没有独立确认新的预筛反例，因此不把“可能漏检”写成已发现产品缺陷。下一轮要求给出覆盖证明或改成逐穿越区间的可证明保守判断，并与无优化全三角oracle作固定种子差分。可保留已证明安全的标量求交和局部去重，不要求推倒整个优化。

## 7. 账务与执行纪律

- 本轮确实再次误写R1台账；搬回后又留指针注释仍属旧文件被改，不等于只读。新报告结尾“旧轮与检查目录只读未动”与披露矛盾。
- R2 ledger共44条，31条processId=null，12条exitCode=null。“processId唯一、attempt/start/exit完整”不能由字段存在证明；未知退出也不能计为失败或成功。
- ledger恢复说明称cargo等条目complete，与用户披露cargo早期未入账不可恢复冲突。cargo3只能称已记录下界，不能据此确认真实调用低于硬上限；不得补造历史条目。
- Chrome12超过软计划10但低于授权硬上限15，本身不是本轮违约阻断，不要求为这2次补批。
- 1002项闭集有效只证明封存一致，不证明产品、验收逻辑或账务口径正确。
没有证据推断主观懈怠或故意造假；但再次旧目录误写、把声明几何当渲染效果、优化缺重入验证、DOM计数未对齐，是明确的执行质量问题，必须改变验证方法。

## 8. 证据与下一步

独立证据目录：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g34-r2-20260928/。
state-repeat-check.mjs/stdout：原函数状态与跨帧反例。
native-clip-check.mjs/stdout、actual.png、isolatedControlPolygonOnly.png、browser-intent.json：一次Chrome、交付clip复制与原生栅格对照、真实包DOM。
closure-check.mjs/stdout：独立闭集和包绑定。
record-review.mjs/stdout：a12原计数、可见文案、clip测试实际条件与账务缺口。
执行任务：G:/difyllmwiki/GlyphWeave/docs/collaboration/dsh-b01-g34-r3-repeatability-clip-task.zh-CN.md。