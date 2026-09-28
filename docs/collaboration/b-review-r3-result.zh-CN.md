# B Review R3 结果：联合预检与独立可行正例

状态：`partial`。独立正例通过；原冻结自然案例仍 unresolved；未签收 B，未推进 C。

## 分类结果

| 项目 | 状态 | 说明 |
| --- | --- | --- |
| originalNaturalCase | unresolved | 未找到 24 个合法完整 parcel 的联合见证；未再 bake 原自然场景 |
| syntheticOrAuthoredPositiveControl | pass | 合成 flat/urban `integration_fixture` 通过 24/24/24、两级路网和真实 yard 审核 |
| jointPlanner | pass/limited | 新增生产链联合审核，单次最多 256 个 joint candidate state；不是全局搜索器 |
| cliIntegration | not proven | 当前 CLI 没有表达测试 raster/zone 注入的接口；未伪造 manifest 或跳过审核 |
| contentStatus | partial | 原自然场景和 B 内容仍未闭合 |

## 联合预检实现

新增 `worldgen/road_parcel_feasibility.rs`，复用现有生产函数：

- `road_alignment::AlignmentContext` / `design` / `road_extensions::append_one`
- `frontage::append` / `frontage::audit`
- `development::plan/validate/verify`
- `landcover::plan_landcover/realize/verify`
- `build_access_graph`

每次联合审核重新构建 entity、access、parcel、landcover 和 yard surface，单条 design pass 不能绕过完整接入、地块竞争和 AccessGraph。`MAX_JOINT_CANDIDATE_STATES=256`，超界明确拒绝。

节点高程合同：

- 同一共享节点在相连边端点必须高度一致；
- 固定节点 hash/高程不可被新候选重规划；
- `requiredRoadElevationMm`、`allowedRoadElevationMm`、`maxFillMm=6000` 分列；R2 的 7120mm 是坡度传播后的最低路面高程，不是 7.12m 填方厚度。

## 独立可行正例

正例使用独立合成 flat raster 和全 urban zone，ID/来源为 `b-r3-feasible-fixture:flat-authored`，不修改 `b-r1-control` 或原自然案例。

- 路网：base edge → 16m collector → 12m local，真实 entity/profile/父接点链。
- 需求：16 个 `residential_home` + 8 个 `storefront`，12×10、8/6m 高、4/4/12 insets。
- 结果：requested/planned/realizedWithAccess `24/24/24`，unfilled 0，24 parcel 每个 20×26、520m²，yardUnresolved 0，accessConnected=accessRequired。
- 实际 yard surface 由 `landcover::realize` 生成并再由 `landcover::verify` 独立重建。

单变量破坏均被生产审核拒绝：

- 删除一个 ManagedGrass yard patch；
- 修改一个固定入口 anchor；
- 两个 parcel boundary 完全重叠；
- 固定 base node 从 0mm 改成 8000mm；
- 同共享节点两端高程不一致；
- z224 北侧和 z160 南侧的 20×26 parcel 越过 128..256m urban 边界。

## 原自然案例

`examples/b-r1-control.manifest.json` 和原自然 height 未改。R2 的 station45 失败保留：

```text
original corridor: [258,224] -> [258,160], 16m, 8000mm/8000mm
requiredRoadElevationMm=7120, allowedRoadElevationMm=7040, maxFillMm=6000
```

7000/7000 的孤立 design 可过，但接回 8000mm 固定父节点会被 junction height 合同拒绝；不能把 design-only 结果当作连通网络。

R3 没有新的原自然场景 512m 生成：R2 已完成的有界搜索没有产生 24 个合法完整 parcel 的联合见证，因此不满足“找到合法 24 候选网络才允许 bake”的准入。原场景保持 unresolved，不扩大搜索预算。

## 验证

- `cargo test -p glyphweave-core`：410 passed / 3 ignored / 0 failed。
- `pnpm doc-tree:check`：PASS。
- `git diff --check`：exit 0，仅既有 CRLF 提示。
- R3 新 512m CLI 生成：0 次。
- gate、maximum-world、release、240 块浏览器：未运行。

## 未完成

- 原自然 128×128m urban zone 仍无合法 24 联合见证。
- CLI 不能表达合成 test raster/zone；正例是 production-function integration fixture，不是 `generate-world` CLI 证书。
- 原自然案例的 collector/local 两级实网和 24 完整庭院仍未完成。
