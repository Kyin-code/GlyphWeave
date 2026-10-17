# R2/W1 收口结果：显式轴优先 + 道路几何版本绑定

日期：2026-09-17（Asia/Shanghai）。两项**分别报告**：

- **修订一（R2 轴选择）：ready_for_review**
- **修订二（W1 道路几何版本绑定）：ready_for_review**

只做这两项。已通过的院落三例、35/35 道路工程成果、T1/T3/T4、道路矩阵、R6 与原自然案例全部保留，不重做、不重开。未提交 Git、未启动 agent、未改 A 冻结材料/F 盘、未开放内部类型（`AlignmentContext`/`ZoningMask` 仍私有）。

## 1. 修订一：只计算真正缺省的轴

### 1.1 缺陷

`compile_frontage` 在逐字段分支之前仍调用 `effective_policy.footprint()`，无条件同时算并校验两个轴；因此“两个轴都显式合法”的请求会被**用不到的默认轴**否决。检查员 `axis-probe.rs`：low-density + storefront、width 12 / depth 16 / height 10、insets 8/16/26、setback 12 → 完整地块 44×50 合法，却被 `profile parcel box 26m wide cannot hold 2x16m side insets` 拒绝。

### 1.2 修复

`ParcelPolicy::footprint()` 拆成 `footprint_width()` / `footprint_depth()`，各自 checked、正偶数校验；编译循环按字段分支：**显式轴原样保留**（只记录 `original-input`），**只有缺省轴**才调用对应的推导函数。两个轴都显式时不再计算任何默认 footprint；一个轴缺省时另一个显式轴的合法性完全不参与判断。显式值越出 profile 启发式区间仍保留并在 `notes` 披露；严格解析 insets、未知字段、显式非法尺寸、consumer/asset/parcel 合同一律不放宽。

effective revision：`b04-c1-r2c-effective-programs-v4`（旧 v1/v2/v3 文档与旧世界全部保留、未覆盖）。

### 1.3 验收

| # | 输入（low-density + storefront，insets 8/16/26，setback 12） | 结果 |
|---|------------------------------------------------------------|------|
| 1 | 缺省 insets（8/8/26） | 10×16 建筑 / 26×50 地块（不变） |
| 2 | 显式 8/2/26，缺尺寸 | 22×16 建筑 / 26×50 地块（不变） |
| 3 | 显式 2/2/2，缺尺寸 | 推导 22×46 → storefront 合同拒绝（不变） |
| 4 | **两轴全显式** width 12 / depth 16 / height 10 | **通过**；完整地块 44×50；notes 披露越出 `parcelWidthM [16,36]`；显式字段未动、`parcelInsets` 原样 |
| 5 | 上例仅缺 depthM | width 12 保持（`original-input`），depth 派生 16（`profile-default`） |
| 6 | 上例仅缺 widthM | 真实缺省 26−2×16 无效 → 拒绝（唯一合法结论） |
| 7 | 对称深度反例：front8/side2/rear50，depth 显式 16 | 通过（parcel 12/16 × 74 ≤128、front 8 ≤ setback 12）；仅缺 depthM → 拒绝（50−8−50 无效）；仅缺 widthM → 通过（26−4=22） |

新增/更新测试：`explicit_axes_are_never_blocked_by_an_unused_default`、`footprint_deduction_rejects_degenerate_and_odd_parcels`（改为逐轴断言，width 退化不得报成 depth 退化）。`morphology` 定向 **28 passed**；检查员三例用其只读探针逻辑重跑（`inset-probe-closeout.json`）结果不变。未以修改测试预期替代实现修复。

## 2. 修订二：把道路几何版本绑进权威身份

### 2.1 适用范围与兼容策略（先定规则）

16m 整平规则（`max(7, 路面/2+1)`）只在宽度 > 12m 时与旧规则不同，因此**只有“可能产出 16m graded 路”的输入**绑定道路几何 revision：

