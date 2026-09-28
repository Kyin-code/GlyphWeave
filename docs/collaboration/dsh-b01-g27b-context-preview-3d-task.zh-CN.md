# B01-G27B：路径B真实3D/WebGL上下文预览迁移（独立可选任务）

日期：2026-09-22。

这是与G27A分离的可选任务。只有在产品明确要求真实3D/WebGL时执行；不要在G27A或G26R2中偷换合同。

依据：
- G26R2路径A收口：G:/difyllmwiki/GlyphWeave/docs/collaboration/b01-g26r2-inspection.zh-CN.md
- G23既有3D消费者：G:/difyllmwiki/GlyphWeave/docs/collaboration/b01-g23-inspection.zh-CN.md

## 0. 目标

设计并实现一个新的、明确声明为真实3D/WebGL的保存场景上下文消费者，输入仍是已审核G24/G25 context包。它必须从保存事实生成可核对的3D几何，不把G25 canvas2D结果改名为3D。

## 1. 允许范围

- 复用G23已冻结的本地Three/vendor或建立明确的新版本适配；
- 将natural/final高度栅格、实体轮廓、道路节点边、access observation path映射到同一世界坐标；
- 保持sourceObservationOnly/notReverified和路径A否定语义；
- 真实浏览器/WebGL2观察、模型计数、来源绑定和坏包拒绝。

## 2. 严格禁止

- 不接默认SceneIndex、ST18、自然生成、工程build、route、bake、realize、世界发布；
- 不把access connected/walkable_route升级为verified route；
- 不修改G23旧viewer或旧证据来凑兼容；如需适配，新增版本化入口并披露；
- 不改变G24高度、实体、道路或坐标合同；不插值造地形、不丢unknown kind；
- 不以截图美观替代逐字段来源重查。

## 3. 最小验收

- natural/final分层可切换且顶点/高度逐值绑定；
- 实体、道路、节点、access路径身份与世界坐标一致；
- 真实WebGL2加载、取景、重载和至少一次真实鼠标命中；
- 自洽natural/final/entity/access篡改由对应审核阶段拒绝；
- schema drift、unknown kind、empty access显式显示；
- 坏包不显示旧模型，报告和包只读；
- 与G25路径A页面合同并列存在，不能混淆2D与3D结论。

## 4. 建议预算与停点

建议新授权：cargo8、CLI16、浏览器5、辅助6、归档≤64 MiB；不使用G26/G25历史余额，不自动启动。若实现范围或预算不足，保留partial，不削弱安全/来源规则。

交付目录：`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/ds-b01-g27b-context-preview-3d/`

正分支只能声明“路径B真实3D/WebGL上下文消费者交检查”，不能声明默认世界、真实城市或工程安全验收。

**A冻结、Bpartial、C不推进。**
