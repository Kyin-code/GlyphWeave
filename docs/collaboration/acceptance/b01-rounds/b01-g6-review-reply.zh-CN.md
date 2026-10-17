本轮暂不能签收为“完整诊断完成”，状态needs-changes；保留28次no-cut首拒绝与4次existing_support_changed的分布，但不批准H1抬高B。

发现实质性对象错误：sample_entity_for忽略sample，返回整条BC road，导致32行buildingFootprintM都写112×12、supportCheckRange都写[174,172,290,188]。真实建筑是12×10，按各自中心加2m padding；北侧yaw180也被错写0。请调用生产candidate_site得到site.entity()，不要用道路fallback，并先加合成回归。

696只是扫描到的首个违规点最大差值，不是全支撑最大缺额。检查员离线读取冻结natural：st66南侧真实support[214,196,230,210]，design147，最高自然1149@(230,210)，缺额1002；首个点只差8。即使假设所有pad均匀抬696，28条no-cut里仍有20条不通过。1002也不能直接当成B增加量；提高B不等于把内部最小包络profile整体平移。

原始RejectionDetail还含四条支撑变化的conflicting_entity/before_mm/after_mm，你的rows把这些丢掉了；planProfiles也只保存len/first/last，不是完整profile包。请保存完整raw trace和生产design/profile，补正确辅助范围与全支撑极值，旧run/receipt不覆盖。不要把4条支撑变化说成未经证实的拓扑冲突，也不宣称no-cut是32条的唯一原因。

按ds-v41f-b01-g6-r1-diagnostic-repair-task.zh-CN.md一次性修正：先小合成测试，再最多一次原两态诊断补证；≤4measure/32attempt、120s，realize/bake均0，禁止任何新B/C取值或放宽合同。完成停止等复验，B partial/C不推进。