| 触发条件（在可信的 effective 程序上判断） | 绑定 | 说明 |
|-------------------------------------------|------|------|
| `roadExtensions.corridors[*].roadWidthM == 16` | 是 | 显式 16m 走廊，**含无 morphologyProfile 的输入** |
| `automaticRoadExtensions.scenes[*].hierarchy` 存在 16m tier 且其 purposes 与该 scene 的 `priorityPurposes` 相交 | 是 | 自动道路可能选到 16m collector |
| `morphologyProfile` 存在 | 是 | profile 编译出的 hierarchy 必含 16m collector |
| 其余（无 graded 程序、或只有 8/12m 走廊且无 16m tier） | 否 | 沿历史身份合同，保持旧 revision |

判定只依赖 manifest/effective 程序本身，不需要先生成世界；不看 name/seed/rulesDir。旧字段缺失的处理：**应绑定却缺失/错误 → 拒绝**；未受影响 legacy 可以沿原合同（不静默当最新）。

### 2.2 实现

- `worldgen::road_geometry_revision_for_style(effective_style) -> Option<&str>`：上述触发规则，纯函数、确定性。
- `world_manifest_revision_with_geometry(manifest, compiled, geometry)`：可显式传几何版本（单测可传 v1/v2）；`world_manifest_revision` 自动从 effective 程序推导。payload 仅在绑定时加入 `roadGeometryRevision`，其余键与历史 payload 完全一致 → 无关 legacy 的 revision 逐位不变。
- 发布：`WorldIndex.road_geometry_revision`（`#[serde(default, skip_serializing_if=Option::is_none)]`），由 bake 按同一规则写入 `world.json`。
- 独立审核：`cli validate-world` 先用原始 manifest 重新编译 effective 程序，再推导期望 tag 并与发布值（缺失即 `None`）比较，不一致即 `road geometry revision mismatch: published …, expected …`；`world-quality-gate` 首段调用它，故 gate 同样拦截。

### 2.3 验收

1. **检查员的无 profile 显式 16m 输入**：只读探针取 W1 控制 manifest 并在内存移除 `morphologyProfile`（`revision-W1-control-drop-profile.json`）→ tag 仍为 `b02-w1-graded-band-v2`，带几何的 revision `a5961aa920feab60…` 与“无几何键”payload `383ec6de8d35ba8e…` **不同**；后者正是检查员用旧公式算出的值 → 差异**来自道路几何绑定**，不是 name/seed/rulesDir。
2. **automatic hierarchy 与 profile 路径**：单元测试 `road_geometry_revision_applies_only_to_inputs_that_can_use_the_16m_band` 覆盖（hierarchy 16m tier + 相交 purposes ⇒ 绑定；同 tier 但 priority 不相交 ⇒ 不绑定；profile ⇒ 绑定；8/12m 走廊 ⇒ 不绑定；无 graded 程序 ⇒ 不绑定）；同一输入重复计算 revision 相同（确定性）。
3. **A 冻结身份保持**：路径 `bevy/work/milestone-a-final-r51-20260908-v2-abs2/`，manifest `glyphweave.manifest.json`（28 733 B，SHA256 `b023377b93092a21…`）、`world.json`（998 B，SHA256 `833453465ca289d7…`）。只读探针结果（`revision-A-frozen.json`）：tag = 无（其 `automaticRoadExtensions` 为 v1 无 hierarchy、无 16m 走廊、无 profile）、带/不带几何 revision 均 `bcc8328c0618ac78756a37bd7eaf013db8098a4f9a73aec19e583822242e52c0`，与 `world.json` 存储值**一致**。另有 legacy 对照：`WorldManifest::default_demo()` 无 graded 程序 ⇒ tag 无、revision 与 `world_manifest_revision_with_geometry(.., None)` 相等（单测与 CLI 测试各一条）。
4. **几何版本单独改变身份**：`world_manifest_revision_with_geometry(&m, None, Some("graded-road-band-v1"))` 与 `Some(GRADED_ROAD_GEOMETRY_REVISION)` 结果不同（单测），与 morphology revision 无关。
5. **发布/审核正负例**（自给 TempDir，一次 512m bake）：`cli/tests/graded_geometry_revision.rs` 烘焙一个 profile 控制世界 → `world.json.roadGeometryRevision == "b02-w1-graded-band-v2"` 且与从 manifest 重编译得到的期望值一致 → `validate-world` exit 0；把 tag 改成 `b02-w1-graded-band-v1` → exit 1 且 stderr 含 `road geometry revision mismatch`；**删除** tag → 同样 exit 1；恢复 → exit 0。测试自带 `TempDir::drop` 清理，不依赖任何历史 work 目录。
6. 规则未改（仍 `max(7, 半路面+1)`）、未重写道路搜索、未开放内部类型；已有 35/35 生产几何证据引用（`probe-w1-collector-local.json`、检查员 road-replay）不重跑。

