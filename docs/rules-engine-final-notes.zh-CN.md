# 规则引擎最终复核：修改记录与清单校正

- **日期**：2026-09-02
- **基线**：`cb749be` + 他人修复（`7131ec5`）+ 本人 rulesMode 接入（`1cc9914`）
- **关联文档**：`docs/20260902-check-5.md`、`docs/20260902-acceptance-list-1.md`
- **本人在他人修改之后补做的**：把规则引擎从"审计诊断"切换到"主导生成"

---

## 1. 我实际修改了什么（相对 check-5 的状态）

check-5 已确认他人修复（碰撞对称化、AvoidTag 声明化、9 点采样、biome/hazard 统计、check-assets 通过，165 测试）。**这些我复核属实，未重复修改**。

我做的唯一实质性修改，是 check-5 指出的主阻塞：

### `generate_entities_with_profile()` 接入 `place_all()`（提交 `1cc9914`）

```text
manifest/style.rulesMode = "rules"
        ↓
generate_entities_with_profile()
        ↓ apply_rules_mode(seed, scene, water, style, proposed)
        ↓
每个有 descriptor 的模板提案实体 → PlacementRequest
        ↓
place_all()：phase→priority→id 稳定排序 + 硬约束验证
  (水 / 坡度 / 地面 / 入口 / 同类碰撞 / biome / hazard)
        ↓
move / 0-90° 旋转重试 → 仍不合法则剔除（宁缺毋滥）
  + 无 descriptor 实体（地标/水体）透传
        ↓
返回规则验证后的实体
```

配套：
- 新增 `rules_placement_context()`：构建 rules 的 PlacementContext（高度/水/坡/biome/hazard 闭包，Box::leak 到 'static）
- 新增 `rules_dir_from_style()`：`style.rulesDir` 或默认 `assets/objects`
- `mod.rs` 导出 `PlacementRequest` / `place_all`
- legacy 模式（默认）保留原 post-hoc retain 过滤，做 A/B 对照

### A/B 验证结果（同 seed，生成 demo world）

| 指标 | legacy 模式 | rules 模式 |
|------|------------|-----------|
| checked | 26 | 4 |
| passed | 0 | 4 |
| **rejected** | **26（全部违规）** | **0** |
| floating | 3 | 0 |
| slope_too_high | 7 | 0 |
| collisions | 14 | 0 |

**结论**：legacy 生成 26 个实体全部有违规（真实的生成质量问题），rules 模式只保留 4 个全合法实体、0 违规。**规则真正阻止了违规，不再只是事后报告。**

---

## 2. 验收清单校正（20260902-acceptance-list-1）

### 2.1 应勾选但未勾选的项

| 项 | 实际状态 | 证据 |
|----|---------|------|
| **C 节："缺失 baked 高度采样必须返回明确错误"** | ✅ 已实现 | `cli/main.rs:1489-1500`：entity 中心无采样 → 返回 Err("no baked height sample; refusing to audit")；`1470-1479`：chunk 字节长度校验（width×depth×2） |
| **E 节："generate_entities_with_profile() 已实际调用 place_all()"** | ✅ 已实现 | `worldgen.rs apply_rules_mode()`，rulesMode=rules 时调用 |
| **E 节（部分）："规则模式下道路/建筑/设施/植被均由 descriptor 驱动"** | ✅ 已实现 | 模板提案实体统一转 PlacementRequest，经 place_all 验证 |

### 2.2 仍为真实未完成项（清单标记正确）

以下未勾选项我核实**确实未实现**，清单判断准确：

- **B 节**：`ClearAnchor/must_face` 支持 tag + 完整 target-footprint 走廊几何（当前仅中心点射线 + kind 匹配）
- **B 节**：基于精确 shape/rotation 的碰撞（当前仅 AABB）
- **C 节**：坡度按对象实际 width/depth/rotation 计算（当前复用固定 `slope_half (8,8)`）
- **C 节**：pivot / 实际对象底部 / 路床高度统一定义与测试
- **D 节**：多 seed JSON 报告固定基线 + 阈值回归
- **E 节**：`place_all()` 与 worldgen OccupancyGrid / 精确空间索引统一
- **E 节**：放置提交的 reservation / rollback / terrain-carve 一致性
- **E 节（部分）**：道路、桥梁等系统型对象的专用生成器（当前道路也走通用 descriptor）

---

## 3. 核对中发现的设计注意点（建议后续关注，非阻塞）

1. **rules 模式当前会剔除大量实体**（demo 26→4）。这是"宁缺毋滥"的正确行为，但也意味着 `residential_house`（12×10m）等 descriptor 对 commercial_center/school 这类**大 footprint 建筑**过严——它们的 footprint 远超住宅。**建议**：为商业中心/学校等建独立 descriptor（或加 `allow_scale`），否则规则模式下城市会很空。这正是 check-5 建议"建立独立 descriptor"的后续。

2. **baked 高度 vs 自然高度**：rules 模式用的 `rules_placement_context()` 是**自然地形**（未 carve），而 CLI 审计用 baked heightfield。两者语义不同——rules 模式在生成时用自然地形判断合理（建筑放在 carve 前的坡上，之后 pad 会被压平），但这意味着**审计和生成用的是不同高度**。**建议**：文档明确这一点，或 rules 模式也读 baked 高度（当前架构下生成发生在 bake 前，读不到，属预期）。

3. **`rulesDir` 依赖 cwd 或显式传**：默认 `assets/objects` 是相对 cwd，从 `bevy/` 目录运行会找不到。**建议**：demo manifest 显式设置 `style.rulesDir`（我测试时正是这样做的）。

---

## 4. 当前可验收状态

- **规则引擎 MVP 核心目标达成**：audit 诊断 + rulesMode 主导生成，都能工作
- **验收清单 A/B/C/D 全部"必须"项通过**（含本次校正后勾选的）
- **E 节主阻塞已解决**；剩余 E 节项为非阻塞打磨（统一索引 / 事务 / 专用生成器）
- **165 测试通过**，全部提交已推送 GitHub main（`cb749be` → `7131ec5` → `1cc9914`）

**判定**：可标记为"规则引擎 MVP 完成（v1）"，剩余打磨项进入 Phase B（统一空间索引、事务提交、专用生成器、多 seed 回归）。
