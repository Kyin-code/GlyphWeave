# B01-G25R1 独立检查（2026-09-22）

## 1. 裁定

**路径 A 合同成立：当前 viewer 被限定为“只读 2D 上下文投影”，G25R1 可作 scoped delivered_for_inspection；G25/G25R1 整体仍不宣称 WebGL/3D、工程可行性、路线安全或 G23 三场景矩阵验收。**

本次没有发现驱动修复后的新产品逻辑阻断。坏包截图缺失和其他未声明观察保持为证据边界，不用复制另一张截图来凑证。

## 2. 独立只读核验

- G25R1 manifest：**18/18** bytes+SHA-256 匹配，0 未列文件。
- receipt 对 manifest、report、progress、read-only-verification、command-log、browser-acceptance 六项绑定全部成立。
- G25R1 五个产品源码与 G25 source-binding 逐项匹配，0 drift；G25/G24 旧目录和 G22/G23/G23R1 冻结证据未改。
- browser-acceptance、browser.log、四张截图均实际存在并被纳入清单；m03 两张截图同字节情况如实保留。

## 3. 路径 A 合同判断

交付明确声明：

- natural/final 是独立可切换的二维表面投影；
- 实体、道路、节点和 access path 是保存场景事实；
- access 始终标注 `sourceObservationOnly` / `notReverified`；
- 页面不提供 WebGL/3D 几何渲染；
- 页面不提供工程可行性、施工安全或路线通过性证明。

这与 G25 任务范围一致，不应把 canvas 2D viewer 写成 G23 Three/WebGL viewer 的替代品。

## 4. 浏览器证据

worker 的三次修正后浏览器会话共 **37/37 checks passed**：

- m01：双 surface 绑定、实体/道路/节点/access 数量、观测标签和禁止性正面声明；
- m02：真实 `page.mouse.click`，命中目标道路并返回身份、位置、旋转、scale、savedBounds；
- m03：坏包实时拒绝、空视口、错误提示，以及干净包三次 reload 无重复对象。

我没有重启浏览器；因此上述属于 worker 本轮实测，我只做了证据和哈希复核。坏包状态没有截图，只有实时断言，不把 m03 的干净重载截图当坏包截图。拾取只验证已知投影中心，不声称其他坐标或半径行为已穷举。

## 5. 剩余边界

1. G23 三场景浏览器矩阵、WebGL/GPU 断言仍未做；
2. 真实 3D/WebGL 若成为产品要求，必须另立路径 B 迁移任务；
3. 坏包截图不补造、不复制；若后续需要图像证据，必须在新授权下重新运行浏览器；
4. 未接 ST18、默认世界、SceneIndex 默认消费者、工程 build、route、bake、realize 或发布。

## 6. 成本和停点

worker 本轮浏览器 3/3、辅助驱动1/3、cargo/CLI均0；本次检查员只读核验，未运行 cargo/CLI/浏览器，未修改任何源码或交付证据。

G25R1 可冻结。下一步不再修 viewer 逻辑，改做一个新的、明确可选的项目入口/目录集成任务；默认世界和 G23 viewer 仍冻结。

## 7. 证据

检查员目录：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g25r1-20260922/。

**A 冻结、B partial、C 不推进。**
