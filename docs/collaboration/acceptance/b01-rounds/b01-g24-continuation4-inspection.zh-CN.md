# B01-G24 continuation-4 独立检查（2026-09-22）

## 1. 裁定

**本轮三项限定实现可接受，G24 达到 scoped delivered_for_inspection；按既定口径整体仍记 partial，不宣布真实工程、默认世界或 ST18 验收。** 没有发现新的生产逻辑漏检。剩余是动态文件系统环境无法复现、部分负例以单测覆盖而非每类都追加 CLI，以及历史 v2 包在当前 v3 合同下拒绝等证据边界。

## 2. 独立复核

### 核心与 CLI

- `cargo test -p glyphweave-cli --offline --bin glyphweave`：**33 passed / 0 failed**。
- `cargo test -p glyphweave-core --offline --lib engineering_ -- --test-threads=4`：**82 passed / 0 failed / 545 filtered out**。
- 当前最终二进制上独立执行 10 次抽查：全新固定窗口 export+audit；非正式 check 改 pass；重复 check；伪造 limitations；伪造/缩短 sourceBindings；错误 manifest 身份；报告写入包内；缺 natural 来源。结果分别为正例四段 pass，或指定的 contextCompleteness/sourceBinding 拒绝；缺 natural 为 sourceBinding fail、依赖阶段 notEvaluated、contextCompleteness notEvaluated。
- 报告路径在被审包根内被明确拒绝，未依赖预存文件冲突分支；固定来源和 worker 目录未被写入。

### 递归绑定和冻结

- continuation-4 清单 **113/113** bytes+SHA-256 匹配，0 未列文件；receipt 绑定 manifest/report/progress/read-only-verification 成立。
- 固定来源 4 文件、G22/G23/G23R1 保护源码/资产及 continuation-1/2/3 旧证据保持匹配；未倒写旧包。
- worker 声明的最终源码绑定：core engineering_context.rs 107271 B / 65da78c8…；CLI engineering_context.rs 83201 B / 9d0e1034…；lib/main 未改。

## 3. 三项实现判断

### P1 checks/limitations

源码和测试形成闭集合同：9 个固定 check 名称恰一次，未知/重复/缺失/额外字段拒绝；state、detail 前缀和禁止性断言校验；limitations 必备事实、漂移披露、去重和与重建结果的精确比较均受控。独立篡改的非正式状态、重复项、替换限制、矛盾声明均拒绝。

### P2 manifest 与结构化聚合

manifest 的版本、固定文件引用、context/marker 哈希、scene/window、sourceBindings 逐项集合、requiredFileSet 及物理来源均校验；来源重建不一致归 sourceBinding，格式/包内不一致归 contextCompleteness。代码不再用诊断字符串关键词决定阶段结果；失败优先 notEvaluated，阶段恰一条。

### P3 包读取边界

包根先 canonicalize；读取使用 core 的 root-confined resolver；规范相对路径、绝对路径、穿越、逐段 symlink/reparse、解析后越根、非普通文件、单文件 64 MiB 和累计 128 MiB 均有实现或具名单测；目录遍历错误不再静默吞掉。

## 4. 尚未声称的内容

1. 本环境无法动态创建 symlink/reparse point（未提权，系统返回 EPERM），所以只接受静态路径规则和单测，不把动态逃逸写成已实测。
2. unknown/missing check、重复限制、重复 binding 等部分边界由具名单测覆盖；CLI 采用代表性子集，不要求每个单测变体都重复成 CLI。
3. continuation-3 的 v2 包被当前 v3 生产者按未知版本拒绝，这是有意版本不兼容；旧记录和旧包未改。
4. 未运行 auto_roads 全套；其既有固定目录写入失败与本包无关。

这些不构成当前已发现的生产安全漏检，但也不能被写成“任意 Windows 文件系统攻击已实测覆盖”。

## 5. 预算和停点

worker 自报累计 cargo 25/25、CLI 58/58、辅助 6/8；本轮检查员运行 cargo 2、生产 CLI 10、只读绑定脚本 1，未运行浏览器/工程 build/生成/ST18/bake/realize/发布。没有追加 worker 额度，没有修改源码、固定来源或 worker 证据。

G24 当前适合作为只读保存场景上下文合同与导出审核基础冻结。后续若要闭动态 reparse 证据，应在具备 Windows Developer Mode/等价测试环境后单独授权；不要为此改宽规则或消耗产品额度。

## 6. 证据

检查员目录：G:/difyllmwiki/GlyphWeave/bevy/work/collaboration/inspector-b01-g24-continuation4-20260922/。

关键文件：bindings.json、cli-tests.log、core-tests.log、cli-sample.json、review-runs/。

**A 冻结、B partial、C 不推进。**
