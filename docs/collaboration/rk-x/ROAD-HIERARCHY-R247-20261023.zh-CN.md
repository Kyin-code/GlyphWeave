# 道路等级与直线约束 r247

## 已实现

`routeEvolution.routes[]` 新增两个可审计字段：

```json
{
  "roadClass": "national",
  "alignment": "terrain-following"
}
```

### 道路等级

`roadClass` 现在是规划决定，宽度只做匹配校验：

| 等级 | 最低 right-of-way | 默认截面 |
|---|---:|---|
| `expressway` | 28m | 6 车道、中央分隔、两侧路肩/人行道 |
| `national` | 20m | 4 车道、中央分隔 |
| `county` | 14m | 4 车道 |
| `township` | 8m | 2 车道 |
| `local` | 6m | 2 车道 |
| `lane` | 4m | 1 车道 |

旧场景没有 `roadClass` 时仍按宽度推导，避免破坏历史 Manifest。显式等级
不匹配宽度会失败关闭。

### 平面线形

`alignment` 支持：

- `terrain-following`：沿山体、水体与既有自然节点搜索；
- `straight`：平地规划强制直线，不满足坡度/水面约束时具名失败；
- `straight-preferred`：优先直线，复杂地形才回退自然路径。

直线采样仍进入统一纵坡、填方、桥/高架和地形修改链；HTML 不自行拉直道路。

## 香港 r247 控制

显式等级统计：

- 快速路 1；
- 国道 1；
- 县道 8；
- 乡道 13；
- 本地路 6。

4,582 实体、7 市场/100% 覆盖、入口断连 0、建筑/道路碰撞 0；
`scale-audit=pass`、`quality-report=pass`。

## 未启用直线的原因

把城市网格全部改为 `straight-preferred` 后，Geometry 能生成直线，但当前
宽道路交叉仍会把大量平交口提升成超长 `overpass`，市场 15 分钟覆盖跌到
0.869。这个结果不能接受，因此香港控制先保留 `terrain-following`，只启用
显式道路等级。直线能力已经在 Core 就绪，等下一步路口/转盘/互通模板把
平交与立交区分后，再对平地网格启用。

这个失败试验也证明：直线不能作为渲染层的拉直贴图处理，必须和交叉拓扑、
纵坡、碰撞与市场步行网络一起重建。
