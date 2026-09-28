# DSH B01-G33-R2：clippedToWindow 语义与证据账目收口

日期：2026-09-28。依据 `b01-g33-r1-independent-inspection.zh-CN.md`。

**这是 G33-R1 的收窄修复任务，不进入 G34。**用户已允许使用较宽资源，不再因一次驱动错误拆成多个微小轮次；但每个实际进程仍必须真实记账。

## 0. 冻结与新目录

冻结、不回写：

- G33 首轮目录及 G33-R1 目录、manifest、receipt、旧截图；
- G32-R2-C1 推荐基线包；
- model、主图几何/拓扑、主图 ray/pick、G32 pan 数学与 pointer 生命周期、材质/光照/shader、读图语义。

新证据目录：

`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g33-r2-minimap-clip-closure/`

建议生产 ID：`g33-overview-minimap-3`。

产品白名单：

- `bevy/crates/cli/src/g28_viewer3d.js`：只修 G33 小地图 footprint 裁剪标记与相关新测试；
- `bevy/crates/cli/src/context_preview3d.rs`：生产 ID 与具名断言；
- 新目录 driver/modules/records/reports。

不得改变已确认正确的 north-up、映射、交互 clamp、半空间裁剪多边形、事件所有权或 G32 主图逻辑。

## 1. 产品合同

在源码前写 `g33r2-contract.zh-CN.md`，必须明确：

1. `clippedToWindow=true` 当且仅当视锥与保存窗口的交集相对原 frustum polygon 发生实际窗口裁剪；
2. 完整落在窗口内部时必须为 `false`；
3. 实际超出任一窗口边界并被裁剪时为 `true`；
4. 空/非法结果不得伪造 clipped=true；
5. points、面积、status 不能因修复标记而改变；
6. 新测试必须包含：yaw ±90°、默认斜视、俯视、浅视、一个完整内含窗口的相机和一个确实越界的相机；
7. 必须以独立 oracle 的 clipped 判定为 expected，不能只读取产品自身的 `points` 或面积。

实现建议：让窗口裁剪函数同时返回 `{ polygon, changed }`，changed 需检测顶点数量或坐标变化，不能只比较数组长度。

## 2. 证据账目合同

- `--run-id` 必填，目录 create-new，driver 副本与 hash 在执行前写入 intent；
- required 场景固定，fail/notObserved/missing/duplicate/fatal/未授权 N/A 非零；
- 每次 cargo、export、audit、browser、driver、验证脚本调用都写 argv/cwd/start/exit/用途；
- 最终 report、budget、facts ledger、command-log 使用同一台账源生成；
- 明确分别记录：计划额度、调用尝试数、成功进程数、真实 Chrome 启动数；
- 不用“同一链”把两个可独立计费的 product process 合并成一个 CLI 数字；
- manifest 最后枚举，receipt/delivery 绑定最终 manifest；旧 R1 数字只在新报告中纠正，不改旧文件。

## 3. 离线闸门

在 build/browser 前必须通过：

- source/package/model binding；
- 41 个受保护函数与 shader/材质冻结；
- clipped flag 的独立正负例全绿；
- 复现检查员的旧失败反例保持绿；
- 清单缺项/重复/0 fail 但缺 required 的 self-test；
- 账目一致性自检；
- R1/G33/G32 旧目录零漂移检查。

## 4. 生产链与浏览器

建议软预算：cargo 4 / CLI 6 / browser 2 / driver 6 / archive 32 MiB。

顺序：

```text
contract + source backup
→ offline clipped-flag fixtures + accounting self-test
→ cargo test + build
→ export + audit
→ deterministic export + audit
→ 一次正式 Chrome 会话
→ 若只发现驱动问题，可在同一轮追加一次诊断/复测，单独 run-id、单独记账
→ report/facts/budget/command-log
→ manifest/receipt/delivery
```

真实会话至少覆盖：

- 默认斜视、俯视、yaw±90°、浅视；
- 实际 SVG `points`、`display` 和 `state.minimap.clippedToWindow`；
- 一个确实裁剪=true与一个完整不裁剪=false场景；
- 方向、点击/拖拽、第二 pointer、已有选择、主图 pan/rotate/wheel/pick；
- 布局、几何、GL/console/page、坏包恢复；
- 不能只通过 `page.evaluate` 调纯函数替代 SVG/真实 render 观察。

## 5. 停止与交付

- 若 clipped flag 或账目仍不一致：准确 partial，禁止自宣通过；
- 若产品行为通过但真实浏览器未覆盖：delivered_for_inspection，不写完成验收；
- 只有产品、账目、闭集和真实会话均成立，才交 `delivered_for_inspection` 供独立裁定；
- G34 保持未启动。
