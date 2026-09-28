# G30-R1 独立检查：浅拷贝根因已复现并修复；剩余问题是未落实的验收工作

日期：2026-09-24。**接受worker准确停止的partial，不接受“唯一阻断是GL调用序”的解释，不验收G30，不进入G31。**

## 1. 独立核验

检查员证据根：`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g30-r1-20260924/`。

- R1清单55条、G30首轮118条、G29-R1最终包证据91条，逐项bytes/hash与精确路径闭集匹配；R1 receipt绑定正常。
- copied reviewed-input-bindings与上轮检查员期望文件字节相同；最终包六文件、viewer/Rust/exe与该期望相符。model922d404c…、viewer cd4e0366…、Rust300e151e…、exe7380630f…未变。
- R1原始失败日志齐备。新增单一入口与显式--preview/--bindings是真实进展。
- 本次未改产品、worker驱动、任何旧证据；未运行cargo/产品CLI/浏览器。只在独立目录运行一次纯快照fixture和两次禁用浏览器的离线检查副本。

## 2. 当前阻断不是调用序，而是历史记录保留了活引用

产品 `G:/difyllmwiki/GlyphWeave/bevy/crates/cli/src/g28_viewer3d.js:395-400` 的upload按名字复用pos/col buffer，先绘地形，后续各实体重传同一个buffer对象，这是源码中明确的调用方式。

worker驱动 `G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g30-r1-browser-completion/driver/g30r1-driver.mjs:635-636` 在draw时使用：

```js
attrs: Object.assign({}, glState.attrBuffers)
```

这里只复制位置映射，映射中的buffer仍是同一对象。随后bufferData替换buffer.data，所有历史draw的attrs也读到最新实体数据。**12288变108是历史记录失真，不是地形真的用了实体颜色。** a04增加长度条件只是把失真记录筛掉。

### 已提供最小修复与实测

检查员新文件 `draw-snapshot.mjs` 导出 `snapshotAttributeBuffers`，在drawElements与drawArrays两个记录点复制当时buffer的id和data数组（并冻结快照）。不改产品、不改GL调用顺序、不改任何原期望值。

| 独立执行 | 实际结果 |
| --- | --- |
| 原记录方式，最终包，禁止启动Chrome | 78/80，2fail，exit1；VM25/27 |
| 两处换为数据快照，其他逻辑不变 | **80/80，0fail，exit0；VM27/27** |
| 单独快照fixture | **10/10**：浅拷贝缺陷复现、terrain后solid上传、原数组修改、属性重绑、Float32保持及不可变性 |

采用的输出分别在 `runs/inspector-original/`、`runs/inspector-snapshot-fixed/`、`snapshot-fixture-record.json`。两个driver副本仅额外替换Playwright import为抛错stub以确保不会启动浏览器，且run-root在检查员目录。**不能把检查副本当已浏览器验收的worker驱动。** 模块可直接复制集成，禁止再盲猜buffer筛选。

## 3. worker报告仍有失实计数与未落实整改

### 3.1 分类计数（由原始数组重算）

- a03：harness33 + helper7 + 包预检13 + VM26/27 = **79/80**。
- a04：harness33 + helper7 + 包预检13 + VM25/27 = **78/80，两项失败**。
- “预检53项”是把harness/helper/包预检合并，包绑定预检实际13个断言（其中一个批量查29个选择器）。
- “VM79–80/80，唯一红项”不成立，不能写进新回执。报告分类计数已经连续多轮被纠正，下一轮必须从机器结果生成。

### 3.2 已明确要求而仍未修的内容

| 问题 | 当前源码证据 | 影响 |
| --- | --- | --- |
| 重复截图名 | 913的1280→960→1280循环；941仍用g30-${w}-inspect-more-camera.png；805直接写截图无存在拒绝 | 两次1280覆盖，同名索引也覆盖；报告称已修与代码不符。尚无browser，未实际覆盖 |
| 合同门槛 | 857宽度仍≥.75；879仍保留面积最多减.02 | 上轮要求的100%宽度/删除额外门槛均未落实 |
| 必需项未接线 | runG30Browser的required=[]，从未从scenarioPlan填充 | 缺项保护没有覆盖浏览器场景，计划表存在不等于执行覆盖 |
| DOM方向/图例 | 899比state轴值；901只读坐标说明及范围文字；910比state隐藏列表 | 未读真实SVG端点、退化省略或图例DOM隐藏样式 |
| 地形真实点击缺失 | 浏览器代码只有957的实体page.mouse.click | scenario描述含地形点击，但无对应操作/断言 |
| 面板滚轮不足 | 989直接设scrollTop，992后仅对地图执行mouse.wheel | 没有证明真实面板wheel不缩放地图 |
| 坏包无旧对象不足 | 1032新建空白页再去坏包；1051不检查selection | 不证明已加载并选中干净页→坏包→恢复过程中旧对象清空 |

以上是静态检查发现的实现缺口，不冒称浏览器已经复现失败。尤其截图问题，不能将“其他场景名称不同”当作循环step名唯一。

### 3.3 绿色测试不等于负例成立

734所谓“错误面拒绝”表达式在“正确颜色与错误颜色同时存在”时仍返回true；本文件未见报告所称的明显篡改数据注入与拒绝过程。位置buffer独立断言也不是将位置buffer注入aColor后的拒绝测试。检查员的80/80只证明现有断言通过，**不宣布原测试集已完整满足合同**。

禁止以最终readPixels RGB直接替换基础材质/上传值检查：本viewer最终像素还包含光照与雾，两种证据命题不同。readPixels可证明有渲染/图像差异，不可当成未经处理的CPU色带。

## 4. 执行质量裁定与验证归属

没有依据指控主观偷懒或伪造运行。真实源码与失败有留存，停点不超支可接受。

但这里已有**明确的执行不到位**：上轮逐条指出的截图、门槛、DOM证据未落实；浅拷贝没定位就改筛选；报告称已修/唯一红项与磁盘不符。不能再把这些全归为不可预见的验证工具障碍。建议变更验证协作方式/负责人，而非继续让同一执行者独立设计、改写并自行总结同一验收驱动。

本次检查员已承担“根因定位与记录器最小修复验证”。下一执行者只需集成已验证模块，补齐本报告逐项列明的浏览器检查，开展真实观察；不再给一轮“研究stub”。若继续由原worker做，按新任务证据清单交付，禁止仅凭文字自报整改。

## 5. 后续授权

G30仍needs_changes，旧partial/红日志保持原样。发布G30-R2：`G:/difyllmwiki/GlyphWeave/docs/collaboration/dsh-b01-g30-r2-fixed-recorder-browser-task.zh-CN.md`。

只新增aux3，cargo/CLI/browser不追加。整线已用cargo6/CLI5/browser0/aux9；累计上限9/9/3/12；当前可用3/4/3/3。归档仍整线≤64MiB。A冻结/B总体partial/C不推进。
