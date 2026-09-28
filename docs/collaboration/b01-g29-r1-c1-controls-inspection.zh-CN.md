# G29-R1-C1 独立检查与最后一次控件浏览器收尾令

日期：2026-09-24（Asia/Shanghai）。裁定：**接受原14项控件缺口已由真实浏览器闭合；C1新增6项红仍未浏览器复测，G29继续needs_changes。** 产品与新包冻结，不需要再改源码、构建、导出或跑CLI；只差一次最终浏览器会话。

## 独立核对

- C1清单58文件、9张截图及关键报告按交付记录自洽；receipt binding字段、package build id `g29-map-first-view-1`、viewer/model来源字段成立。worker本轮未改产品。
- `b01-focused`真实浏览器记录：103项，96 pass、6 fail、1 notApplicable；报告逐项披露6项是驱动测量问题，不能把它们写成产品通过。
- 原14项红在四种模式/尺寸中以真实summary点击、子控件点击、面板滚动、选择详情展开闭合；该部分可接受为真实浏览器观察。
- p01/p02预检与离线夹具覆盖驱动修复；但最后6项只被离线夹具证明，未被Chrome复测。
- C1额度已实际用尽：本轮browser1/1、aux3/3，cargo/CLI为0。旧G29/G28/G27B超支和覆盖历史不回写。

本轮检查器尝试读取receipt时最初误假设sourceBinding键结构，失败发生在检查器目录且未写worker；现以receipt实际`binding`结构为准。该检查器失误保留于检查证据，不扩大为worker证据缺陷。

## 剩余范围

只需复测5项`summary-visible`、行级`fields-visible`及其展开后的实际可达/点击结果；必须使用修正版驱动的统一`REACHABLE/isReachable`、快照`selectionDetailsOpen`、requiredChecks摘要和真实run-id。若任何一项仍为真实fail，按产品缺陷路径记录具体selector/矩形/祖先details/裁剪/遮挡和截图；若全通过，G29才可交独立最终裁定。

不重做原14项、不改产品、不重跑CLI、不重建、不重新导出；新run目录不得覆盖C1旧目录。

## 新预算与任务

G29累计已到cargo6/6、CLI9/9、browser4/4、aux10/10。现正式追加仅：**browser +1、aux +1**；新累计上限browser5、aux11，本次可用browser1/aux1，cargo/CLI仍为0。此次新增不合法化任何历史超支。

已发布任务：
`G:/difyllmwiki/GlyphWeave/docs/collaboration/dsh-b01-g29-r1-controls-final-browser-task.zh-CN.md`

**A冻结；G29仍needs_changes；C不推进。**
