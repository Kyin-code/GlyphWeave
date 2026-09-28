# B-04 结果：形态 profiles 与公共服务关系

状态：`partial`（工程 profile 可编译，参数尚未完整展开到所有生成器）。

## 实现

- 新增 `settlement/morphology.rs`：六个 Rust-owned profile：compact-mixed-block、perimeter-block、low-density-residential、clustered-residential、terrain-constrained-compact、village-field。
- 字段包含来源 caseId、证据等级、地形、地块宽深、前后侧退界、高度、覆盖率、道路层级、公共空间比例、服务半径、农村田块模板；未知 ID/非法范围拒绝。
- 新增 `service_relations.rs`：学校/市场/市政与住宅或公共空间的路径距离关系；欧氏距离不作可达证据，实际仍由最终 AccessGraph 审核。
- `examples/morphology/b04-village-field.manifest.json` 已成功生成 `village-field-2`，worldRevision `8c8cab40...`。

## 案例索引

`docs/collaboration/b04-case-register.zh-CN.md` 收录中国/美国/欧洲各 12（5 城区+4 乡村+3 荒野），日本 6、香港 6；每条绑定本地研究文档或官方入口，证据等级和许可边界分列。

## 未完成

- profile 目前是验证/参数模块和代表性 manifest 入口，尚未把全部数值展开到 frontage/services/道路默认值；因此“同 seed 六 profile 几何统计对照”未完成。
- 资料仍有 source_pending/heuristic 项，不声称所有参数已官方核实。

## 结论

profile 与关系合同已实现；完整形态展开和研究覆盖仍 partial。
