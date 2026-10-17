# A-T2 Terra：生态障碍候选查询优化

- 任务：`A-T2`；日期：2026-09-08（Asia/Shanghai）。
- 状态：`ready_for_review`，仅窄候选验证；不声明里程碑 A 或整体世界质量通过。
- 唯一仓库：`G:/difyllmwiki/GlyphWeave`；HEAD 基线 `6020fbe2713e8c79c1b2934b244402dc93e1df14`。
- 本任务持独占窄 Rust test 槽，下面所有 cargo 测试已串行执行完毕；无后台句柄。

## 1. 写集及实现

只写这三个 Rust 文件及自己的结果/work：

- `G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/settlement/ecology.rs`
  SHA-256：`a7ccde4c549c00d548692b6d47487d846960b5649672d64f7890c9749df442c6`
- `G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/settlement/landcover.rs`
  SHA-256：`c11cc8fde436d99f56e770d89707cb9032ccba24938c3f15aa14775a40e72337`
- `G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/settlement/ecology_spatial.rs`
  SHA-256：`ab2204b5537378c84d9e81d84a9cdd25f1abb90dd33796eda5d369339013d1da`

- ecology.rs：通过 `#[path = "ecology_spatial.rs"]` 引入辅助，**未改 mod.rs**。
  生成阶段按完整 patch 范围取候选；候选仍传入原 choose_ecology_position。
- ecology_spatial.rs：32m 格子保存障碍的原输入 ordinal；旋转、scale 后的
  保守 AABB 用于筛选，候选排序去重恢复输入顺序。实体最多占 256 桶；长道路、
  超大/异常边界放入 spill，每次都检查；查询超过 4096 桶或数值不可靠则全量回退。
  桶范围向外 padding 并含边界，保留切边附近候选；原 SAT 再判断真碰撞。
- verify_contract：从传入的最终实体**重新独立建索引**，逐植物完整 footprint 查询，
  不复用生成结果或缓存。非法植物、越界和水体失败仍先于障碍检查。
- landcover.rs：仅新增 `cfg(test)` 的 thread-local 实际 SAT 调用计数器；
  发布运行没有计数开销。没有修改候选位置算法或 SAT 公式。

## 2. 保持不变的边界

真正线性参考 `ObstacleQuery::linear` 只在 `cfg(test)` 存在：既不建桶，也不作
空间筛选，每次返回全障碍原始序列。线性与索引运行同一原放置/审核逻辑，只替换
障碍候选来源，不使用“索引自证”。发布入口固定走 indexed，没有全局开关/环境修改。

以下保持原样：density/stratum 配比、seed、ID、patch/候选/结果排序、preferred 点、
1m 网格 ix/iz 遍历、尝试上限、完整 footprint、水体谓词、植物间不同宽度 overlap、
失败 reason、各级 requested/realized/unfilled 预算以及既有审核顺序。

植物相互检查仍只扫描同 source patch 的局部列表，未替换为网格占用；没有跨 patch
新增约束，也没有删植物/降低密度/修复原最大图“缺 1 株”。
辅助只是保守 broad phase，不能声称桶占用即几何碰撞。

## 3. 等价及负向证据

8 种小 fixture 场景：每场景 32×32m 地形、4 个 <=16m patch、1024 个远障碍，
再加入不同近障碍/长道路/异常输入。比较 EcologyRealization **完整 PartialEq、
序列化字节严格相等及 BLAKE3 输出 hash**，不使用 matches 的浮点容差放宽等价。

- 空间边界：负 origin、32m 桶线、切边/内外微小偏差、不相交近邻、旋转薄障碍、
  scale、长道路、超大 AABB、接近 i32 极限的大 origin。
- 保守回退：NaN/无限 yaw、零尺寸、负 scale 的原阻挡语义；非法/巨大查询全量回退。
- 搜索等价：多种植物宽度（0.8/1/1.5/2.5/4/8/16/17/NaN）、不同 occupied 宽度、
  preferred/边缘点、river 水体、搜索耗尽；输出坐标/具体 Err 相等。
- verify 负向：重复/缺失/错误来源/非法尺寸/NaN/植物重叠/预算错误/错误 rejection。
  indexed 与线性必须返回**同一个首个错误字符串**。
- 在有效生成后增加最终障碍，独立 verify 必须报 `ecology plant intersects hard entity`；
  水体 control 显式命中并先于障碍回调拒绝，避免水体全为空的假覆盖。
- 全阻挡仍 `0 realized / 43 unfilled`；部分阻挡仍 `22 realized / 21 unfilled`，
  顺序与预算逐字段相等。完整局部 raster content_hash 前后不变。

### 实际 narrow-phase 调用数量

以下是测试计数器记录的原 SAT 函数调用数，不是估算，也不是最大图性能测量。

