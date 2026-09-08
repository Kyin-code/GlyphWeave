# A-FINAL-R1 独立复验与下一阶段裁定

日期：2026-09-08，Asia/Shanghai。角色：检查员／项目管理者；不代写DS业务实现，不启动协助agent。

## 1. 结论

R1有实质修复：F1 header绑定、F2 auditInput绑定及原复访实体／height分支漏洞在本轮复验通过。接受这些修复的候选范围，不再要求其重写。

总体仍为 `ready_for_review / needs_changes + collection_pending`，不是 `milestone-A-accepted`：
- 实体实例观测还有一处实际存在性漏洞；
- 最终引用检查还有一处跨文件world hash矛盾未拒绝；
- 新字段下最终240块采集、真实进程性能测量尚未补齐。

`final-acceptance-check-r1d.json.status=pass`只说明其当前实现检查的候选引用通过；它接受measurement_pending和旧完整观测，本身不是里程碑证明。DS这次保持ready_for_review并承认pending是正确的。

## 2. 本次独立复验结果

所有本次检查材料位于：
`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-r1-20260908/`

| 检查 | 结果 | 材料 |
|---|---|---|
| 提交收据内8项主要产物hash | 全部相符，包括两个不同runtime、world、parity、candidate、closure、源码清单、measurement-pending | identity.json |
| 源码清单605项 | 当前逐项hash相符，无缺失 | identity.json |
| 与原生成源码清单差异 | 未发现worldgen/settlement变动；主要是audit、HTML观测、脚本和文档 | source-delta.json |
| Node协作回归 | 208/208，退出0 | node-tests.log |
| 候选引用checker复跑 | pass，候选仍ready_for_review | closure-control.json |
| 小world独立audit控制 | 新审计runtime退出0 | prior-defects-retest.json |
| 原header heightFile单字段篡改 | 新审计runtime退出1 | prior-defects-retest.json |
| 原紧凑source入口篡改 | 消费者退出1 | prior-defects-retest.json |
| 保持source字节长度的入口坐标+1篡改 | 明确audit.hash SHA256 mismatch，不只因为长度变化 | same-bytes-hash-rejection.json |
| 新2块浏览器原观察重放 | pass | browser-checker-adversarial.json |
| 全部revisit实体清空、height错hash（独立变异） | 各自fail，原分支问题已修 | browser-checker-adversarial.json |

本轮未重新编译Rust、未跑完整workspace、未生成最大图或启动浏览器。Rust F1控制/反例直接运行提交的新审计exe；独立小world仅在检查员副本里变异，原世界不改。

## 3. R2只需先修的两处关键问题

### R2-1 实例ID仍可脱离实际几何／有效实例

直接提取当前app.js的__previewStats函数，用仓库vendored Three.js的真实Scene/Group/InstancedMesh运行（无WebGL、无浏览器）：

- 一个没有任何children的Group，仅设置userData.entityInstanceIds=['synthetic-tree']，收集器仍报告该树存在；
- 一个实际count=0的InstancedMesh，设置同样元数据，仍报告该树存在。

证据：actual-collector-counterexamples.json。

根因：遍历所有object时，无条件信sourceEntityId或entityInstanceIds数组；不检查Group内对应可绘制子节点，不检查有效instance索引/count。当前树木批组把列表挂在Group上，此时“遍历来自实际场景树”仍不等于“该ID有实际几何实例”。

第二层也未检查renderedEntityInstanceSources：我删除2块真实观察中所有该字段，evaluateEvidence仍返回pass（browser-checker-adversarial.json）。因此补采后即使来源字段为空也可宣称完整。

修复边界：不做像素级/摄像机遮挡证明、不改视觉，不要求复合建筑只一个mesh。允许一个树逻辑实例对应多个树干/树冠InstancedMesh，但必须记录稳定的逻辑ID到实际batch UUID/instanceIndex/count及group几何子节点的可核验关系。空Group、count=0、越界索引、缺失来源、重复逻辑实例必须拒绝。不要把一个树的多个组成mesh误判为多个实体；也不能用Set消除真正的重复根实例。

这属于A-07现有“实际实例/不漏不重”要求，不是新增渲染质量要求。先小控制修复，再补采240块，避免浪费完整巡检。

### R2-2 候选引用checker漏检parity.worldFile与world绑定

以真实final-acceptance-r1候选为控制；仅把parity副本的worldFile.sha256置为64个0，同时刷新候选中该parity文件的bytes/sha256，使文件完整性检查仍成立。checkAcceptance返回pass。

证据：closure-parity-world-conflict.json。

应在已有字节hash校验之外，核对parity.worldFile的规范文件身份/hash与candidate.world、实际world一致；保持worldRevision一致不足以代替字节绑定。最终浏览器观察相关header/parity/check报告也需要形成明确引用边，不能把任何同revision的报告当同一次证据。

不要把该工具扩成签名/编译认证系统；它依旧只校验内容一致性。把任务ID或measurement_pending硬编码成唯一合法值也不能供下一阶段使用：应允许新候选taskId，支持pending与有真实绑定过程报告的measured两种明确状态，closureStatus与milestone readiness分开输出；不能因改了字符串measured就通过预算验收。

## 4. 仍待补齐的运行证据，不应伪造

- 原240块观测缺新增source/instance结构，R1明确incomplete，正确。不得补写旧JSON使其看起来新采集。
- generation/gate原始采样确实未保存，R1明确measurement_pending，正确。旧267.164s/980MiB等仅历史自报，不算现成预算证据。
- 原r51 world仍可作为已存在内容候选。下一次大图生成只为补实际受控测量和再现，不是重做地图内容；保持原生成exe和manifest hash。
- A-09本轮仍按“库存与限制披露”处理，建筑来源和workerd元数据未核实不隐去，不因本次补采引入全依赖升级。

## 5. 下一阶段策略与资源授权

先完成R2-1/R2-2小修及2块实际实例控制，通过后允许按配套任务书顺序执行一次generation、一次gate与一次完整浏览器采集；这是本次明确新增授权，不引用R1的禁大图指令绕过。

运行必须串行，生成/gate各600s/8192MiB原预算不变；使用新输出目录，原world/runtime/证据不改。用原生成runtime生成新的measurement-world，再用原runtime在新world执行gate（其报告会写world，因此不能直接拿原冻结world跑gate）。

只更新新measurement-world内的派生preview文件为本次观测实现；记录adapter全量文件hash、更新前后权威文件不变。用既有新审计runtime生成绑定该新world的新parity。新world权威内容与原r51通过流式hash等价后，可复用原生态linear证据并附对应关系，无须再跑7分钟线性扫描。

一次完整浏览器采集前先小控制验证；若实际绘制/LOD不符，不能改需求或剔除ID，请停在准确失败位置给检查员。没有重复全跑的无限许可。所有运行结果都仍提交ready_for_review，由检查员最后逐项签收。

## 6. 流程说明

DS R1实际执行了release审计构建和小preview服务，范围比前任务书“先提计划再授运行”更宽；没有修改原world、没有大图重跑，且我当前只看到历史8099 preview。已保留成果但不将越过资源许可当作今后默认权限。R2以新任务书明确授权范围为准，自己的新进程按PID关闭，不碰历史8099。
