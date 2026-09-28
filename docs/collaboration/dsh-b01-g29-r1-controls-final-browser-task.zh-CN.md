# DSH G29-R1-C2：最后一次控件展开浏览器复测

日期：2026-09-24。**正式授权立即执行；这是G29同范围最后一次浏览器收尾，不是新产品任务。**

依据：`docs/collaboration/b01-g29-r1-c1-controls-inspection.zh-CN.md`。

## 1. 预算与一次性运行

本任务只新增并允许使用：

- browser：1 次启动尝试；
- aux：1 次驱动执行；
- cargo/CLI：0；
- 归档≤32 MiB。

aux驱动执行必须在**同一个调用**内完成：唯一性/包hash/driver preflight → 启动唯一Chrome → 复测 → 写结果。不要拆成“先单独precheck，再单独browser”，那会消耗两个aux。若preflight失败，禁止启动Chrome，退出并交partial；不得再申请预算。

新证据目录：
`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g29-r1-controls-final-browser/`

确认run-id和run-root均不存在后原子创建；已存在立即拒绝。C1旧目录、G29/R1/H1/G28/G27B目录只读，不覆盖、不清空、不复用截图或记录。

## 2. 被测对象冻结

只读包：
`bevy/work/collaboration/ds-b01-g29-r1-final-package/packages/g29-r1-map-first/`

绑定不变：build id `g29-map-first-view-1`、model `922d404c…`、viewer `6812141c…`、exe `99fd458d…`。不改源码、不构建、不导出、不跑CLI。

## 3. 必须使用的驱动修正

修正版驱动必须在本次调用前通过静态/自检：

1. `runPrecheck`返回实际`harness.selfTest`和`harness.driverTests`，不再引用未定义变量；
2. `reachable-after-scroll`在主段和展开段都归入可达；
3. 捕获异常、缺必需项、必需`notObserved`、重复ID和意外`notApplicable`都使最终退出非零；
4. `selectionDetailsOpen`从快照复制，不拿同一活对象与自身比较；
5. 不对browse下隐藏检查面板执行点击；折叠details只在inspect中真实展开；
6. 记录原始actual和截图，不用评论扫描或正则替代真实操作。

预检必须证明：唯一run目录、被测包六文件hash/build id、VM 39/39、驱动夹具全绿；但预检结果与最终browser结果分开记录。最终浏览器失败也必须保留已有截图/actual并写exit。

## 4. 最后复测范围

使用1280×800和960×640，在inspect模式：

- `#more-camera`真实点击summary展开；5个相机子控件逐项实际点击并保存前后camera；
- `#evidence-details`真实展开；`#evidence`与`#limitations`按文本可达/滚动可达记录；文本详情不要求伪造点击；
- 对所有展开子项记录selector、rect、display/visibility、祖先details.open、滚动前后rect、裁剪、elementFromPoint命中；
- 对有控件的子项必须真实点击至少一个，并记录状态变化；
- 选择实体后展开selection details，核对details.open、`selectionDetailsOpen`、selectedEntityId、pickCount不漂移。

只复测C1剩余的5项`summary-visible`与行级`fields-visible`，不要重新声明原14项“本次新观察”。统一requiredChecks必须完整执行；任何真实fail或notObserved都保留partial，不用0/false补绿。

如驱动本身发现新的错误，记录为driverError并退出非零；不能在同一次aux内静默编辑后重新跑第二次。若需要修改驱动，下一次已无aux，必须停在partial并交出错误位置。

## 5. 退出标准

- 全部必需项pass、无fail、无notObserved、浏览器正常关闭、截图和browser record内部hash匹配 → `delivered_for_inspection`，交独立最终裁定；
- 任一产品真实不可达 → `needs_changes`，写具体selector与最小产品改动位置，但不能在本预算内修改产品；
- preflight/驱动失败或运行中断 → `partial_stop_allowance_exhausted`，保留raw actual/日志，不能宣称收口。

最终生成manifest/receipt/报告，但不改C1旧manifest、receipt或G29旧证据；嵌套包文件按精确路径列入，旧超支和旧覆盖历史照实引用。

**现在执行这一唯一aux+browser调用；不得再追加运行，不进入G30。**
