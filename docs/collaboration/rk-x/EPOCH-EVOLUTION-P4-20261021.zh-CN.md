# P4 城市演化阶段验收记录

日期：2026-10-21

状态：**静态规划史切片通过**

## 1. 原则

P4 不把旧城和新城当成两套材质模板。最终建筑仍是 P3 容量结果；P4 在其上
增加可追踪的建设史：

```text
founding
→ old-core
→ expansion
→ industrial
→ modern-new
→ renewal
```

每个最终建筑都有：

- `cohort`；
- `operation`：`new / preserve / modify / replace`；
- `parentId`；
- 中心距离；
- 功能片区来源。

历史基座、改造、拆除和替换分别写入 `operations`，不用颜色伪造阶段。

## 2. 样例

输入：

`examples/geo-bay-route-1km.manifest.json`

输出：

`bevy/work/rk-epoch-p4-20261021b/`

中心：`(592,528)`。

最终 15 栋建筑的 cohort：

| cohort | count |
|---|---:|
| old-core | 3 |
| expansion | 9 |
| modern-new | 1 |
| renewal | 2 |

最终建筑操作：

| operation | count |
|---|---:|
| new | 10 |
| modify | 3 |
| replace | 2 |

规划事件：

- 5 个历史基座；
- 17 次新建事件；
- 3 次旧核心现代化改造；
- 2 次历史住宅拆除；
- 2 次带 `parentId` 的中心住宅替换。

## 3. 验收

- `epoch-evolution-plan.json`：status pass；
- 15/15 最终建筑均有 epoch assignment；
- 所有 replace 均有 parent identity；
- `scale-audit`：pass；
- `quality-report`：pass；
- 战略图用轮廓线型区分 preserve/modify/replace，不只依赖材质颜色；
- 同代码复烘焙：111 个可比数据文件 SHA-256 全相同，mismatch 0；
- 测试覆盖 modify/demolish/replace/parentId 以及半径单调性失败。

## 4. 边界

- 当前是规则化静态规划史，不是完整人口、地价、产权和拆迁行为模拟；
- 历史基座由最终核心建筑的缩减体量生成，仍需要接入更真实的 48 城历史样例；
- 中国单位大院/封闭小区、美国收入分层社区、欧洲公共住房等细粒度 functional
  program 尚未全部接入；
- 建筑仍通过独立计划渲染，尚未完成 P3 到最终 EntityInstance/AccessGraph
  的统一。
