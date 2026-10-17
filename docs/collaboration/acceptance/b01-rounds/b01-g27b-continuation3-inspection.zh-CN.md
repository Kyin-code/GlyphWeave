# B01-G27B continuation-3 独立检查（2026-09-23）

## 1. 裁定

**接受本轮为真实完成进展，并接受其交付状态为 `delivered_for_inspection`；不直接改判 G27B 已验收。** §3.1/§3.2/§3.3、几何/拾取、最终构建、CLI矩阵和真实WebGL观察均有实际证据，不能再说“没有干活”。独立检查发现一个证据打包缺陷和两个已披露的UI语义缺陷；前者必须先修证据闭集，后两者适合另行体验/交互收尾，不应回写为本轮已通过。

## 2. 独立实测

- `context_preview3d.rs` 已为 181865 B，SHA-256 `1df9286d9cbaddf9d216fc7d2851daef2359486f8a018e891c7b15b70f31ee6e`；`main.rs` 与 receipt 绑定一致。
- worker 的 final exe `64f72697841fc10de01ce56685625541ac438593190b075dd08121f7ad59bbca` 与 source binding 一致。
- 独立运行 `cargo test -p glyphweave-cli --offline --bin glyphweave`：**86 passed / 0 failed**。
- 独立运行 `cargo test -p glyphweave-core --offline --lib engineering_ -- --test-threads=4`：**82 passed / 0 failed**。
- continuation-3 manifest 自列文件逐项 bytes/SHA-256 匹配；receipt 关键报告、源码和exe绑定匹配；历史清单无漂移；clean package文件哈希与receipt一致。
- 本轮检查员没有重新执行 worker 的21项CLI矩阵或浏览器，故只接受其已保存的逐进程与截图证据，不把检查员单测扩大成新的矩阵/浏览器运行。

## 3. 已接受的实质闭合

1. 3D资产和字段合同已从前轮的部分实现推进到导出/audit共用规则：资源实际字节与声明哈希、生产者viewer指纹、HTML内嵌模型绑定、来源链、闭集/类型/拓扑约束等均有代码和负例。
2. 无高度记录不再用0.6/0.7/0.8常数Y；实体和道路有实际三角面；地形使用索引三角求交。
3. CLI矩阵21项逐进程记录，清洁导出确定性、阶段归属和保护根/哨兵用例均有报告原文。
4. WebGL证据包含真实WebGL2、ANGLE/NVIDIA渲染器、shader link、GL错误、双表面、三次重载、page.mouse实体命中、独立地形oracle、2×2斜面fixture、坏包拒绝与当场截图。

## 4. 必须修正的证据问题

独立递归检查发现 continuation-3 manifest 的 `files[]` 有106项，但磁盘仍有一个未列文件：

`packages/clean-3d/manifest.json`

worker manifest只列了该包的五个文件，虽然receipt.cleanPackage和report都引用了这个manifest。manifest的顶层excluded只排除了本轮根目录的manifest、receipt、review，不能推导为排除嵌套package manifest。因此worker报告中的“0 unlisted”不能原样接受。

这不是产品源码缺陷，但会使“递归清单0漏列”结论失效。不得修改已封存continuation-3以补写；在新R1证据目录重新生成闭集manifest/receipt，明确每个排除项，或把该嵌套manifest作为正常文件列入清单。同步更新报告哈希绑定，保留continuation-3原状并披露这一纠正。

## 5. 已披露的UI缺陷（不隐瞒，不扩大为几何失败）

- HUD的“no height / not positioned / not snapped”数字只统计实体/道路；节点/access没有高度，但旁边另显示“observation-only records 22 (nodes 22)”，其中实际为4个节点+18条access，标签不准确。
- 真实实体命中后，`window.__PREVIEW3D_STATE__.selectedFacts`和体块高亮已更新，但截图中的`#selection`仍是“nothing selected”。这不否定page.mouse命中几何证据，却会让用户误以为选择失败。

这两项不应偷改后继续使用旧exe/旧矩阵。若要修，必须重新构建并绑定新包，按剩余额度做聚焦复验；或者明确把它们转入后续G28体验任务。

## 6. 预算与下一步

按worker本轮台账：cargo 9/10、CLI 21/23、browser 4/7、aux 7/8，剩余cargo1/CLI2/browser3/aux1。此前失败启动和迭代已经计入，不能重置。检查员本轮cargo2（86项CLI单测、82项core回归）、CLI0、browser0，另列不占worker额度。

先做不改产品的R1证据闭集修复；独立验收在收到修正后的manifest/receipt后再裁定。UI两项不自动并入本轮，因为会使最终exe/包/矩阵证据全部需要重新绑定，剩余额度不足以重跑原21项矩阵。

**A冻结、B delivered_for_inspection待独立验收、C不推进。**
