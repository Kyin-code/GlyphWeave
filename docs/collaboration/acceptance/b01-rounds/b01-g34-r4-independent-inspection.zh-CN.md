# G34-R4 独立检查：原子快照部分成立，但像素证据与整合验收未闭合

日期：2026-09-28。结论：accurate partial / needs_changes；不接受 G34 收口，G35 不启动。
本次读取 R4 报告、checkpoint、源码、驱动、旧校准文件、包绑定和台账。没有修改产品、R4目录或旧证据，没有运行 cargo、产品CLI或新的产品验收Chrome；独立资料只写入本检查目录。

## 1. 可以保留的成立项

- R3关于“同一帧自相矛盾”的判断已被正确更正：原记录混合了渲染入口、拼装入口和不同canvas尺寸，不能作为产品自相矛盾结论。
- 当前产品源码确实新增单调snapshot serial、冻结cell/top-level对象、聚合计数与cell计数自检、`selfConsistent`字段，并把独立grid hook标记为`independent=true/fromRenderedFrame=false`。这是正确的证据模型方向。
- R4 probe-boot记录的交付包可启动，`modelLoaded=true`、snapshot存在、256 cells、selfConsistent=true、无console错误；这只证明启动烟测，不能替代整合浏览器验收。
- R3的五项验收误区已有针对性mutation/control，8022字段已废弃，几何合同改为4096/7938/23814/7，旧目录本轮快照零漂移的口径也比上一轮准确。
- 生产链在报告中有独立子进程和确定性说明；但最终R4闭集未完成，不能把生产链完成等同于交付完成。

## 2. T2仍没有成立：`r4-clip-pixels.json`是旧失败产物

R4报告明确承认新版窗口比例形状尚未运行。当前`calibration/r4-clip-pixels.json`仍含旧的世界坐标0..64映射，生成的polygon坐标为负值（例如[-560,-70]），五种形状`insidePainted=false`、`outsidePainted=false`、`allOk=false`。

所以“新版已按窗口比例修复”只是源码/驱动改动事实，不是像素验收事实。不得把旧JSON、结构断言或R3外部公布结果写成R4的新版像素通过。必须在新任务中先清理旧产物语义（保留为旧失败证据），再运行新版，保存版本/hash、实际SVG、采样点和RGBA；超时/栅格失败必须显式失败，不得无限等待。

## 3. T4挂起是可定位的驱动健壮性问题

R4 `driver/r4-browser.mjs` 的五形状像素段在`page.evaluate`内对每个形状执行：
`await new Promise(resolve => { const img=new Image(); img.onload=...; img.onerror=...; img.src=url; })`。
该Promise没有超时、取消或总场景deadline；任何Image事件不回调都会让整个driver永远等待，最后只留下intent而没有results。报告说“某个场景挂起”是准确的，但当前驱动没有把场景ID、阶段和失败写出来，因此无法接受“运行过但未完成”的T4。

下一轮必须：
- 每个scene有begin/end日志和独立deadline；
- page.evaluate内部的Image/栅格化也有明确超时并返回结构化error；
- 总driver无论成功、失败、超时或异常都写results/exit/checkpoint；
- 超时产生fatal/nonzero和剩余场景清单，不得只等待30分钟后人工猜测；
- 先用仅启动/clip像素最小driver定位，再跑完整矩阵，不能原样重复挂起。

## 4. T4/T5仍未完成

`runs/g34r4-a01/`只有intent，没有results；因此29场景没有任何完整结论，不能接受面积、坏包、几何、快照集合和性能的最终复测。R4的T5闭集也明确未做：没有manifest/receipt/delivery/read-only verifier/二次枚举闭合。

已修的R3执行问题不能自动转化为R4验收通过：mutation通过证明闸门能拒绝构造错误，不能证明真实交付场景完成。

## 5. 账务边界

R4报告已如实记录：cargo/CLI生产链可观察，但Chrome三次`exitCode`未观察，driver实际运行却未经过`runRecorded`，driver数为0只是台账下界而非真实次数。下一轮必须从第一次进程开始统一记录driver自身、真实child pid、开始/结束/退出或timeout；未知不得估算填入成功/失败。

R4两次过程问题：首次CLI先于cargo造成旧viewer包，随后被绑定检查捕获；audit因覆盖已有report退出1，清理输出后重跑。下一轮必须把正确顺序和每次create-new输出目录作为前置闸门，避免再次消费额度。

## 6. 执行质量评价

R4能主动更正R3错误、停止自宣、披露旧像素产物和记账缺口，这是积极行为。仍有明确的“做得不够好”清单，下一轮必须逐条回应：

1. 新版clip坐标修正后没有实际跑出新像素结果，却保留旧失败JSON，容易造成证据误读；
2. 像素等待没有超时和阶段日志，导致整合driver挂起30分钟才发现；
3. driver没有走统一记账入口，实际运行次数未知；
4. 生产链一度先CLI后cargo，生成旧viewer包；
5. audit输出目录未create-new，因覆盖旧report产生可避免失败；
6. T4挂起后没有写出结构化results/checkpoint，只留下intent；
7. T5闭集、verifier和二次枚举没有完成。

没有证据判断故意造假或主观懈怠；但以上是可验证的执行质量问题，不能只以“已经披露”代替修复机制。

## 7. 资料与下一步

独立资料目录：`G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g34-r4-20260928/`。
独立报告和R5执行令：
`G:/difyllmwiki/GlyphWeave/docs/collaboration/b01-g34-r4-independent-inspection.zh-CN.md`
`G:/difyllmwiki/GlyphWeave/docs/collaboration/dsh-b01-g34-r5-final-browser-closeout-task.zh-CN.md`