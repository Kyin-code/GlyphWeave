# A-D2 首轮候选：全域 preview 巡检与生命周期观测

日期：2026-09-08（Asia/Shanghai）。任务 A-D2，worker Luna。
状态：ready_for_review；真实世界集成状态 integration_pending。
这是可执行工具和小测试候选，不是240块真实浏览器通过证据，也不是里程碑A通过。

## 实际写集

唯一仓库 G:/difyllmwiki/GlyphWeave。本轮实际代码写集为：

- G:/difyllmwiki/GlyphWeave/scripts/collaboration/check-preview-world.mjs
- G:/difyllmwiki/GlyphWeave/scripts/collaboration/check-preview-world.test.mjs
- G:/difyllmwiki/GlyphWeave/adapters/html/app.js
- G:/difyllmwiki/GlyphWeave/adapters/html/core/render-stream.js

HTML 两文件仅生命周期、只读观测、请求代次/失败重试相关。没有编辑 ecology.js、
Rust、A-D1/A-T1、资产、依赖、许可证、旧整scene脚本或其他协作者文件。
没有修改 F:/modenmapbox，没有向review写入或执行review脚本。

## 实现范围

### 独立来源与覆盖

消费总控冻结的 glyphweave.render-parity/v1：worldFile/worldRevision、每scene的
source/header绝对路径/hash/bytes及每chunk完整geometry/descriptor/ID摘要。

- parity采用受限文件句柄读取（默认64MiB），不读权威scene；缺少当前parity文件或
  旧未绑定parity返回incomplete，不以旧r47代替当前r49。
- 先只读HTTP world.json和preview-scene.json，核对真实字节hash及descriptor集合；
  HTTP位置从world相对scene路径映射，不把本机绝对路径拼进HTTP地址。
- 浏览器运行时再次核对实际收到的world/header字节，阻止scene.json请求。
- active payload仍由真正production loader检查长度、SHA256、revision、scene/chunk
  和数量；实例ID与加载patch ID由浏览器快照取出，与独立parity摘要比较。
- ID摘要严格使用UTF-8字节字典序、紧凑JSON数组、SHA256；重复ID先拒绝，不能去重
  后掩盖。coverage阶段还独立检查跨块重复植物和全scene唯一植物/patch数。
- 导航使用实际descriptor中心，再减scene origin传给既有__focusAt；支持负origin、
  非零origin、最后部分块。无硬编码6000x10000或“首尾=全图”。

### 同页面与三轮复访

新增--browser分支是真正可执行Playwright collector，动态加载现有Playwright/msedge；
默认不启动浏览器。一次page.goto，后续scene切换用原选择器，导航用__focusAt。
先遍历所有chunk，再对每scene首/中/末去重关键区建立warm基线，随后三轮复访。
每条快照检查pageId、requested/completed generation、focus、当前highDetailChunk，
旧feedback和未完成导航不能通过。全局watchdog到故障上界时关闭浏览器。

同一页面额外执行一次chunk请求失败后同focus重试，和一次可控延迟的旧请求后
切换新focus；记录旧/新token、故障诊断及新focus完成快照。实际执行仍待RUN槽。
截断、坏hash、错revision、跨块重复/漏植物已经有Node判别性负例。

### 冻结预算（未放宽）

- 稳态active高细节chunk=1、payload cache=1、residentBytes等于对应descriptor.bytes。
- render-stream pending峰值<=2，第三个并发读取明确抛错，失败promise释放后可重试。
- 几何数量相对同focus warm增量<=8，纹理<=4，且不能每轮持续增加。
- heap用CDP HeapProfiler.collectGarbage后Runtime.getHeapUsage；相对warm<=64MiB，
  第2到第3轮增量<=8MiB。不能取得可比较post-GC数据则incomplete/unverified。
- 单步120秒；整体故障上界=实际chunk总数*120秒，不当作性能达标标准。

pending是render payload loader的在途读取，不是整个浏览器所有HTTP连接数；
residentBytes是经长度/hash验证的payload字节，不是反序列化堆/GPU占用估算。
GPU驱动字节数未暴露，明确null/unverified；renderer.info实际geometry/texture/
frame drawcall计数单独记录。没有把这些计数冒充显存字节。

### 必要HTML生命周期修复

