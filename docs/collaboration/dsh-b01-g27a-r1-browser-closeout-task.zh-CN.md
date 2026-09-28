# B01-G27A-R1：显式交接入口浏览器断言补测

日期：2026-09-22。

依据：
- G27A独立检查：G:/difyllmwiki/GlyphWeave/docs/collaboration/b01-g27a-inspection.zh-CN.md
- G27A交付：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g27a-context-preview-handoff/

## 0. 唯一目标

只补测G27A两个已修正的浏览器驱动断言：

1. 坏包加载失败后使用**新页面**打开合法包，确认失败选择不会留下旧模型或污染后续合法入口；
2. 同一包选择两次时，在**iframe文档**中核对徽标计数不重复。

本任务不修改G27A产品源码，不修改G26/G25/G25R1/G24旧证据，不启动G27B。

## 1. 浏览器矩阵

一次浏览器运行、一个辅助驱动，建议在同一浏览器进程内使用独立页面：

### Case A：坏包→合法包

- 使用G27A入口拒绝的坏包；
- 记录拒绝/端口未绑定；
- 不复用`chrome-error://`页面；
- 创建新页面加载合法G27A入口；
- 记录入口页、status、同源iframe、G25 viewer和事实字段；
- 明确断言坏包没有旧模型残留，且合法包正常显示。

### Case B：同包二次选择

- 在iframe文档中查询徽标，而不是入口父文档；
- 同一合法包选择两次；
- 断言model hash相同、实体/access/观测标签数量稳定、徽标数量不增加、没有第二个服务或重复对象。

## 2. 证据规则

- 保留G27A p02/p03失败原始日志，不覆盖或删除；
- 记录每个page/context的创建顺序、URL、服务PID/端口和实际断言；
- 如果浏览器再次失败，准确保留partial，不改拾取半径、不改端口合同、不把HTTP旁证冒充浏览器通过；
- 不需要坏包页面截图；连接错误页可作为浏览器环境记录，但不能称为包页面。

## 3. 预算与范围

建议新授权：

- 浏览器：1次；
- 辅助驱动：1次；
- cargo：0；
- CLI/入口进程矩阵：0；合法入口前置服务如需启动，单独记录，不计CLI矩阵；
- WebGL/GPU、G23三场景、ST18、默认世界、工程build/route/bake/realize/release：0。

## 4. 停点与交付

新目录：

`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g27a-r1-browser-closeout/`

顺序：driver→browser log/acceptance→截图（如真实存在）→report/progress→递归manifest→receipt→只读核验。旧G27A目录不覆盖，manifest生成后不再修改绑定文件。

正分支只能声明“G27A显式只读交接入口的浏览器断言补测完成”，不能声明真实3D、默认世界或工程安全验收。

**A冻结、Bpartial、C不推进。**
