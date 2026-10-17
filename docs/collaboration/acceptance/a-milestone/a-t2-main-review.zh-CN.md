# A-T2主Agent验收：accepted_candidate，最大图等价待验

日期2026-09-08。主Agent复核三个候选hash与Terra交付一致：ecology.rs a7ccde4c549c00d548692b6d47487d846960b5649672d64f7890c9749df442c6；landcover.rs c11cc8fde436d99f56e770d89707cb9032ccba24938c3f15aa14775a40e72337；ecology_spatial.rs ab2204b5537378c84d9e81d84a9cdd25f1abb90dd33796eda5d369339013d1da。

主Agent独立运行：cargo test --manifest-path bevy/Cargo.toml -p glyphweave-core settlement::ecology:: -- --test-threads=1 --nocapture，8/8退出0；settlement::landcover::同样运行，11/11退出0。日志G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/review/a-t2-ecology-main.log及a-t2-landcover-main.log。

代码审查：32m空间桶仅筛保守障碍候选，旋转包围盒按原SAT尺度/旋转构造，边界向外pad；过大/非法几何进入spill，过大query回退全扫描；候选排序恢复输入顺序。每patch筛选后的候选仍用原choose_ecology_position和完整SAT/water/overlap谓词。verify从最终实体重新索引，不共享生成缓存。候选/层预算、seed/ID、结果排序未变；landcover只有测试计数器增量。

独立测试观察8类场景输出逐字段/序列化hash相等。旋转场景生成SAT 950175→927，verify 44075→43；部分阻挡生成4613525→4501，保留22实现/21缺额。计数下降是真实测试事实，不以其替代6×10km性能证明。没有植株时索引构建可比线性额外花费，未隐去。

接受窄实现候选；A-05仍需最终真实世界同输入线性/索引对照和600s/8192MiB证据。线性参考目前cfg(test)，后续需显式ignored真实场景等价入口（从源scene BufReader，不整JSON字符串）供总控单次串行执行；不得默认在全workspace中跑大图。

未跑最大图/release/完整workspace，本轮不修原生态缺1株，不改密度门禁。A-T1/A-D1冻结，未提交或推送。Terra可在新的独立T3写集推进身份问题，T2最大图证据留总控最后集成。