- 新增版本化__previewStats()：返回detach副本，没有让调用者写渲染状态；提供代次、
  状态/错误、实例IDs、loaded/mesh/non-mesh patch IDs、缓存项数、真实renderer计数。
- __focusAt返回本次token。异步draw串行推进最新代次，旧terrain/asset响应不能提交
  到新focus；显式failed状态支持原focus重试，不再只留下错误文本。
- 新帧重建前释放旧frame资源、地形/实体缓存和未引用prototype；共享几何/材质/
  texture及非ecology实例matrix通过WeakSet去重释放。ecology仍由其原owner释放。
  原视觉构造、Rust实体/patch数据不变；不存在为减资源而删除权威实体。
- 记录对象创建/释放次数，真实GPU驻留是否符合预算必须通过浏览器复验。
- 为保留既有API，loader直接多chunk调用仍支持原ring用途；正式巡检显式ring=0、
  focus模式，任何多块稳态快照都会fail。这不是提高验收缓存预算。
- access_pavement patch原本由accessSurfaces绘制以避免z-fighting；观测将这类patch
  ID单独列为nonMeshPatchIds，而不虚构重复landcover mesh。

## 实际运行与结果

仅运行Node轻量测试/语法和只读静态检查，无浏览器、HTTP真实世界探测或Rust执行。

- 新D2测试56项；既有render-stream 7项、ecology-instancing 7项。
- 最终合计70 tests / 70 pass / 0 fail / 0 skipped，退出码0。
- 合成240chunk全遍历是Node数据/loader回归，不是WebGL240块证据。
- 四个修改代码文件的node --check均退出0。
- node scripts/generate-tree.mjs --check退出0；git diff --check（HTML写集）退出0。

完整命令/退出码/hash索引：
G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/luna-d2/commands.json

合并测试日志：
G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/luna-d2/final-command-0.log

其余static日志为同目录final-command-1.log至final-command-6.log。测试生成的全部
合成fixture位于同目录unit-*下。入口HTML快照为app.before.js、render-stream.before.js。

## 建议总控验收入口（浏览器命令未运行）

先确认实际服务使用本轮patched HTML、准备当前独立parity并授RUN槽，再调用：

```text
node G:/difyllmwiki/GlyphWeave/scripts/collaboration/check-preview-world.mjs --url http://HOST:PORT/WORLD/preview/index.html --parity G:/ABS/CURRENT-PARITY.json --out G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/luna-d2/NEW-BROWSER-RESULT.json --browser
```

上述HOST/PORT/WORLD/parity路径是必须由总控替换的示意，不是已验证地址或现存证明。
不带--browser时只核对compact HTTP绑定并写incomplete。--observations ABS.json可消费
collector的glyphweave.preview-traversal对象；不能与--browser混用。所有out在任何网络
或启动浏览器前先以wx创建，已有文件拒绝覆盖。退出0仅工具证据pass，退出1为fail或
incomplete，退出2为用法/输出路径错误；worldQualityStatus固定not_run。

## 具体未就绪点 / 首轮审查注意

1. 没有生产当前独立parity，也没有真实r49来源认证；由D3/总控后续补充。没有用chunk
   自证或旧r47数据冒充权威。parity来源、producer runtime和采集真实性仍须工具外认证。
2. collectBrowser分支、页面串行生命周期、现场网络故障恢复尚未由真浏览器运行；
   不声称视觉等价、真实heap/GPU预算通过或240块完成。需要总控先小量browser smoke，
   再串行完整巡检；若实际字段/诊断或资源计数不符，保留fail/incomplete后按锁回派。
3. 当前生命周期策略每次导航释放并重建frame，不再持有历史chunk缓存；实际冷/热
   导航耗时与GC效果尚未测量，不宣称性能提升。全库其他长期对象是否泄漏仍需实测。
4. 没有修改现存serve目录的导出副本；若服务仍用旧app.js，工具将因缺少版本化
   __previewStats或完成代次而不能通过，不能静默回退旧首尾巡检。
5. 未迁移旧整scene读取脚本、未做D3许可证工作、未改worldgen.rs、未开展T3/T2。

## 禁止项声明

没有cargo、server、新依赖、额外模型、后台进程或自动轮询；没有Git写操作，未暂存、
未提交、未推送。review包括后续three-r178-LICENSE材料均保持只读。本轮交首轮候选后
等待总控串行审查，不自行进入A-D3或宣布里程碑1=A达成。
