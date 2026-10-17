# DSH B01-G28-R1-H1：动态HUD同步与最终证据一致性

日期：2026-09-23。状态：**窄范围修复任务，禁止扩展为美化或新地图任务。**

依据：`docs/collaboration/b01-g28-r1-hud-inspection.zh-CN.md`。G28-R1的地形索引、实际三角拾取和体验主链已修复；本任务只处理用户看到的HUD陈旧事实与新证据内部引用。

## 1. 预算

正式新增：cargo 2、CLI/服务3、浏览器1、辅助1、归档≤64MiB。加上G28及R1既有记录后的累计上限：cargo17、CLI20、browser12、aux11。当前可用：cargo2、CLI3、browser1、aux1。G28历史CLI+1超支不追溯合法化。失败尝试照计；不复用G27B预算；不申请hillshade预算。

若新增额度尚未由产品确认，先只使用R1剩余cargo2实施/测试，停在“代码已修、最终包待复验”，不得写成delivered_for_inspection终态；本任务的完整交付需要上述新增额度。

## 2. 允许范围

允许修改：`bevy/crates/cli/src/g28_viewer3d.js`，必要的`context_preview3d.rs`测试/资源接线；不改G27B、旧G28、R1旧目录或固定来源，不改地形拓扑算法，不开G29，不做hillshade/等值线。

新证据目录：
`bevy/work/collaboration/ds-b01-g28-r1-hud-sync/`

旧R1目录只读。最终包使用新viewer build id或明确资源版本，必须重新构建、导出和audit。

## 3. 必须修复

### H1-1 HUD刷新

把HUD生成拆为可重复调用的`fillHud()`/`refreshHud()`，并确保在每次render完成状态更新后刷新：

- surface切换后页面来源必须从`crop.finalMm`变为`crop.naturalMm`或反之；
-实体/道路/观测层切换后，页面显示的indexed draws、solid draws、drawn triangles、overlay状态必须与运行状态一致；
- HUD不能只依赖boot快照；state、runtime seam和页面文字三者必须一致；
- `fillHud()`不能把保存事实计数错误地替换成派生计数；只刷新展示派生行和当前surface/图层状态。

### H1-2 测试

至少新增具名可执行测试：

1. final→natural后HUD来源更新；
2. 关闭实体/道路/观测后HUD显示0 solid、7938 terrain、overlay hidden；
3. 恢复图层后HUD恢复8022与solid7；
4. 页面文字与`__terrainRuntime3d()`逐项相等；
5. 不改变model/source bytes和G27B/R1冻结绑定。

不能只断言JS源码含`fillHud`字符串，也不能只读隐藏state。

### H1-3 证据修复

新目录中统一记录：

- 最终源码、viewer资源、exe和新包hash/bytes；
- 真实audit报告；
- browser-record和每张截图的哈希必须互相匹配；
- 禁止引用已被后续复跑替换的旧截图哈希；
- 清单按精确相对路径列入嵌套manifest；receipt绑定所有关键文件；
- 明确R1旧目录未改，R1旧browser-record中的引用失配作为历史披露，不倒写。

## 4. 最小执行矩阵

cargo最多2次既有额度+新增额度建议总2次：一次单测，一次最终构建；若单测失败必须使用新增额度/准确停点，不用旧exe顶替。

CLI最多3：最终导出、最终audit、负例或确定性控制，实际剩余额度不足时以报告明确覆盖映射。

浏览器最多1次：同一会话完成final/natural、solid/road/overlay切换、HUD文字与runtime对照、三次重载、实体/地形既有命中 smoke、坏包拒绝/恢复至少保留一项。若浏览器工具策略阻断，记录notObserved，不填默认值；不绕过安全策略。

辅助最多1次：单一驱动完成上述读取与记录，不拆成多次“微观察”。

## 5. 停点

满足H1-1/H1-3及最小矩阵后，状态可到delivered_for_inspection，G28是否最终验收由独立检查裁定。若额度未批准或浏览器无法运行，保持needs_changes并交实际源码/单测结果，不把旧R1证据改称新验证。

不做：hillshade、等值线、地图生成、ST18、默认世界、EngineeringInput、route/bake/realize、世界发布，不改G27B和旧G28/R1。

**现在先处理HUD刷新，随后按新目录绑定最终证据。**
