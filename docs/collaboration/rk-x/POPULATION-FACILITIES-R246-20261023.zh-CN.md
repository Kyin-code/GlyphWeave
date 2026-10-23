# 人口驱动需求与公共设施 r246

## 目标

住宅和岗位不再只靠作者填写的 `householdsTarget` / `jobsTarget` 拍数：
场景可声明居民、日间岗位、户均人口、人均零售面积和设施服务人口阈值。
户数、零售需求与设施需求由这些基数推导；建筑数量仍是合法地块和需求的
结果，不是完成配额。

## 数据合同

`style.settlementEvolution.populationProgram`：

```json
{
  "version": 1,
  "residents": 45212,
  "daytimeWorkers": 80000,
  "householdSize": 2.82575,
  "retailFloorAreaM2PerResident": 8.8474,
  "facilities": [
    {
      "kind": "school",
      "basis": "residents",
      "populationPerFacility": 30000,
      "maxCount": 6,
      "widthM": 40,
      "depthM": 32,
      "heightM": 14
    }
  ]
}
```

`basis` 支持 `residents`、`workers`、`daytime-population`。推导结果发布在
`settlement-evolution-plan.json` 的 `population` 与 `facilityRequirements`。

## 香港校准

- 居民 45,212；
- 日间岗位 80,000；
- 户均 2.82575 人，推导 16,000 户；
- 人均零售面积 8.8474m²，推导约 400,009m²；
- 学校、诊所、医院、宗教、社区中心、体育馆、体育场按服务人口阈值推导。

这些是切面的编辑假设，不冒充实测人口普查。未来可由城市人口栅格和
48 城公开规划指标替换，但不改变字段与闸门。

## 放置顺序和硬约束

1. 先生成住宅、办公和商业地块。
2. 完成鲜食市场选址与覆盖审计。
3. 再放置人口设施，避免医院或体育场提前吃掉唯一合法的市场沿街地。
4. 设施必须通过水体 footprint、坡度、道路 OBB、建筑 OBB、跨 Chunk seam、
   土方上限与入口审计。
5. 无合法点位时按设施类型具名记录，禁止缩小碰撞盒补数。

## r246 结果

- 4,582 实体，678 个规划建筑；
- 7 个市场，15 分钟覆盖 100%；
- 已物化学校 1、诊所 1、教堂 1；
- 入口断连 0，建筑/道路/细节碰撞 0，悬空/穿地/落水 0；
- `scale-audit=pass`、`quality-report=pass`。

大型设施仍有 7 类/9 个合法点位缺口。香港高密度网格中，大 footprint 常同时
碰到多条道路或既有建筑；这正是下一模块“重规划权重”的输入，不能通过放宽
碰撞合同解决。

## 下一步

- 增加候选拆除/合并权重：年代、结构、地价、临街潜力、地标保护、工程净空；
- 允许一组普通建筑经完整账本结算后改成医院/体育场校园；
- 拆除后在最终实体审计中重算需求、入口、市场和碰撞，失败则回滚整组；
- 再将该模型用于旧金山、上海和贵阳切面。
