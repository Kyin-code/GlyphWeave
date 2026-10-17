G7离线提案通过限定审查，批准按B01-G8任务书执行一次受控实验。B=3110有明确目标：station18北/南完整支撑自然最高2307/2390，原设计2280，B端9m平台后传播到该站2390；这是待实测的条件推算，不是预先通过。

固定原A/B/C坐标、12m路/shoulder3/junction9/fill6000/rise80和所有原需求，只改共享B为3110。最多两态：新B下infer_end派生C，以及C=3110，去重；旧B3000对照直接引用，不重跑。先两段append/196格接点，再原顺序测量，有预测才惰性baseline≤1及candidate realize≤2。至少1个真实新增通过全部现有审核才叫见证，目标24不变。

先用已有JSON和合成fixture验完整发布schema、85/113点数、raw trace/profile、单位/失败输出，再启动自然入口。自然全轮仅1次、≤2态/4measure/32attempt/120s。任何自然失败立即停，禁止修后自行重跑。bake/大图/CLI/gate/browser均0。

几处非阻断勘误随本轮追加：B在东/C在西；18m只是blend初值，旧R2 BC实际97.626m；纵向solver不含carve合成；两旧点natural可读为-606/-535mm且冻结scene.json有实体几何，不能称整体不可恢复；精确before贡献者仍未确定，不另启carve调查。北窗上界7726。

完整边界以ds-v41f-b01-g8-st18-first-increment-task.zh-CN.md为准。B partial/C不推进，完成后停止等复验。
