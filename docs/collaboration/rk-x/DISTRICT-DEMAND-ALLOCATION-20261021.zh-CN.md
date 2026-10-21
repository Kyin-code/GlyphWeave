# 分时代需求预留与全局回填

日期：2026-10-21

状态：生产路径已接入；1km 核心切面实烘焙、scale audit 与 quality report 通过。

## 1. 解决的问题

旧版聚落选择按道路遍历顺序消费全局需求。更新带通常先得到足够地块，需求在
到达新区前已经耗尽，结果虽然有 `old-core / renewal / new-growth` 原型，
实际城市却几乎只表现为更新带。

本轮不把“每区必须完成多少栋”写成固定完成数，而是给住宅、岗位、商业和服务
需求设置**阶段预算**：

1. 第一轮按 `old-core / renewal / new-growth / periphery` 的声明份额消费；
2. 某阶段供地不足时，其未使用份额保留到第二轮；
3. 第二轮才进入全城回填，避免在局部失败时把整张图判成不可实现；
4. 最终建筑仍只由合法地块、地块尺寸、中心压力、建筑原型和容量产生。

## 2. 输入

`settlementEvolution.districtDemand` 为可选输入。未声明时保留旧行为。

```json
{
  "districtDemand": {
    "version": 1,
    "allowSpill": true,
    "shares": [
      {"stage": "old-core", "share": 0.18},
      {"stage": "renewal", "share": 0.32},
      {"stage": "new-growth", "share": 0.40},
      {"stage": "periphery", "share": 0.10}
    ]
  }
}
```

校验规则：

- 阶段只允许上述四个稳定标识；
- 每阶段只能出现一次；
- 每项份额必须大于 0；
- 份额之和必须为 1，误差小于 `1e-6`；
- 住宅和岗位按整数预算分配，余数归最后阶段；
- 商业面积和服务预算按浮点预算分配。

## 3. 输出

`settlement-evolution-plan.json` 新增 `districtAllocation`：

```json
{
  "stage": "new-growth",
  "share": 0.4,
  "planned": {"households": 320, "jobs": 280, "retailFloorAreaM2": 3200},
  "delivered": {"households": 79, "jobs": 175, "retailFloorAreaM2": 588},
  "remaining": {"households": 241, "jobs": 105, "retailFloorAreaM2": 2612},
  "eligibleLots": 1,
  "acceptedLots": 1,
  "spillAcceptedLots": 0
}
```

字段含义：

- `planned`：阶段预留预算；
- `delivered`：阶段内及全局回填实际交付量；
- `remaining`：阶段内未由本阶段地块交付的预算；
- `eligibleLots`：物理合法候选地块；
- `acceptedLots`：该阶段最终产生建筑的地块；
- `spillAcceptedLots`：第二阶段使用全局剩余预算而接受的地块。

`remaining` 不会被写成“已完成”；全城需求是否闭合仍由总
`status / delivered / unmetDemand` 决定。

## 4. 1km 控制结果

Manifest：

`examples/natural-evolution-core-city-1km.manifest.json`

输出：

`bevy/work/rk-x/v3-district-allocation-1km-r1/`

- 住宅 `800 / 800`、岗位 `700 / 700`、商业 `8000 / 8000m²`、
  服务 `20 / 20`；
- 24 个需求/容量建筑计划；
- 旧城 2 个合法地块、更新带 22 个、新区 1 个；
- 旧城保留窄街宅，更新带产生公寓/街面，新区产生 `new-town-midrise`；
- 旧城未用预算进入全局回填，形成 15 个更新带 spill 地块；
- 新区内部住宅预算仍有缺口，没有用固定建筑数伪造完成；
- 最终 10 栋普通建筑、3 个市场，入口连通实体 13、断点 0；
- `scale-audit: pass`；
- `quality-report: pass`。

阶段预算证明“新区在需求分配上取得独立入口”，但不保证每个阶段都能完成其
预留量。当前新区只有 1 个合法候选，所以其剩余预算被明确记录；这比把更多
山坡强行建成房屋更符合地形约束。

## 5. 仍未完成

1. 入口步骤仍使大量坡地建筑在最终 AccessGraph 阶段被拒绝；本轮没有放宽
   0.3m 入口高差或 25% 步行坡。
2. 阶段候选数量受 `road_step_m` 和道路覆盖制约。新区需要更完整的站点/支路
   供给，而不是扩大阶段份额。
3. 市场已能进入 AccessGraph，但尚无与普通建筑同等的入口工程报告。
4. 本控制没有真正的新区站城、TOD 裙房和公共空间组合；当前 `new-town-midrise`
   只是第一个地块级切片。

## 6. 验证命令

```powershell
$env:CARGO_TARGET_DIR='bevy/target-main'
cargo test --manifest-path bevy/Cargo.toml `
  -p glyphweave-core --lib settlement_evolution::tests

cargo run --manifest-path bevy/Cargo.toml `
  -p glyphweave-cli -- generate-world `
  examples/natural-evolution-core-city-1km.manifest.json `
  bevy/work/rk-x/v3-district-allocation-1km-r1

bevy/target-main/debug/glyphweave.exe scale-audit `
  bevy/work/rk-x/v3-district-allocation-1km-r1

bevy/target-main/debug/glyphweave.exe quality-report `
  bevy/work/rk-x/v3-district-allocation-1km-r1
```
