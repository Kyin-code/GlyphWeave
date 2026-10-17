# B01-G7 离线提案独立审查（2026-09-19）

## 结论

**accepted-proposal-for-bounded-experiment**。接受station18北/南为首选、共享B=3110mm为单一变量方向，批准下一任务B01-G8至多2态、1次自然harness。此为实验输入批准，不是工程/地块成功证明。下面几处来源/措辞错误随G8追加勘误，不单开G7修订轮。

## 独立核验

- G7 receipt 8214bytes，SHA256 7f7bb4f618834af56deb6b90f060a558826c83345ebcccf7f4baf43b83d2611f。
- 9项回执绑定、20项source-hashes条目匹配；G6-R1回执21项、台账7项仍匹配。读取16条window-state记录与原正确32行/完整profile交叉核对，设计值、自然极值、必要上下界及B/C传播算术均一致。
- station18原设计2280；北最高自然2307、南2390；B起点9m平台后到station18距离9m，B=3110给起点传播2390。两窗必要高程上界分别7726/7773，故现几何no-cut必要条件有依据。
- 这是从现有profile/代码得出的条件性响应；新C的infer结果、AB可行性及完整联合审核未运行，不把2390写成新实测值。
- 检查员自然harness/production solver/design/append/realize/bake/cargo全部0，仅hash、已有JSON与两个natural栅格点读取。

## 需要随下一轮纠正，但不阻塞试验的小问题

1. BC从B(288,180)走向C(176,180)，**B在东端、C在西端**，文中相反；符号和算术未随之写错。
2. road_alignment::solve受自然下界/固定端点/坡度约束决定最小包络，填方上界用于可行性拒绝；**carve合成不参与这个纵向solver**。windows.endpointRule里的“natural / fill / carve composition sets the level”应改清楚。
3. road carve的18m是初始blend值，实际取max(18,3×(groundHeight+32))。原J9-R2 BC中心profile station56=542mm，对应实际blend=97.626m，不能把18m当真实外边界。两观测点距中心线8/9m，位于7m整平带外、真实blend范围内这一判断仍成立。
4. (196,171)/(196,188)的自然值并非不可知：冻结natural文件中分别为-606/-535mm。原冻结scene.json也保留实体几何，只是G6-R1候选包未保存全部实体。不能说整个证据集合没有这些数据。精确的before填方贡献者，本次仍未确定，也不据此授权carve重放。
5. before5074/4265、road profile2120、after1960属于不同来源层；候选前侧连接carve覆盖两点、硬支撑覆盖分支与变化相容，这是**静态机制解释**，不是实际贡献者已被逐个追踪的实测证明。不要因一个局部分支只抬高就宣称整个apply_terrain_carves单调；末尾full_target分支可能覆盖此前blend。
6. 北窗fill upper应为7726而非回复里的7729。文档/JSON主表7726正确。没有必要修改旧回执，后续追加说明即可。

## 当前整体位置

A于2026-09-09正式签收，并已按记录提交推送归档。B持续partial：基础地块、道路宽度层级、effective profile编译、真实地表与审核保护链已有实现及小控制，但当前冻结自然W1诊断中的24个frontage请求仍零真实增量。G5已在两态中通过新AB/BC道路append与196格接点匹配；G6-R1说明8个几何窗口仍被no-cut/支撑变化阻断，尚未进入候选完整final/development/yard/access联合realization。

本轮G7把下一步收敛为station18两个窗口的最小+110mm端点提案。即使G8出现1个真实新增，也只是突破首个增量，后续仍须实现完整24请求、小图内容准入、B03/B04剩余内容/覆盖问题，再进入B05本轮最大图/gate/release/浏览器验收。不能把A历史浏览器通过复用为B变更已通过。

## 下一步

见ds-v41f-b01-g8-st18-first-increment-task.zh-CN.md。1次自然执行预算是硬停止规则，失败不得擅自重试；预先对保存JSON/合成fixture完成字段/schema/profile长度/阶段统计检查。先测量，有真实预测才惰性baseline≤1/candidate realization≤2；bake0。任何超时/错误/缺证据按失败或incomplete交回，不自行增加B值。

证据目录bevy/work/collaboration/inspector-b01-g7-20260919/。本轮无源码修改/agent/Git写操作/F盘写入。
