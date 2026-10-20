# P7 完整生成链控制验收

日期：2026-10-21

状态：**单地理 1km 完整链控制通过**

## 1. 同一 bake 的阶段产物

输入：

`examples/geo-bay-route-1km.manifest.json`

输出：

`bevy/work/rk-fullchain-p7-20261021a/`

同一份世界包含：

| 阶段 | 产物 | 关键结果 |
|---|---|---|
| P0 自然 | `natural-terrain.json` / audit | 山地、湖、切谷、历史河道 |
| P1 道路 | `route-evolution-plan.json` | 2 条曲线道路、8% 纵坡、5.9m 最大地形填方、3 段高架 |
| P3 聚落 | `settlement-evolution-plan.json` | 93 候选、15 建筑、需求全部满足 |
| P4 epoch | `epoch-evolution-plan.json` | 5 历史基座、3 modify、2 demolish、2 replace |
| P5 市场 | `market-catchment-plan.json` | 260 需求、1 市场、覆盖率 1.0、缺口 0 |
| P6 工程 | `engineering-scale-plan.json` | 2 路堤 + 3 高架，预算全部批准 |

最终实体数：3,386 个自然实体；规划建筑与设施由独立 Rust 计划表达并由 HTML
直接读取，不另行伪造浏览器侧布局。

## 2. 验收

- `scale-audit`：pass，4/4 chunk；
- `quality-report`：pass，警告 0；
- `visual-feedback.json`：pass；
- 战略图可同时读到地形、道路、建筑、市场与 epoch 轮廓；
- 探索视图没有灰白地表空洞；
- 同代码复烘焙：113 个可比数据文件 SHA-256 全相同，mismatch 0；
- 主要测试：
  - natural-only 零人工痕；
  - 地形道路曲线、桥/高架；
  - 容量建筑和 `unmetDemand`；
  - epoch 父子对象与拆改；
  - 市场网络覆盖；
  - 工程规模预算和局部合同不变。

## 3. 仍是阻塞项的边界

1. 48 城目前有完整机制表和审核文档，但尚未全部转成机器可读数据并逐个烘焙；
2. P3 建筑仍是独立计划，尚未全部转回 `EntityInstance` 和既有 AccessGraph
   做最终入口逐栋审计；
3. 当前只验证一个山地湖谷切面，平原三角洲、湾区、特区、旅游区、放牧区和
   种植区仍需各自的控制样例；
4. 道路/建筑/植被仍是占位级视觉模型，不能宣称最终美术完成；
5. 宏观工程尚未与全部排水、挡墙、生态和灾害账本合并。

因此：主链逻辑和单场景闭环已成立，但完整项目目标尚未全部完成。