### 2.4 诚实拒绝（按任务允许）

新规则使**绑定前**烘焙的产品缺 tag，因而在 `validate-world` 被明确拒绝（不再默认当最新）：

- C1 两份世界、R1 两份世界、W1 控制世界（均含 profile/16m 触发）；例如 W1 控制 `revision-W1-control.json`：期望 tag `b02-w1-graded-band-v2`、存储 tag 缺、`storedMatches=false`。
- B01 控制（`examples/b-r1-control.manifest.json` 含 16m 走廊）：期望 tag 同上、存储 tag 缺（`revision-b01-control.json`）。

这些旧世界的**文件、报告、hash 全部未改**（见表 §4），其道路几何证据仍可由只读复算取得；不要求、也未尝试把它们重建/改写成新版本。

## 3. 文档修正（本轮同时落实）

1. 前轮结果“八个宽度/朝向组合” → **六组**（3 宽度 × 2 朝向）。
2. “临时搜索测试已删除”与文件状态不符 → 现为 `#[ignore]` 诊断 `w1_cli_control_corridor_search`（保留，不影响普通套件；普通 road_network 套件 12 passed / 1 ignored）。
3. “过渡带 18m” → 明确为 carve 初始 blend 参数，生产取 `max(blend, 3*(groundHeight+32))`；当时 transition 用例覆盖的是施工足迹内、整平带之外的 shoulder 环，不是完整外侧 blend 证书。
4. 前轮“只对缺省轴推导”“几何 revision 已记录”两处措辞已按 §1/§2 事实更正。

以上 1–4 已作为勘误追加到 `docs/collaboration/b04-c1-r2-b02-w1-result.zh-CN.md` 顶部。

## 4. 预算、旧证据与状态

- 新的正式 512m CLI 控制：**0 次**；自给 TempDir bake：**1 次**（§2.3 第 5 条，测试结束即删除）；未跑 release/最大图/浏览器/全 workspace/自然四档/ignored 搜索。
- 定向测试：core `morphology` 28、`worldgen::road_network` 12（+1 ignored）、`worldgen::frontage` 10、`worldgen::road_extensions` 6、`worldgen::road_parcel_feasibility` 8/2 ignored；CLI `--test graded_geometry_revision` 2、`--test morphology_compile` 2。日志见 `bevy/work/collaboration/ds-r2-w1-closeout/`。
- 旧证据完整性：C1 18 文件、R1 14 文件 hash 全部保持（§2.4 列出的“拒绝”是**审核判定**，不是文件改动）；R6 模块 hash 保持。
- `pnpm doc-tree:check` PASS；`git diff --check` exit 0。

## 5. 剩余 / 阻断（不在本轮范围）

- frontage 仍 0/24、六项质量缺额、原自然 512m unresolved；C 不推进、B 不签收。
- 绑定前世界的复验只能走只读复算（`validate-world` 会因缺 tag 诚实拒绝）；若后续需要它们重新可审核，应作为独立迁移任务处理（不覆盖旧世界）。
- 外部走廊预检接口仍缺（`AlignmentContext`/`ZoningMask` 私有）；本轮未开放，维持记录。

## 6. 结论

- **修订一**：逐轴 lazy 推导落地，检查员反例与对称深度反例全部按真实合同处理，来源表/严格解析/发布审核未放宽。状态 **ready_for_review**。
- **修订二**：道路几何版本进入权威身份与发布字段，独立审核从原 manifest 重建核对，负例（缺失/篡改）被拒绝，A 与未受影响 legacy 的 revision 逐位不变，绑定前世界诚实拒绝。状态 **ready_for_review**。

不自行签收 B、不推进 C，交付后停止等检查员复验。
