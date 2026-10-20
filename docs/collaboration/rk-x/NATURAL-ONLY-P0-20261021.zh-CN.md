# P0 纯自然阶段验收记录

日期：2026-10-21

状态：**P0 核心切片通过**

## 1. 交付范围

- `style.naturalOnly=true` 现在是独立生成阶段，而不是“跳过部分聚落内容”；
- 自然阶段必须声明 `style.naturalGeography`；
- 湖区不再继承旧模板的滨湖道路、桥和村落；
- 自然植被、岩石和水体只能在自然白名单中；
- 任何道路、建筑、桥、街道家具、农业设施、工程地形或未知实体类型都会
  使最终审计具名失败；
- 输出根级与场景级 `natural-audit.json`，包含 `humanTraceCount`、工程
  操作数、道路/开发计划状态、被拒绝实体和失败原因；
- `naturalGeography` 湖岸加入确定性的多频岸线扰动；旧世界未声明地理背景时
  保持原椭圆形语义；
- 纯自然探索视图围绕湖体取景，避免场外空背景占据主体。

## 2. 样例

输入：

`examples/geo-bay-natural-1km.manifest.json`

输出：

`bevy/work/rk-natural-p0-20261021f/`

世界参数：

- 场景：`scene-0`，1000m × 1000m，4 个 stream chunk；
- 自然地理：海岸山地、900m 区域起伏、28°造山轴、湿润季风、旧河道
  260m 迁移、切谷；
- 水体：山间湖，水平水位、岸边浅滩和微风波面；
- 实体：3,426 个，全部属于自然白名单。

| kind | count |
|---|---:|
| tree | 589 |
| bush | 420 |
| grass_clump | 1,914 |
| rock | 502 |
| lake | 1 |

## 3. 机器验收

`natural-audit.json`：

```json
{
  "status": "pass",
  "humanTraceCount": 0,
  "terrainOperationCount": 0,
  "roadPlanPresent": false,
  "developmentPlanPresent": false,
  "traces": [],
  "failures": []
}
```

- `scale-audit`：pass，3,426 个实体，4/4 chunk，失败 0；
- `quality-report`：pass，警告 0；
- 相邻边界最大高差：0.585m；
- 同 seed 同代码复烘焙：109 个可比数据文件 SHA-256 全相同，mismatch 0；
  三个报告文件因输出路径不同不参与逐字节比较；
- `visual-feedback.json`：pass，战略图与自然探索视图均无空白 chunk、无人工
  痕迹，湖岸为非规则岸线。

## 4. 测试

- `natural_only` 模块：自然白名单、未知/道路/建筑失败、宏观地理必填；
- 湖区自然模板：不得生成旧滨湖道路、桥或村落；
- 人工地标进入 `naturalOnly`：生成阶段 fail closed；
- authored 湖岸：不同方向半径存在确定性差异，不再是完美椭圆；
- `cargo check -p glyphweave-core --tests` 通过。

## 5. 明确边界

- 本记录只证明 P0 自然阶段与一个山地湖谷切片，不证明 48 城均已完成烘焙；
- 当前植被仍是自然类别占位模型，生态群落和地貌—土壤—物种适配尚未完成；
- 48 城结构化先验、季节/灾害图和自然视觉质量仍需要后续样例扩展；
- P1 非直线地形道路尚未开始。
