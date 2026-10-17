# G33 独立最终裁定：限定接受，保留流程超支与历史例外

日期：2026-09-28。

## 1. 最终裁定

**G33 在声明范围内接受为 `accepted_scoped_with_process_exceptions`。**

接受候选：

- build id：`g33-overview-minimap-3`；
- package：G33-R2 `cli-runs/c06-export`；
- accepted browser run：`g33r2-a05`，35/35、exit 0；
- C1/C1-R1：证据闭集管理收口成立。

G34 **不自动开始**，仍需新的产品目标、范围合同和预算闸门。

## 2. 产品接受范围

在当前保存场景 3D 只读预览范围内，接受：

- 140×140 SVG 导航总览小地图；
- 保存窗口 X/Z 映射与北向上方向；
- 当前相机水平参考面 footprint；
- `clippedToWindow` 实际裁剪语义；
- 小地图点击/拖拽移动 target.x/z，保持 target.y、yaw、pitch、distance；
- 交互越界 clamp；
- 第二 pointer、foreign up、cancel、lostcapture、blur 的手势所有权；
- 小地图与主图拾取/旋转/滚轮/Shift pan 隔离；
- 已选对象在小地图操作后保持；
- 两种视口、两种 surface、模式/图层/复位、坏包拒绝与恢复；
- 几何 `23814 / 7938 / 4096 / 8022 / solid7`；
- WebGL2、GL/console/page error 0 的声明范围。

## 3. 接受依据

### 产品与真实浏览器

- `g33r2-a05`：35/35、exit 0；
- packageUsed 明确指向 `c06-export`；
- c05/c06 六文件确定性一致；
- clipped 正类 5、负类 3，产品与独立 oracle 一致；
- 内含相机的 `clippedToWindow=false` 负例成立；
- 真实 SVG points、status、display 和面积均有记录；
- a04 明确标记为回退包诊断，不计入接受会话。

### C1/C1-R1 证据闭集

- C1-R1 manifest：39 项绑定文件；实际 43 项，另外 4 项为精确排除：
  - `manifest.json`；
  - `receipt.json`；
  - `delivery.json`；
  - `verify-log.stdout.txt`。
- 39/39 bytes/hash 一致；
- verifier exit 0、`ok=true`、`problems=[]`；
- 核验前后摘要一致，drift=0；
- C1 原始副本 20/20 一致；
- G33 首轮、G33-R1、G32-R2、检查方目录零漂移；
- C1 与 C1-R1 产品进程均为 cargo/CLI/Chrome/driver = 0。

## 4. 流程例外

限定接受不消除过程问题：

- G33-R2 实际 cargo 8/4；
- CLI 33/6；
- Chrome 5/2；
- driver 10/6；
- G33 首轮与 G33-R1 的历史超支；
- G32/G31 历史超支和旧归档缺口；
- a04 使用回退包，虽 35/35 但不作为接受证据。

这些数字按实际台账保留，不追认、不抵消、不合法化。它们是过程例外，不否定 a05/c06 在声明范围内的产品证据。

## 5. 明确未验收范围

G33 不代表以下能力已接通：

- ST18；
- 默认世界；
- 自然生成；
- 全城可达；
- 工程安全；
- route/bake/realize；
- 发布链；
- 真实城市生产闭环。

## 6. 下一步

G33 不再追加产品修复或重复浏览器验收。下一步只能先完成 G34 范围闸门：明确一个新用户目标、冻结 G33 基线、定义产品白名单、预算和停止规则。

在 G34 范围闸门完成并获得实现授权前：

- G33 包与证据只读；
- 不启动 cargo、CLI、浏览器；
- 不把 G33 证据复制为 G34 证据；
- 不自动进入 G34 实现。