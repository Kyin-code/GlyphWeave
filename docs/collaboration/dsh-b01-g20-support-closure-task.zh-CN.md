# DSH：G20同包收尾建议（原范围、原预算）

用户转发本任务后继续；不新开设施开发包，不追加额度，不回写G19R3C。无需确认稿。

先读G:/difyllmwiki/GlyphWeave/docs/collaboration/b01-g20-inspection.zh-CN.md及G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g20-20260920/中的short-bend-probe.rs/log。

## 已接受，保留不重做

最小直跑/折返的生产build/route/verify均由检查员独立复现；原窄平台、原偏移端点、墙穿楼梯拒绝；保存平台缩窄且hash自洽时指定支持重查拒绝。保留当前生成器几何与无碰撞豁免方向，不退回G19坐标补丁或A/B二选一。

## 必须闭合的原合同缺口

1. **完整支持带**：单平台6×1.6m，折线(7.9,1,8.2)→(8,1,8.3)→(8.1,1,8.2)，当前报带宽1.5556/carried=true，实际内部折点1.4142m<1.5。子区间严格内部采样漏掉边界，端点代码仅检查整条折线首尾；steps=2还重复同一个中点。固定该反例，不修改其几何/阈值。把每段两端、折点入出方向与支持/净宽变化纳入有限矩形区间判定；无法证明覆盖就不能报carried。不能仅缩步长或硬编码坐标。
2. **测试无证据写副作用**：删除具名几何篡改单测对冻结work/collaboration/runs的固定路径写入，保留全部内存断言或使用每次独立临时目录。CLI篡改材料由显式导出步骤写本轮新文件。常规cargo test不得覆盖历史交付。
3. **支持事实准确保存/审核**：当前成功区间未保存layer/parts，指定审核仅比较narrowestBandM；伪造其它事实虽被整包diff拒绝，但指定重查仍true。将所承诺的事实保存并从保存几何逐能力复核，缺失/不符形成指定检查失败；不要把builder diff成功抓住写成支持重查抓住。保持真实几何篡改专项通过。
4. **结果映射/来源**：原偏移端点是检查员endpoint-near-edge.json，不是terminal-own-stair-solid。G20资源before hashes为中间版本而非G19最终快照，下一次完整汇报说明版本链；禁止为补档重写旧文件。

## 验收与预算

新反例拒绝、现有19项语义保留（测试不再写历史证据）、最小直跑/折返found+verified、指定支持事实/几何篡改重查拒绝。保留实测与helper证据层级，不把任一未知称完整通过；实际视图仍来自生产mesh。

按G20已报cargo6/8、CLI12/20，**最多余cargo2、CLI8**，失败调用计入，查漏扣减；旧结转额度已在G20内，不再另加。先完成局部源码/资源检查再用剩余测试与构建额度，不在最后验证后大段编辑。

仍仅现有engineering_connections/core/必要CLI及测试资源白名单。自然/ST18/bake/realize/默认世界/新设施0，无Git写操作；不改auto_roads、旧access阈值、墙/障碍合同。

正分支：全部原门闭合后一次交检，不自动接入真实世界。负分支：预算耗尽或仍有缺口，保存准确partial与执行版hash，停止且不自动开包。

全部新产物定稿后清单→回执→只读核验；旧证据保留，新结果引用supersedes，不倒写旧验证。A冻结、B partial、C不推进。
