# G34-R5 独立复核与限定收口

日期：2026-09-28。结论：G34-R5 的当前交付证据达到 `delivered_for_inspection`；不替独立最终裁定宣称“G34无条件通过”。G35产品实现未启动。

本复核读取R5报告、r5-a01结果、r5 smoke、R5 manifest/receipt/delivery、verify stdout、r5像素证据、生产绑定和账目；重新运行只读 `verify-delivery.mjs` 与 `final-check.mjs`。没有修改产品、R5目录或旧证据，没有运行cargo、产品CLI或新的产品验收Chrome。

## 1. 独立确认的成立项

- `runs/r5-a01/results.json`：29个required scene，29 pass，0 fail，0 timeout，missing=[]，duplicate=[]，exitCode=0。
- 最终包为 `g34-terrain-visibility-6`；R5 receipt把包内viewer与当前源码绑定，model hash保持 `922d404c...`；生产记录声明cargo test/build/core protection与两次export/audit全部成功，c01/c02六文件确定性一致。
- `calibration/r5-clip-pixels.json` 是新版结果，不是R4旧文件覆盖：triangle、obliqueQuad、hexagon、subCell均有内外RGBA；windowEdge明确 `outsideSampleExists=false`、`outsidePainted=null`，只验内点，没有把不存在的外点写成通过。`allOk=true`。
- 原子快照的最终验收结果包含冻结对象、256 cells、逐类聚合一致、hidden cell IDs与DOM IDs一致、snapshotId绑定和50次逐格digest稳定；这些是本轮结果，不用R4/R3结果替代。
- 两尺寸门槛已按合同分开：1280×800为0.831≥0.80，960×640为0.7877≥0.75；交互面积另报，未把960错误套用0.80。
- 坏包测试破坏实际内嵌模型块，外部json保持有效；坏包无hidden rect，恢复后model/snapshot恢复。几何字段使用4096/7938/23814/7，8022禁用。
- 闭集独立复核：manifest 1178项，missing/drifted=0；核验器exit0、`ok=true`、`problems=[]`；核验前后全目录二次枚举 identical=true；receipt/delivery/manifest三向绑定成立；G33基线六文件零漂移。
- `final-check.mjs`为只读输出；没有发现R5 verifier在控制流写文件。

## 2. 不阻断本次G34交付、但必须保留的限定

### 2.1 Chrome子进程退出码未观察

R5 ledger中Chrome为attempt=1、observed=0、success=0、failure=0、unobserved=1。不能把它改写成Chrome成功退出；但同一R5 acceptance driver自身有可观察exitCode=0，29/29结果已落盘，因此这是账务可观察性例外，不推翻浏览器检查结果。后续统一记录Chrome关闭/退出状态。

### 2.2 16个场景trail为空

R5结果中16/29场景的`trail=[]`，但这些场景仍有begin/end、stage=done、verdict和detail；R5合同要求每场景边界记录，并未要求每个纯读取场景必须有非空动作轨迹。因此这不是R5漏观测，也不是把pass改成fail的理由。下一版driver应对纯读取场景写至少一个明确action，提升可审计性。

### 2.3 历史闭集缺口不回写

R5 verifier正确保留：g34-r1有具名漏列`confirm-delivery.mjs`，g34-r3/r4没有逐文件manifest，早期台账存在继承漂移。它们是历史例外，不是R5闭集漂移；不得为了“全历史整齐”修改旧目录，也不要求再开G34文档轮次。

### 2.4 R5期间的driver失败尝试

R5账目记录2次失败driver和2次成功driver；成功整合矩阵仍是可接受的observed run。失败尝试需保留，不能清零或从账目删除；它们没有污染最终R5闭集。

## 3. 对执行质量的最终评价

R5积极完成了上轮七项未做好清单：新增像素结果、三路deadline、driver入账、生产顺序闸门、create-new audit目录、finally结果写出、闭集核验。尤其没有把R4旧失败文件冒充R5证据。

仍有三项可改进但不阻断本次收口：

1. Chrome退出码仍未观测；
2. 16个纯读取场景没有非空stage trail；
3. R3/R4历史没有逐文件manifest，且继承台账漂移只能作为已知例外。

没有证据表明存在故意造假或主观懈怠。G34现阶段应停止追加收口轮次，保留上述限定，进入下一阶段范围闸门。

## 4. 资料

R5交付：`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g34-r5-final-browser/`
本复核证据：`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g34-r5-20260928/`
G35范围闸门：`G:/difyllmwiki/GlyphWeave/docs/collaboration/dsh-b01-g35-scope-gate.zh-CN.md`