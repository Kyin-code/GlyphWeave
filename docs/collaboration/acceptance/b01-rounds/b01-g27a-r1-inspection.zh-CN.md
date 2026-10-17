# B01-G27A-R1 独立检查（2026-09-22）

## 1. 裁定

**G27A-R1 浏览器补测成立，G27A 路径A显式只读交接入口至此收口；G27B真实3D/WebGL仍未启动。**

本轮没有发现新的产品逻辑问题，两个先前驱动缺陷均由修正后的驱动实测闭合。

## 2. 独立只读核验

- G27A-R1清单：**11/11** bytes+SHA-256匹配，0未列文件。
- receipt对manifest、report、progress、read-only-verification、command-log、browser-acceptance六项绑定成立。
- G27A、G26、G26R2、G25、G25R1、G24旧目录均保持；G27A产品源码绑定匹配。
- 本次未修改产品源码、旧证据、固定来源，未运行cargo/CLI。

## 3. 浏览器补测结果

### Phase 1：坏包到合法包

**10/10通过**：

- 坏包端口拒绝；
- 一次读取确认无入口状态、无viewer状态、无iframe、空body、无旧模型；
- 坏包页面关闭后才打开合法入口；
- 合法入口modelLoaded、status model hash、entities/roads/access计数均正确；
- 没有旧对象残留。

修复了G27A p02的`chrome-error://`页面复用问题。

### Phase 2：同包二次选择

**7/7通过**：

- 两次model hash一致；
- 实体4、道路3、access18、观察标签18均稳定；
- 徽标在`/package/preview.html` iframe内部查询，两次均为36（2×18）；
- access列表两次均为18，无重复对象。

修复了G27A p03错误查询入口父文档而非iframe的问题。

总计：**17/17，0失败**。坏包截图为浏览器自身连接错误页，没有伪造包页面截图。

## 4. 范围边界

- G27A路径A完成：显式只读项目交接入口；
- G27B真实3D/WebGL未开始；
- 不接ST18、默认世界、G23三场景矩阵、WebGL/GPU、工程build、route、bake、realize或发布；
- G25/G26的canvas2D仍不改名为真实3D。

## 5. 下一阶段

G27A已经收口。下一任务是先前已发布的独立可选 **G27B：路径B真实3D/WebGL上下文预览迁移**，但只有产品明确要求真实3D/WebGL并确认新合同和预算后才启动。未获明确要求前不运行、不改源码、不消耗G27B预算。

任务书：G:/difyllmwiki/GlyphWeave/docs/collaboration/dsh-b01-g27b-context-preview-3d-task.zh-CN.md

**A冻结、Bpartial、C不推进。**
