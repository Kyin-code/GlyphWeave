# A-T1-R1-T：固定 frontage 核心测试候选

- 日期：2026-09-08（Asia/Shanghai）
- 状态：`ready_for_review`，**候选待总控编译和串行执行；不是测试已通过或 A 已通过**。
- 唯一仓库：`G:/difyllmwiki/GlyphWeave`。
- 只读核对 HEAD：`6020fbe2713e8c79c1b2934b244402dc93e1df14`。
- 当前分支：`refactor/authoritative-baked-audit`；未切换或修改分支。

## 写入隔离与基线

唯一 Rust 写入：`G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/worldgen/access.rs`，仅在原有 `#[cfg(test)] mod tests` 中插入
两个参数化测试、两个局部测试辅助及窄注释；生产段逐字节未改变。
本轮前该文件已经有固定 frontage 的生产保护和一条回归；全部保留，不归属到本轮。
其他既有 6 个脏 Rust 文件和脏 docs 不碰，Luna 的 CLI 测试不碰，A-D1 不改。
未执行 review 目录任何脚本，未写 review；未修改 F:/modenmapbox。

| 材料 | SHA-256 |
|---|---|
| G:/difyllmwiki/GlyphWeave/bevy/crates/core/src/worldgen/access.rs | e2ba76fe02b979ac877c8152ccd878b63f3bcd58d5825217bc4b2a86234b6b04 |
| G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/terra-t1-r1/access.before.rs（本轮开始前含脏修改的完整副本） | e5774ce54091b8c7d458747eecc7d11ca98c3b1f62852a0ccf036b2e700c34d1 |
| G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/terra-t1-r1/static-checks.json | bac0160c6cfed5d4ea3b54d92b6d0ef6745510237c53315f04a85e60fe292039 |

## 新增测试及判别力（尚未运行）

### 1. fixed_frontage_cardinal_yaws_signed_mm_nonzero_origins_keep_reachable_contract

4 个 yaw（0/90/180/270）× 2 个非零 origin（-256,384 与 512,-384）×
2 个非整米、非四分米支撑（-1354mm/+1354mm）= 16 个小 fixture。
使用真实 `RoadsideSite::entity()`，14×10m 非正方形建筑，128×128m 毫米地形。
明确的 cardinal 向量表独立核验门坐标、朝向、roadPoint、world_y 的欧几里得商和
bottom_offset_m 的余数；不是仅改 yaw 而仍沿用北门位置。
道路、墙的坐标和 yaw 同步变换；道路接点距建筑中心 39m。
断言完整实体列表不变、唯一 required link 成功，路线从实际墙面出发，终点为
对应道路边缘，长度 30m；不是只断言任意 connected。

### 2. fixed_frontage_cardinal_yaws_signed_mm_nonzero_origins_keep_blocked_obligation

同样 16 个小 fixture；18×1m 的旋转墙阻挡固定门。
每个样例先只清除 `frontage_road_point` 标记，用同一几何、同一入口验证 generic
fallback 真正换到另一面并可达（其余字段必须不变），避免“所有面都不可达”的假负向。
随后固定实体必须保持原完整 JSON：位置、yaw、毫米 grounding、门和 roadPoint
以及实体数量均不变；断言 0 connected / 1 disconnected、required link 保留，
原因精确为 `entrance_clearance_blocked`，不接受任意非零/任意断连。
失败 link 无 road/path/distance，不生成伪造访问面。
再用可达替代门做独立反例，重建图必须变为可达且不等于固定失败图，检验旧图不能
被当作最终实体状态的证据。

### 图和局部地形证据

两测试都将最终实体序列化后反序列化，**只用最终实体、最终地形和水几何**调用
`build_access_graph`，与返回图逐 JSON 比较；不将原 links 注入重建。
最终访问面接受重建图校验，并同重建图重新生成的面逐 JSON 比较。
完整 128m fixture 的 `BakedTerrain::content_hash()` 前后必须相等，覆盖局部
全部毫米样本、origin、尺寸和编码；**不外推为全地图自然层、保护区或土地账本通过**。

