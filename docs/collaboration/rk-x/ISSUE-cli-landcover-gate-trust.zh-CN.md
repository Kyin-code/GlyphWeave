# 未决问题：CLI 质量门读取 landcover audit，而非重算 surface

状态：**未修复**，2026-10-20 记录。

## 现象

`glyphweave world-quality-gate` 的 landcover 判定读取场景中已发布的
`landcover-audit.json` / `scene.json` 字段。删除
`landcover-surfaces.json` 中一个真实 baseline patch 后，gate 仍只报告
frontage 与 road audit 两条失败，没有出现 `landcover remains unrealized`。

## 已确认

- `glyphweave-core` 的 `settlement::landcover::verify` 会从最终地形和
  landcover plan 独立重建 surface，并能拒绝伪造 mesh
  （core 测试 `serialized_mesh_forgery_and_missing_layers_fail_independent_rebuild`）。
- CLI `quality-report` / `world-quality-gate` 当前没有调用该重算路径。
- 这与 D18.4“拒绝必须可复算，不能只信发布字段”不一致。

## 修复方向

在 `world-quality-gate` 的 scene 加载后增加：

1. 读取 `landcover-plan.json`、`landcover-surfaces.json`、最终 terrain、
   development plan 与 entities；
2. 调用 `landcover::verify`；
3. 将重算结果与已发布 `landcover-audit.json` 逐字段比较；
4. 任何差异写为具名 failure，而不是覆盖旧 audit。

修复前，landcover 的伪造防护只能在 core 测试层证明，不能作为 CLI gate 证据。
