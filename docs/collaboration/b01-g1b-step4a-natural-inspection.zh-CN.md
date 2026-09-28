# G-1b step4a局部自然诊断独立审查（2026-09-19）

## 裁定

**accepted-bounded-negative**。固定G-1b北/南候选在既有BC保护格(270,171)/(270,188)分别6509→2390、6991→2390，足以否定它们各自的旧支撑保持性。两个x268对照6502/6987保持，不构成完整世界不变证明。无真实增量，B partial，C不推进，A不变。不重复运行G-1b。

## 核验

清单10/10、receipt6/6匹配，预约文本与run.json逐字一致。使用既有只读table-hash工具重算auto_roads原始字节BLAKE3，与运行记录3df46c4b9c0e882f5e0b2bf042049d61da2ed32c202cf8fb5492b19864e12c84一致；不是仅凭619692字节判定源码相同。生产前缀仍81685字节/ba6deb959c3e3b913ed2c65a887f4860b1cddf505cfd2ae2fb71758997224d34。

逐行检查四点natural/raw/clamped/delta/changed/decision一致，identity映射和base+candidate拼接符合代码；AB/BC读取保存G8 profile逐点比较。检查员一次no-run后仅exact运行新增g1b_preflight_natural_cell_plumbing，PASS；无自然重跑或源码修改。证据目录G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g1b-step4a-20260919/。

## 非阻断事项，随下一整包补记/修复，不单开修订轮

1. 南格的blendErasedSources包含index49、road-network25-26、road_pad、priority10；北侧主要来源为p1住宅pad。不能将两侧概括为仅p1 pad，但不影响full-target覆盖旧环填方的结论。
2. applyTerrainCarvesCalls=8仅对应四点raw before/after的直接调用；g1b_frontage_clamped每次也调用apply，因此点值阶段合计16（不含准备叶子内部调用）。必须明确计数范围，未instrumented的内部调用不称0。
3. g1b_natural_cell直接调用g1b_decide，而未像g1b_probe一样断言decision.valueMm等于生产rawAfter；本轮保存的四条确实相等，下轮将该断言接入实际管路。
4. 自然入口在构造payload之前assert elapsed<90s，未把已验收g1b_publish的最终门接到写文件路径。本轮测得2.432s，现存结果无失效证据；下轮将计算后、序列化后、写出前检查接入实际发布路径，过期标incomplete，不能只留孤立合成测试。
5. 无限cap应按已验收schema的null+infinite标识，而非新helper中的-1哨兵。本轮四点cap均有限，不影响结果；随管路修订同步统一。

## 批量推进建议

停止同一全长平坦连接的反复收窄试验。若继续，就一次回答：不覆盖道路肩部高台的候选生成规则，能否同时保旧支撑并获得真实access？建议两个固定、无生产语义改动的测试专用反事实方案：原connection矩形裁到自身parcel，或去掉connection组件。二者不改pad/apron/实体位置/2390/道路/terrain合成/审核，仅改变候选生成列表；都不预判可行。

后续整包任务写在ds-v41-b01-g11-connection-decision-bundle-task.zh-CN.md。该文件为可转发授权文本，本次检查不执行。任务将实现、合成、预注册、一次自然批次、完整受影响支撑/pad/条件式access、归档合并；用户仅在越界或任务完成时介入，不为普通合成失败或预期拒绝反复审批。