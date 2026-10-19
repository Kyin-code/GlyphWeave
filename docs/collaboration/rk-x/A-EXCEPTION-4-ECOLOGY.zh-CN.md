# A 里程碑例外 #4（生态配额缺 1 株）：证据状态

## 缺陷与修复

最大图曾出现 `planned 342,380 / realized 342,379 / unfilled 1`，拒绝记录为
`scene-0.landcover.4563.conservation.208.32.canopy.4`、层 `canopy`、第 5 株、原因
`no clear full-size ecology anchor in source patch`。

成因**不是容量**：锚点随机选取，早被接受的一个锚点可能把后面实例的位置关死，于是一块本可
放下全额的地报出 1 条拒绝，既不重试也不声明地不够。

修复（`fix(ecology)`，2026-10-18）：短少的 patch 在**确定性足迹晶格**上整体重排，只有实际
多放才采用；达标 patch 永不触碰，因此完整世界字节不变。唯一准入谓词 `anchor_is_clear` 由
**锚点扫描、重排、独立 verify** 三处共用；足迹宽于 patch 仍是具名拒绝而非静默短少。

## 真实世界测量（24 个世界，修复前 vs 修复后两个二进制）

- 山城 `hillside-town-512` seed 83–94；平原村落 `multiscale-farms` seed 42–53。种子先固定
  后开跑，全部报告，含被拒的 seed（4 个山城 seed 因道路坡度/台地不平被拒，与生态无关）。
- 20 个 512m 世界 = **4,352 patch / 38,471 株 / 0 拒绝**；`planned > realized` 的 patch 数为 0。
- 另含滨水 off/derived、山城 off、1km 探针（6274/6274/0）。
- **修复前 vs 修复后：planned / realized / unfilled 差值全部 0，植物 moved/added/removed 0，
  worldRevision 20/20 相同。**
- 逐文件 SHA-256：24 对世界共 **2,552 个文件，2,528 个逐字节相同**，其余 24 个都是
  `generation-report.json` 里的 `output` 路径。审计、`scene.json`、`landcover-surfaces.json`、
  预览全部相同。

**结论**：修复对已达标世界是**严格 no-op**，但重排分支在 512m 上**从未进入**
（`unfilledPlantCount` 全为 0）。这是预期结果而非修复无效：已发布最大图是 38,825 patch 里
1 个拒绝（2.6e-5），本轮 4,352 patch 的命中期望 0.112，`P(≥1) ≈ 10.6%`；要有 50% / 95% 把握
需约 124 / 535 个 512m 世界。

## 因此：例外尚未关闭

真实世界唯一已知实例仍是 **6×10km** 世界。关闭它需要的是一次 6×10km `generate-world`
（约 380–420 s），期望得到 `342,380 / 342,380 / unfilled 0`。该烘焙**未获授权，尚未执行**。

若改走"重放已发布世界"路线，注意 `real_world_dir_linear_indexed_ecology_equivalence`
（`#[ignore]`）断言"重放 == 已发布 ecology"；修复后重放应为 342,380 ≠ 已发布 342,379，
该断言必须随契约一起更新，不能原样保留。

## 已可写入验收的部分

- 修复机制有模块测试见证：用已发布的 4 个 anchor 重排后达到 5+3+3、0 拒绝。
- 修复对 24 个真实世界严格 no-op（逐文件比对）。
- 未关闭的原因与关闭所需的条件已明确，不是"未知"。
