# V4.1 B Review R6：完整支撑与非零持久化收口

先读 G:/difyllmwiki/GlyphWeave/docs/collaboration/b-review-r5-inspection-20260916.zh-CN.md。

保留R5已通过的道路设计重建、固定边身份、中心点扰动拒绝和内存非零正例。本轮不新增城市profile/验收框架、不重复大图、不要求强行解出原自然24个地块。

## 1. 一次性覆盖完整道路支撑

现有joint对base road只检查中心。请复用 G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/worldgen/road_network.rs 现有完整道路/支撑面审核，必要时提取窄的可复用生产函数；不要复制简化算法或增加固定采样点列表。

覆盖固定base与扩展道路的实际纵横支撑范围，含宽度/端部/连接约定；与自然、设计、final/保护/fill口径一致。设计mesh verify不能替代final支撑核对。

正例0m和合法0.1m工程继续通过；以下单点±1mm扰动必须由实际工程审核明确拒绝：中心、非中心纵向/横向点、支持范围边界/端部、扩展road支撑。测试用从geometry派生的点集，不硬编码只检查检查员坐标。支持范围外合法无关地形变化不应被无理由拒绝。

原两个复现点[65,160]与[64,161]作为具体回归保留；原mesh+1m/删base edge负例仍通过。新增报告不得通过去掉final输入或不再称audit来规避合同。

## 2. 完整保存现有非零正例

直接使用nonzeroEngineeringControl，不再从平地built_artifacts生成另一份同内容目录。

保存scene、zones/water、请求、可信base及义务、自然和final高程（标编码/有效范围）、road/site/parcel/entities/surface及hash索引。自然与final应有真实100mm级差量且原自然不变。

读回入口从文件恢复所有必要来源，不调用fixture()重新生成隐藏输入；用生产工程/来源审核重建，验证24/24、yardUnresolved=0及两宽度连通关系。删yard/改final支撑+1mm/错natural source/删base edge应明确拒绝。

这仍是integration fixture，不是原自然案例或generate-world成功证书。包可以很小，不需要完整世界/preview或新CLI特权入口。

输出路径由测试参数/新临时目录或唯一run目录指定，文件写入拒绝覆盖；既有目录输出应返回明确错误，不能截断旧文件。原R4/R5证据只读。默认单元测试不得隐式改worker历史交付目录。

## 3. 统一固定端口和需求接线

从可信fixed node map按坐标/身份确定**每条**候选边的固定起点和终点高程，不能仅保护数组第0条。冻结原8000mm节点不改；只给新增内部节点选择候选高程，共享节点一致。

加入数组重排/两个独立固定接入根的定向测试（包括原[224,96]和[288,224]根）；请求顺序不应导致固定义务消失。

统一evaluate_style_candidate与自然runner的service/需求传递，不能一个传原settlementProgram另一个传None。自然最终审核成功分支使用生产final/完整道路支撑，不能仅测试中正确。

本轮先做这些接线与测试，不再重复原四档全网高程扫描。若新增合法候选值得运行，最多一次原自然内存预检（非bake），最多8个不重复状态，记录与R5相比真正改变的几何/节点来源、拒绝原因；没有新方案就写not_run，不重复日志凑完成。

## 4. 白名单和执行

允许：bevy/crates/core/src/worldgen/road_parcel_feasibility.rs、road_network.rs及worldgen.rs的必要函数复用/接线；相关road_profile/road_extensions如确需提取已有严格支撑函数可最小修改，不放宽原谓词。测试/harness和本轮新work/结果文档允许。

只定向测试/小集成；单项120s，首次构建可独立300s，不跑完整core/workspace/release、512m generate、gate、大图或浏览器。不复验与本轮无关的profile/生态/材质功能。

交付 docs/collaboration/b-review-r6-result.zh-CN.md、bevy/work/collaboration/ds-b-r6/receipt-r6.json和完整非零包：源码/输入hash、实际命令/退出码、所有正负例、源自然不变、旧R4/R5证据恢复/未改核验、缺额和未执行项。

完成交ready_for_review或partial，等待主Agent；不签收B、不推进C、不Git add/commit/push/reset/clean/checkout/switch/stash，不启动子agent，不改F盘/A冻结材料/原自然需求/工程阈值。