旧固定入口、generic/authored、负毫米 rebasing 和道路朝向测试原样保留。
本轮测试没有把访问搜索的 0.30m 入口阈值当作 1mm 高程审计；全流程 1mm 篡改
拒绝、CLI 原门/roadPoint 来源审计与恢复后重跑由 Luna/总控验证。

## 已执行的非 Rust 检查

| 命令 | 退出码 | 日志（绝对路径） |
|---|---:|---|
| `node G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/terra-t1-r1/static-checks.mjs` | 0 | `G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/terra-t1-r1/static-checks.log` |
| `node G:/difyllmwiki/GlyphWeave/scripts/generate-tree.mjs --check` | 0 | `G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/terra-t1-r1/tree-check.log` |
| `git diff --check -- bevy/crates/core/src/worldgen/access.rs` | 0 | `G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/terra-t1-r1/diff-check.log` |

Node 检查只验证文本插入范围、既有内容完整保留、生产段不变、参数矩阵和 fixture
坐标算术；**不等价于 Rust 语法、类型、编译或运行测试**。
Git diff-check 的 LF/CRLF 提示是当前仓库换行配置提示；未修改 Git 配置或执行格式化。

## 交总控的精准命令（未运行；统一串行 RUN 槽）

工作目录 `G:/difyllmwiki/GlyphWeave`。每条 `--exact` 应匹配 1 个测试（新增测试各循环 16 个样例）；
0 tests 不算通过。

```text
cargo test --manifest-path G:/difyllmwiki/GlyphWeave/bevy/Cargo.toml -p glyphweave-core worldgen::access::tests::fixed_frontage_cardinal_yaws_signed_mm_nonzero_origins_keep_reachable_contract -- --exact --test-threads=1
cargo test --manifest-path G:/difyllmwiki/GlyphWeave/bevy/Cargo.toml -p glyphweave-core worldgen::access::tests::fixed_frontage_cardinal_yaws_signed_mm_nonzero_origins_keep_blocked_obligation -- --exact --test-threads=1
cargo test --manifest-path G:/difyllmwiki/GlyphWeave/bevy/Cargo.toml -p glyphweave-core worldgen::access::tests::fixed_frontage_entrance_is_never_reassigned_even_when_another_facade_is_reachable -- --exact --test-threads=1
cargo test --manifest-path G:/difyllmwiki/GlyphWeave/bevy/Cargo.toml -p glyphweave-core worldgen::access::tests::generation_selects_another_facade_but_never_rewrites_authored_door -- --exact --test-threads=1
cargo test --manifest-path G:/difyllmwiki/GlyphWeave/bevy/Cargo.toml -p glyphweave-core worldgen::frontage::tests::negative_millimetre_frontage_is_not_raised_by_legacy_quarter_metre_rebasing -- --exact --test-threads=1
cargo test --manifest-path G:/difyllmwiki/GlyphWeave/bevy/Cargo.toml -p glyphweave-core worldgen::road_network::tests::roadside_front_doors_face_the_assigned_road -- --exact --test-threads=1
```

本轮 **cargo test/check/fmt 全部 not_run**，无编译退出码可报告。
不自行执行 cargo，也不以其他工具替代启动 Rust 编译。

## 未验证项与停止状态

- 新增测试的 Rust 编译、实际图搜索、正向/负向断言均待总控运行；未做旧逻辑 mutation 红测。
- 未跑 CLI 集成、全 workspace、最大图、真实 r49、HTML/性能；未重新证明全域自然/保护层。
- 未新增依赖、模型、网络 fixture、服务或后台进程。
- 未执行 Git 写操作；未暂存、提交、推送。review 目录始终只读。
- 本任务交候选后等待总控，不开始 T2/D2/C，不扩写生产逻辑。
