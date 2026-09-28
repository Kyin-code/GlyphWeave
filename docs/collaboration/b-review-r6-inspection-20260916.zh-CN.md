# B Review R6 独立验收：收口子任务通过

日期：2026-09-16（Asia/Shanghai）。主Agent裁定：**accepted-subtask（R6约定范围）**。

R3～R6这条“道路支撑、非零正例持久化、固定接点/需求传递”的修订队列关闭。B整体仍partial；A保持accepted；不签收B/C，不将合成fixture成功说成原自然案例或CLI生成成功。

## 1. 身份与独立检查

仓库G:/difyllmwiki/GlyphWeave，HEAD fb36dc4f9fdd38e6aa9c5a89676a65b96e5001b1。

回执：bevy/work/collaboration/ds-b-r6/receipt-r6.json，5371 bytes，SHA256 25741b53cd0ceaa7195cab925aca682f8b5d1c5fd70e66f73daff2bdd06351a2。
回执22项源码/文档/产物/日志/历史证据绑定独立核对全部匹配；旧R4/R5自然评估文件hash保持。

本轮证据目录：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b-r6-20260916/。

- 模块定向测试独立重跑：8 passed、2 ignored、0 failed，4.85s。
- 两个ignored集成有写输出副作用，本轮未直接运行；读取其实际输出和日志，并另编译检查员只读reader。
- 为reader链接当前代码，执行一次offline core库构建，9.72s；未构建release，未运行完整core/workspace。
- 独立reader不调用fixture()，从scene、zones、water、base、candidate、natural/final payload、development、landcover、entities、surfaces恢复实际输入；调用当前生产trusted/joint/development/landcover审核。

没有生成新512m世界，没有gate/最大图/浏览器；没有修改业务代码、调用子agent或Git写操作。

## 2. 非零包独立重读结果

包：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b-r6/persisted-nonzero-r6-final/，11份输入/产物。

- natural content hash：21a582757b863bac5e28a00b61433754db218e4235d53d67159628b02cf3d441。
- final content hash：98c1999f9f56e829bb6ad25b8498d408d8803d73303e4e28d379e9c8a5c13024。
- 有44132个采样点改变，最大绝对差100mm；并非所有道路/地面都均匀抬高0.1m。
- frontage requested/planned/realizedWithAccess=24/24/24，unfilled=0。
- parcels=24，buildingsRealized=24，yardUnresolved=0，requiredConnected=24，requiredUnconnected=0。
- 3条road edges含16m、12m扩展链；development/landcover读回检查无failures。全场未开发土地仍使其status=incomplete，这与局部正例通过不矛盾。

独立探针读取包中none水体定义；该包是明确单场景合成输入的integration_fixture，不外推任意多场景、水文或受保护工程能力。

## 3. 支撑和来源正负例

使用原包的内存副本，不修改原始文件：

- 10个道路支撑位置，分别±1mm，共20个反例均拒绝。包含中心、非中心纵横点、端部/边界、扩展道路，以及0.1m非零设计端点[126,160]和[480,160]。
- 支撑范围外[0,0]的无关+1mm变化允许，不做全场无理由锁死。
- extension design网格顶点+1m：源重建拒绝。
- 删除固定base edge：身份合同拒绝。
- 删除yard patch：独立landcover重建拒绝。
- 错误natural来源：拒绝。

原“中心点通过但偏移一米仍漏检”的阻断关闭。当前实现确实遍历道路支撑范围并用生产terrain carve计算期望final，不再只检查样例坐标。

证据：probe-results-expanded.jsonl、nonzero-readback-expanded.rs、probe-expanded-build.log；初版reader/结果亦保留。

## 4. 固定接点与需求接线

源码与定向测试确认apply_fixed_endpoint_levels按节点坐标匹配所有起终点，不依赖数组第0项；数组重排及两固定接入根测试通过。evaluate_style_candidate已解析并传递service program，与自然runner一致。

这些是本轮既定接口修复的验收，不声称所有未来坐标格式/扩展模式已全面审计。后续修改调用边界按影响面验证，当前不追加泛化加固清单。

## 5. 原自然场景与剩余B内容

R6自然runner记录4个计划尝试，仍全部plan_rejected；输出与R5自然结果hash相同。结合源码/执行日志可确认流程重跑并补了固定接点绑定，但没有新的可行道路/地块见证。

- 原自然场景：unresolved。
- 原自然CLI 24地块、两级实际供给：not_proven。
- B03大面积用途/地表缺额、B04 profile展开/公共服务关系、B05最大图和全域浏览器：仍未完成。
- A六类内容失败和两项许可未核实继续披露，不因R6通过消失。

不再要求重复这四档高程得到相同拒绝，也不要求把原自然场景改成简单输入冒充通过。把它保留为困难回归/后续容量规划问题。

## 6. 后续安排

当前不再派R7同类审核修订。进入B04-C1“profile真实编译与生成接线”，利用已稳定的地块/道路/工程合同获得实际生成能力增量；原自然困难案例单独保留，不阻断可独立开发的profile编译工作，也不被替换为成功案例。

任务：G:/difyllmwiki/GlyphWeave/docs/collaboration/ds-v41f-b04-c1-task.zh-CN.md。

当前验收的是记录hash的未提交工作区子任务；没有自动提交/推送，也没有修改A标签。
