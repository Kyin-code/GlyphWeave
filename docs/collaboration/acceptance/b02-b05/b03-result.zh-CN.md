# B-03 结果：用途分区、地表执行与缺额闭合

状态：`partial_content`。

## 实现

- 新增显式 `residualLandProgram`（version 1）：城市开放空间比例 0..0.25、农村牧草地比例 0..0.50；缺省关闭。
- `LandcoverPlan` 记录 scene/source policy；residual 只从 `FutureDevelopment`/`AgricultureCandidate` 来源选真实 1m 地表，避水、障碍和坡度，并按比例停止。
- 新增 `PublicOpenSpace`/`PastureGround` 材质与 ledger 物理字段；共享农路继续由 agriculture/farm_access 只计一次。
- residual rejection 与 yard rejection 分开，修复了庭院守恒被新类别污染的审核错误。

## ≤1km 生成

`bevy/work/collaboration/ds-b-run/b03/land-execution-2` generate exit 0，worldRevision `86920e4d...`：

- publicOpenSpace 1293m²，pasture 32386m²，agriculture fields 19，farmTrack 2912m²。
- landcover realized 57744m²，unresolved 123760m²；urban realized 4385/16384m²，rural realized 84847/196608m²。
- ledger realized 138384m²、unresolved 123760m²，scene 面积守恒；frontage 仍 4/24。

## 结论

城市开放空间/农村牧草地已有可复算真实地表，但没有把全部未开发土地铺草冒充完成；用途缺额仍大，状态 partial。
