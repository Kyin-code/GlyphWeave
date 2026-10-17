# DSH B01-G32-R1：手势生命周期修复与一次导航收尾

日期：2026-09-27。**正式授权直接执行；不再重做G32设计，不进入G33。**依据：`G:/difyllmwiki/GlyphWeave/docs/collaboration/b01-g32-independent-inspection.zh-CN.md`。

## 0. 冻结和新目录

G32候选 `g32-camera-pan-1` 不采用；旧G32目录和失败记录冻结。新证据目录：`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g32-r1-pan-lifecycle-repair/`。基线为G31限定验收包；model、HTML、几何、mesh、拾取求交、光照、材质、读图、路径A、G23/core全部冻结，除下述手势白名单外不改。

允许修改：
- `bevy/crates/cli/src/g28_viewer3d.js`：pointerStart/state.pointerGesture生命周期、panCount/panMoves状态语义；
- `bevy/crates/cli/src/context_preview3d.rs`：生产者ID、导航帮助文案、相关测试；
- 新目录driver/fixtures/modules与证据，不改旧目录。

新build ID：`g32-camera-pan-2`。model仍必须`922d404c…`；HTML/材质/光照不变，viewer仅导航白名单变化。

## 1. 最小产品修复合同

必须先写`navigation-repair-contract.zh-CN.md`，再编辑源码：

1. `state.pointerGesture`作为唯一活动手势来源；pointermove/up/cancel/lostcapture/blur都读取同一对象，不能留下第二个未清闭包状态；或者同一出口明确清除pointerStart，二者只能选一种并具名单测。
2. 一次手势结束后pointerdown可建立新手势；旧move不能继续改变相机。
3. `panCount`只在一个pan手势完成时加1；`panMoves`统计应用的pan step；非有限输入`panRejections`只增加一次并保持相机不变。
4. Shift在pointerdown锁定模式；途中放开Shift不切换；普通点击/旋转/滚轮保持旧合同；cancel/lostcapture/blur后下一个普通点击正常。
5. 单pointer锁定，第二pointer不接管；UI/侧栏事件不进入画布手势。
6. 默认恢复同时复位target、yaw、pitch、distance；不能只复位角度。

修复必须是最小差异，不改pan数学合同、不改拾取算法。

## 2. 修复前必须通过的离线硬闸门

在启动cargo/browser前静态检查：

- driver能在默认折叠状态真实点击`#more-camera > summary`并断言open=true，再点击相机按钮；不能依赖列表或假设默认已展开；
- 所有`expectedScreen`调用签名统一并用canvas bounding rect加页面偏移；对1280/960各做fixture；
- 余额JSON用显式扁平字段，拒绝嵌套误读；启动前打印本次剩余和run id；
- 截图keys从唯一场景表生成，写入采用wx，缺项/重复项/超时均非零；
- 27项VM fixture必须全通过，且把15 fixture + summary的分类写清；失败不得启动Chrome；
- preflight校验新build/source/package绑定，不把当前文件读成自身expected。

## 3. 生产链和一次浏览器收尾

cargo仅1次：修复后运行`cargo test -p glyphweave-cli --bin glyphweave`（若需build，使用同一次cargo命令链按台账定义；不能用`--lib`试探）。新exe绑定后，CLI最多2次：一次export、一次audit；若确定性需要第二export，必须在台账中说明且不超CLI2，不能在失败目录上audit。

browser最多1次启动，推荐同一aux full：

- 首屏/普通拖拽/Shift拖拽/滚轮/点击烟测，两尺寸1280×800、960×640；
- 斜视和俯视pan，yaw约90°平移；正反回位；记录target前后、yaw/pitch/distance不变；
- 真实展开“更多视角”，再点普通旋转按钮；不再因隐藏details中止；
- Shift中途放开、pointercancel/lostcapture/blur及第二pointer受控事件；后续普通点击确认恢复；
- 已选实体pan后选择保留，pan后真实点实体/地形；普通点击不误判pan；
- 模式往返、两面/图层保持、摘要/详情面积、几何23814/7938/4096/8022/solid7、GL/console/page错误0；坏包拒绝恢复一次。

所有浏览器结果按“真实操作/受控事件/离线fixture”分栏，不能把VM27等称浏览器检查。若页面实际产品失败，记录具体状态并停止，不用夹具改成通过。

## 4. 预算和停止

本轮新增额度：cargo1 / CLI2 / browser1 / aux1 / archive16MiB。G32旧额度4/6/1/4已用、历史失败与G31/G30超支不重置、不吸收。启动前写balance-before；任何额度为0不得启动对应进程；参数拒绝也计数，不能事后折扣。

按顺序一次连续执行：contract → 静态/fixture硬闸门 → cargo → export/audit → browser → report/manifest/receipt。报告/预算/command-log/facts-ledger在最终manifest前写完，最后独立枚举；报告后若无产品运行变化只做管理绑定，不把postManifestDocumentation当排除授权。

## 5. 交付

成功只交`delivered_for_inspection`，不自行宣称通过；缺任何浏览器项准确partial。修复后若仍有真实问题，不追加第二次cargo/browser，保留G32-R1明确阻断并建议停止该功能扩展。A冻结/B总体partial/C不推进。