| 场景 | 植物/缺额 | 生成 linear → indexed | verify linear → indexed |
|---|---|---|---|
| 稀疏远障碍 | 43/0 | 797696 → 0 | 44032 → 0 |
| 旋转近障碍 | 43/0 | 950175 → 927 | 44075 → 43 |
| 全部阻挡 | 0/43 | 8497250 → 8290 | 0 → 0 |
| 长道路 | 43/0 | 1133650 → 1106 | 44075 → 43 |
| 超大 AABB | 0/43 | 8497250 → 8290 | 0 → 0 |
| 混合层与 river | 34/0 | 992256 → 0 | 34816 → 0 |
| 异常几何保守阻挡 | 0/43 | 8497250 → 8290 | 0 → 0 |
| 部分阻挡 | 22/21 | 4613525 → 4501 | 22550 → 22 |

旋转近障碍场景输出 BLAKE3：
`e490116098ee0f49d7aff821a2433895f357bfe7e0defe909164ea8f08583757`。
两路径均输出 43 株且无缺额，indexed 实际保留非零检查（不是全删候选蒙混）。

耗时记录在机器证据中，含索引构建、debug 编译及测试计数器成本。
例如旋转近障碍生成 linear/indexed 为 69825/1411 微秒，verify 为 3488/1320 微秒。
**单次、小场景、人工稀疏分布，不能据此声称最大图加速倍数。**
无植物的 verify 没有 SAT 扫描，索引构建反而有额外开销（日志保留，不隐瞒）。

## 4. 命令、退出码、日志

工作目录 `G:/difyllmwiki/GlyphWeave`。

- `cargo test --manifest-path bevy/Cargo.toml -p glyphweave-core settlement::ecology:: -- --test-threads=1 --nocapture`
  - 退出码：101；首次 7/8；新测试预期写错既有拒绝字符串。随后仅纠正测试字符串，原生产拒绝原因不变。
  - 日志：`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/terra-t2/ecology-test-01.log`
- `cargo test --manifest-path bevy/Cargo.toml -p glyphweave-core settlement::ecology:: -- --test-threads=1 --nocapture`
  - 退出码：0；8/8；其中 2 索引测试、3 原生态测试、3 新等价/负向测试。
  - 日志：`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/terra-t2/ecology-test-02.log`
- `cargo test --manifest-path bevy/Cargo.toml -p glyphweave-core settlement::landcover:: -- --test-threads=1`
  - 退出码：0；11/11；原 SAT/旋转/大 origin、密度/mesh/来源伪造等回归保持。
  - 日志：`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/terra-t2/landcover-test-01.log`
- `node bevy/work/collaboration/terra-t2/check-static.mjs`
  - 退出码：0；不变块和测试计数证据提取。
  - 日志：`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/terra-t2/static-check.log`
- `node scripts/generate-tree.mjs --check`
  - 退出码：0；AGENTS.md 目录级树不需改写。
  - 日志：`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/terra-t2/tree-check.log`
- `git diff --check -- bevy/crates/core/src/settlement/ecology.rs bevy/crates/core/src/settlement/landcover.rs bevy/crates/core/src/settlement/ecology_spatial.rs`
  - 退出码：0；通过；未追踪新文件另检查尾空格。
  - 日志：`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/terra-t2/diff-check.log`

首轮失败是新负向测试臆写了“ecology rejection provenance mismatch”，原生产实现
实际为“invalid ecology rejection”；已读取原分支并纠正本方测试，未改生产 reason
或放松验证标准。该首轮 101 日志保留。

总控复验建议（仅窄模块，串行）：

```text
cargo test --manifest-path G:/difyllmwiki/GlyphWeave/bevy/Cargo.toml -p glyphweave-core settlement::ecology:: -- --test-threads=1 --nocapture
cargo test --manifest-path G:/difyllmwiki/GlyphWeave/bevy/Cargo.toml -p glyphweave-core settlement::landcover:: -- --test-threads=1
```

预期分别匹配 8 项和 11 项，零匹配不是通过。

## 5. 可审计材料

- 起始内容/hash：`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/terra-t2/baseline.json`，与本轮自己的 *.rs.before 副本对应。
- 机器等价/计数证据：`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/terra-t2/equivalence-evidence.json`。
- 静态不变块证明：`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/terra-t2/check-static.mjs` 和 `G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/terra-t2/static-check.log`。
- 单独机器回执：`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/terra-t2/receipt.json`。

静态检查确认 profile/累计预算、ID/seed/preferred 算式、结果统计/排序代码块原样；
landcover 删除新增的 test-only 计数器后与起始文件逐文本一致。

## 6. 未证实事项与资源边界

- 未运行 workspace/release、CLI gate、大图生成、真实 6×10km linear/indexed 对照，
  无 600s/8192MiB 最大图证明；由总控另授 RUN 槽。
- 未证明当前世界自然层/保护区全域一致、HTML 全域资源或整体 A-01～A-10。
- 未新增依赖/模型/网络服务；未运行 cargo fmt，更未格式化全库。
- A-T1/A-D1、CLI 生产/测试、HTML/资产/输入、其他 Rust、review 和 F:/modenmapbox
  均未写；review 原副本仅只读列目录，不执行其中脚本。
- 未执行 Git 写操作/暂存/提交/推送；既有 dirty 不清理、不归属。
- 测试进程已结束，释放本轮独占窄 Rust 槽，等待总控验收。费用/token 无法取得为 unknown。
